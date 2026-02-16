mod config;
mod state;

use crate::config::{load_config_from_path, load_config_from_str};
use crate::state::{Action, RuntimeState, UiSnapshot};
use gilrs::{Button, EventType, Gilrs};
use notify::{Event, EventKind, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use rfd::FileDialog;
use tauri::menu::{Menu, MenuItem, Submenu};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

const MENU_ITEM_LOAD_CONFIG: &str = "load_config";
const EVENT_STATE_UPDATED: &str = "scoreboard://state-updated";
const EVENT_ERROR: &str = "scoreboard://error";
const DEFAULT_CONFIG_NAME: &str = "basketball.toml";

#[derive(Clone)]
struct AppState {
    runtime: Arc<Mutex<RuntimeState>>,
    action_by_shortcut: Arc<Mutex<HashMap<String, Action>>>,
    action_by_gamepad: Arc<Mutex<HashMap<String, Action>>>,
    hotkeys_paused: Arc<Mutex<bool>>,
    active_config_path: Arc<Mutex<Option<PathBuf>>>,
    config_watcher: Arc<Mutex<Option<notify::RecommendedWatcher>>>,
}

#[tauri::command]
fn load_config_from_file(app: AppHandle, state: tauri::State<AppState>, path: String) -> Result<(), String> {
    let resolved_path = resolve_config_path(Path::new(&path))?;
    let config = load_config_from_path(&resolved_path)?;
    apply_config(app.clone(), &state, config)?;
    configure_config_hot_reload(&app, &state, Some(resolved_path))
}

#[tauri::command]
fn load_config_from_text(
    app: AppHandle,
    state: tauri::State<AppState>,
    content: String,
) -> Result<(), String> {
    let config = load_config_from_str(&content)?;
    apply_config(app.clone(), &state, config)?;
    configure_config_hot_reload(&app, &state, None)
}

#[tauri::command]
fn update_label_text(
    app: AppHandle,
    state: tauri::State<AppState>,
    id: String,
    value: String,
) -> Result<(), String> {
    let changed = {
        let mut runtime = state.runtime.lock().map_err(|_| "Runtime lock poisoned".to_string())?;
        runtime.set_label_value(&id, value)?
    };
    if changed {
        emit_snapshot(&app, &state.runtime)?;
    }
    Ok(())
}

#[tauri::command]
fn pick_image_source(
    app: AppHandle,
    state: tauri::State<AppState>,
    id: String,
) -> Result<bool, String> {
    let selected = FileDialog::new()
        .add_filter("Image files", &["png", "jpg", "jpeg", "gif", "webp", "bmp"])
        .set_title("Select Image Source")
        .pick_file();

    let Some(path) = selected else {
        return Ok(false);
    };

    let changed = {
        let mut runtime = state.runtime.lock().map_err(|_| "Runtime lock poisoned".to_string())?;
        runtime.set_image_source(&id, path.to_string_lossy().to_string())?
    };

    if changed {
        emit_snapshot(&app, &state.runtime)?;
    }

    Ok(changed)
}

#[tauri::command]
fn set_hotkeys_paused(
    app: AppHandle,
    state: tauri::State<AppState>,
    paused: bool,
) -> Result<(), String> {
    {
        let mut guard = state
            .hotkeys_paused
            .lock()
            .map_err(|_| "Hotkey pause lock poisoned".to_string())?;
        *guard = paused;
    }

    if paused {
        unregister_hotkeys(&app, &state)?;
    } else {
        register_hotkeys(&app, &state)?;
    }

    Ok(())
}

fn apply_config(app: AppHandle, state: &tauri::State<AppState>, config: config::ScoreboardConfig) -> Result<(), String> {
    let previous_runtime = {
        let mut runtime = state.runtime.lock().map_err(|_| "Runtime lock poisoned".to_string())?;
        let previous = runtime.clone();
        runtime.replace_config(config);
        previous
    };

    let paused = *state
        .hotkeys_paused
        .lock()
        .map_err(|_| "Hotkey pause lock poisoned".to_string())?;

    let hotkey_result = if paused {
        unregister_hotkeys(&app, &state)
    } else {
        register_hotkeys(&app, &state)
    };

    if let Err(error) = hotkey_result {
        {
            let mut runtime = state.runtime.lock().map_err(|_| "Runtime lock poisoned".to_string())?;
            *runtime = previous_runtime;
        }
        if paused {
            let _ = unregister_hotkeys(&app, &state);
        } else {
            let _ = register_hotkeys(&app, &state);
        }
        return Err(error);
    }

    emit_snapshot(&app, &state.runtime)?;
    Ok(())
}

fn resolve_config_path(path: &Path) -> Result<PathBuf, String> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    let cwd = std::env::current_dir().map_err(|e| format!("Failed to get current directory: {e}"))?;
    Ok(cwd.join(path))
}

