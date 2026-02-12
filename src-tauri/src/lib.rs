mod config;
mod state;

use crate::config::{load_config_from_path, load_config_from_str};
use crate::state::{Action, RuntimeState, UiSnapshot};
use std::collections::HashMap;
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
}

#[tauri::command]
fn load_config_from_file(app: AppHandle, state: tauri::State<AppState>, path: String) -> Result<(), String> {
    let path = PathBuf::from(path);
    let config = load_config_from_path(&path)?;
    apply_config(app, state, config)
}

#[tauri::command]
fn load_config_from_text(
    app: AppHandle,
    state: tauri::State<AppState>,
    content: String,
) -> Result<(), String> {
    let config = load_config_from_str(&content)?;
    apply_config(app, state, config)
}

fn apply_config(app: AppHandle, state: tauri::State<AppState>, config: config::ScoreboardConfig) -> Result<(), String> {
    {
        let mut runtime = state.runtime.lock().map_err(|_| "Runtime lock poisoned".to_string())?;
        runtime.replace_config(config);
    }

    register_hotkeys(&app, &state)?;
    emit_snapshot(&app, &state.runtime)?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState {
            runtime: Arc::new(Mutex::new(RuntimeState::new())),
            action_by_shortcut: Arc::new(Mutex::new(HashMap::new())),
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
            load_config_from_text
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

fn spawn_timer_thread(app: AppHandle) {
    thread::spawn(move || loop {
        thread::sleep(Duration::from_millis(200));
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

fn register_hotkeys(app: &AppHandle, state: &tauri::State<AppState>) -> Result<(), String> {
    app.global_shortcut()
        .unregister_all()
        .map_err(|e| format!("Failed to clear existing shortcuts: {e}"))?;

    let bindings = {
        let runtime = state.runtime.lock().map_err(|_| "Runtime lock poisoned".to_string())?;
        runtime.collect_hotkeys()
    };

    let mut action_map = HashMap::new();
    for binding in bindings {
        let shortcut = Shortcut::from_str(&binding.shortcut)
            .map_err(|e| format!("Invalid shortcut '{}': {e}", binding.shortcut))?;
        let shortcut_key = shortcut.to_string();
        app.global_shortcut()
            .register(shortcut)
            .map_err(|e| format!("Failed to register '{}': {e}", binding.shortcut))?;
        action_map.insert(shortcut_key, binding.action);
    }

    let mut map = state
        .action_by_shortcut
        .lock()
        .map_err(|_| "Shortcut map lock poisoned".to_string())?;
    *map = action_map;
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