fn configure_config_hot_reload(
    app: &AppHandle,
    state: &tauri::State<AppState>,
    path: Option<PathBuf>,
) -> Result<(), String> {
    {
        let mut active_path = state
            .active_config_path
            .lock()
            .map_err(|_| "Active config path lock poisoned".to_string())?;
        *active_path = path.clone();
    }

    let mut watcher_slot = state
        .config_watcher
        .lock()
        .map_err(|_| "Config watcher lock poisoned".to_string())?;
    *watcher_slot = None;

    let Some(path) = path else {
        return Ok(());
    };

    let app_handle = app.clone();
    let mut watcher = notify::recommended_watcher(move |result: notify::Result<Event>| match result {
        Ok(event) => {
            if !is_hot_reload_event(&event) {
                return;
            }
            if let Err(e) = reload_active_config(&app_handle) {
                emit_error(&app_handle, &e);
            }
        }
        Err(e) => {
            emit_error(&app_handle, &format!("Config watcher error: {e}"));
        }
    })
    .map_err(|e| format!("Failed to start config watcher: {e}"))?;

    watcher
        .watch(&path, RecursiveMode::NonRecursive)
        .map_err(|e| format!("Failed to watch config {}: {e}", path.display()))?;

    *watcher_slot = Some(watcher);
    Ok(())
}

fn is_hot_reload_event(event: &Event) -> bool {
    matches!(
        event.kind,
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Any
    )
}

fn reload_active_config(app: &AppHandle) -> Result<(), String> {
    let Some(state) = app.try_state::<AppState>() else {
        return Ok(());
    };

    let path = {
        let guard = state
            .active_config_path
            .lock()
            .map_err(|_| "Active config path lock poisoned".to_string())?;
        guard.clone()
    };

    let Some(path) = path else {
        return Ok(());
    };

    let config = load_config_from_path(&path)?;
    apply_config(app.clone(), &state, config)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState {
            runtime: Arc::new(Mutex::new(RuntimeState::new())),
            action_by_shortcut: Arc::new(Mutex::new(HashMap::new())),
            action_by_gamepad: Arc::new(Mutex::new(HashMap::new())),
            hotkeys_paused: Arc::new(Mutex::new(false)),
            active_config_path: Arc::new(Mutex::new(None)),
            config_watcher: Arc::new(Mutex::new(None)),
        })
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state() != ShortcutState::Pressed {
                        return;
                    }
                    handle_shortcut(app, shortcut.to_string());
                })
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            setup_menu(app)?;
            spawn_timer_thread(app.handle().clone());
            spawn_gamepad_thread(app.handle().clone());

            let maybe_default_path = std::env::current_dir().ok().and_then(|dir| {
                let local = dir.join(DEFAULT_CONFIG_NAME);
                if local.exists() {
                    return Some(local);
                }
                let parent = dir.parent().map(|p| p.join(DEFAULT_CONFIG_NAME));
                parent.filter(|p| p.exists())
            });
            if let Some(path) = maybe_default_path {
                let app_handle = app.handle().clone();
                let state: tauri::State<AppState> = app.state();
                if let Err(e) = load_config_from_file(app_handle.clone(), state, path.to_string_lossy().to_string()) {
                    emit_error(&app_handle, &e);
                }
            }

            Ok(())
        })
        .on_menu_event(|app, event| {
            if event.id().as_ref() == MENU_ITEM_LOAD_CONFIG {
                let selected = FileDialog::new()
                    .add_filter("TOML config", &["toml"])
                    .set_title("Load Scoreboard Config")
                    .pick_file();
                if let Some(path) = selected {
                    let state: tauri::State<AppState> = app.state();
                    if let Err(e) = load_config_from_file(app.clone(), state, path.to_string_lossy().to_string()) {
                        emit_error(app, &e);
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            load_config_from_file,
            load_config_from_text,
            update_label_text,
            pick_image_source,
            set_hotkeys_paused
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn setup_menu(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let load_config = MenuItem::with_id(app, MENU_ITEM_LOAD_CONFIG, "Load Config...", true, None::<&str>)?;
    let file_submenu = Submenu::with_items(app, "File", true, &[&load_config])?;
    let menu = Menu::with_items(app, &[&file_submenu])?;
    app.set_menu(menu)?;
    Ok(())
}

fn handle_shortcut(app: &AppHandle, shortcut: String) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let paused = match state.hotkeys_paused.lock() {
        Ok(g) => *g,
        Err(_) => return,
    };
    if paused {
        return;
    }

    let action = {
        let guard = match state.action_by_shortcut.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        guard.get(&shortcut).cloned()
    };

    let Some(action) = action else {
        return;
    };

    let changed = {
        let mut runtime = match state.runtime.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        runtime.apply_action(&action)
    };

    if changed {
        let _ = emit_snapshot(app, &state.runtime);
    }
}

fn handle_gamepad_button(app: &AppHandle, button: String) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let paused = match state.hotkeys_paused.lock() {
        Ok(g) => *g,
        Err(_) => return,
    };
    if paused {
        return;
    }

    let action = {
        let guard = match state.action_by_gamepad.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        guard.get(&button).cloned()
    };

    let Some(action) = action else {
        return;
    };

    let changed = {
        let mut runtime = match state.runtime.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        runtime.apply_action(&action)
    };

    if changed {
        let _ = emit_snapshot(app, &state.runtime);
    }
}

fn spawn_timer_thread(app: AppHandle) {
    thread::spawn(move || loop {
        // Keep updates frequent enough for tenths-of-a-second display modes.
        thread::sleep(Duration::from_millis(50));
        let Some(state) = app.try_state::<AppState>() else {
            continue;
        };

        let changed = {
            let mut runtime = match state.runtime.lock() {
                Ok(g) => g,
                Err(_) => continue,
            };
            runtime.tick_timers()
        };
        if changed {
            let _ = emit_snapshot(&app, &state.runtime);
        }
    });
}

fn spawn_gamepad_thread(app: AppHandle) {
    thread::spawn(move || {
        let mut gilrs = match Gilrs::new() {
            Ok(gilrs) => gilrs,
            Err(e) => {
                emit_error(&app, &format!("Gamepad input unavailable: {e}"));
                return;
            }
        };

        loop {
            while let Some(event) = gilrs.next_event() {
                if let EventType::ButtonPressed(button, _) = event.event {
                    if let Some(button_key) = map_gamepad_button(button) {
                        handle_gamepad_button(&app, button_key.to_string());
                    }
                }
            }

            thread::sleep(Duration::from_millis(8));
        }
    });
}

fn map_gamepad_button(button: Button) -> Option<&'static str> {
    match button {
        Button::South => Some("A"),
        Button::East => Some("B"),
        Button::West => Some("X"),
        Button::North => Some("Y"),
        Button::LeftTrigger => Some("LB"),
        Button::RightTrigger => Some("RB"),
        Button::LeftTrigger2 => Some("LT"),
        Button::RightTrigger2 => Some("RT"),
        Button::Select => Some("BACK"),
        Button::Start => Some("START"),
        Button::Mode => Some("GUIDE"),
        Button::LeftThumb => Some("L3"),
        Button::RightThumb => Some("R3"),
        Button::DPadUp => Some("DPAD_UP"),
        Button::DPadDown => Some("DPAD_DOWN"),
        Button::DPadLeft => Some("DPAD_LEFT"),
        Button::DPadRight => Some("DPAD_RIGHT"),
        _ => None,
    }
}

fn register_hotkeys(app: &AppHandle, state: &tauri::State<AppState>) -> Result<(), String> {
    unregister_hotkeys(app, state)?;

    let bindings = {
        let runtime = state.runtime.lock().map_err(|_| "Runtime lock poisoned".to_string())?;
        runtime.collect_hotkeys()
    };

    let mut keyboard_action_map = HashMap::new();
    let mut gamepad_action_map = HashMap::new();
    for binding in bindings {
        if let Some(button) = binding.shortcut.strip_prefix("Gamepad:") {
            gamepad_action_map.insert(button.to_string(), binding.action);
            continue;
        }

        let shortcut = Shortcut::from_str(&binding.shortcut)
            .map_err(|e| format!("Invalid shortcut '{}': {e}", binding.shortcut))?;
        let shortcut_key = shortcut.to_string();
        app.global_shortcut()
            .register(shortcut)
            .map_err(|e| format!("Failed to register '{}': {e}", binding.shortcut))?;
        keyboard_action_map.insert(shortcut_key, binding.action);
    }

    let mut keyboard_map = state
        .action_by_shortcut
        .lock()
        .map_err(|_| "Shortcut map lock poisoned".to_string())?;
    *keyboard_map = keyboard_action_map;

    let mut gamepad_map = state
        .action_by_gamepad
        .lock()
        .map_err(|_| "Gamepad map lock poisoned".to_string())?;
    *gamepad_map = gamepad_action_map;

    Ok(())
}

fn unregister_hotkeys(app: &AppHandle, state: &tauri::State<AppState>) -> Result<(), String> {
    app.global_shortcut()
        .unregister_all()
        .map_err(|e| format!("Failed to clear existing shortcuts: {e}"))?;

    let mut map = state
        .action_by_shortcut
        .lock()
        .map_err(|_| "Shortcut map lock poisoned".to_string())?;
    map.clear();

    let mut gamepad_map = state
        .action_by_gamepad
        .lock()
        .map_err(|_| "Gamepad map lock poisoned".to_string())?;
    gamepad_map.clear();

    Ok(())
}

fn emit_snapshot(app: &AppHandle, runtime: &Arc<Mutex<RuntimeState>>) -> Result<(), String> {
    let snapshot: UiSnapshot = {
        let runtime = runtime.lock().map_err(|_| "Runtime lock poisoned".to_string())?;
        runtime.snapshot()
    };
    app.emit(EVENT_STATE_UPDATED, snapshot)
        .map_err(|e| format!("Failed to emit state update: {e}"))
}

fn emit_error(app: &AppHandle, message: &str) {
    let _ = app.emit(EVENT_ERROR, message.to_string());
}
