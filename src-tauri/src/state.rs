use crate::config::{ComponentAlignment, ComponentKind, ScoreboardConfig, TimerRounding};
use serde::Serialize;
use std::collections::HashMap;
use std::time::Instant;

#[derive(Debug, Clone)]
pub enum Action {
    NumberIncrease { id: String },
    NumberDecrease { id: String },
    NumberReset { id: String },
    TimerStart { id: String },
    TimerStop { id: String },
    TimerReset { id: String },
    TimerIncrease { id: String },
    TimerDecrease { id: String },
    ImageToggleForward { id: String },
    ImageToggleBackward { id: String },
}

#[derive(Debug, Clone)]
pub struct HotkeyBinding {
    pub shortcut: String,
    pub action: Action,
}

#[derive(Debug, Clone, Serialize)]
pub struct UiSnapshot {
    pub background_color: String,
    pub components: Vec<UiComponent>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UiComponent {
    pub id: String,
    pub component_type: String,
    pub x: i32,
    pub y: i32,
    pub alignment: Option<String>,
    pub font_family: String,
    pub font_size: i32,
    pub font_color: String,
    pub text: Option<String>,
    pub source: Option<String>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub opacity: Option<f32>,
    pub editable: bool,
}

#[derive(Debug, Clone)]
pub struct RuntimeState {
    pub config: Option<ScoreboardConfig>,
    number_values: HashMap<String, i32>,
    timer_values: HashMap<String, TimerRuntime>,
    label_values: HashMap<String, String>,
    image_toggle_indices: HashMap<String, usize>,
}

#[derive(Debug, Clone)]
struct TimerRuntime {
    remaining_ms: i64,
    running: bool,
    last_tick: Option<Instant>,
}

impl RuntimeState {
    pub fn new() -> Self {
        Self {
            config: None,
            number_values: HashMap::new(),
            timer_values: HashMap::new(),
            label_values: HashMap::new(),
            image_toggle_indices: HashMap::new(),
        }
    }

    pub fn replace_config(&mut self, config: ScoreboardConfig) {
        self.number_values.clear();
        self.timer_values.clear();
        self.label_values.clear();
        self.image_toggle_indices.clear();

        for component in &config.components {
            match &component.kind {
                ComponentKind::Number { default, .. } => {
                    self.number_values.insert(component.id.clone(), *default);
                }
                ComponentKind::Timer { default_ms, .. } => {
                    self.timer_values.insert(
                        component.id.clone(),
                        TimerRuntime {
                            remaining_ms: *default_ms,
                            running: false,
                            last_tick: None,
                        },
                    );
                }
                ComponentKind::Label { default, .. } => {
                    self.label_values.insert(component.id.clone(), default.clone());
                }
                ComponentKind::Image { .. } => {}
                ComponentKind::ImageToggle { .. } => {
                    self.image_toggle_indices.insert(component.id.clone(), 0);
                }
            }
        }

        self.config = Some(config);
    }

    pub fn set_label_value(&mut self, id: &str, value: String) -> Result<bool, String> {
        if value.contains('\n') || value.contains('\r') {
            return Err("Label text must be a single-line string".to_string());
        }

        let Some(config) = &self.config else {
            return Err("No config loaded".to_string());
        };

        let Some(component) = config.components.iter().find(|c| c.id == id) else {
            return Err(format!("Unknown component '{id}'"));
        };

        let ComponentKind::Label { edit, .. } = &component.kind else {
            return Err(format!("Component '{id}' is not a label"));
        };

        if !edit {
            return Err(format!("Component '{id}' is not editable"));
        }

        let current = self.label_values.get(id).cloned().unwrap_or_default();
        if current == value {
            return Ok(false);
        }
        self.label_values.insert(id.to_string(), value);
        Ok(true)
    }

    pub fn collect_hotkeys(&self) -> Vec<HotkeyBinding> {
        let mut bindings = Vec::new();
        let Some(config) = &self.config else {
            return bindings;
        };

        for component in &config.components {
            match &component.kind {
                ComponentKind::Number {
                    keybind: Some(keybind),
                    ..
                } => {
                    if let Some(increase) = &keybind.increase {
                        bindings.push(HotkeyBinding {
                            shortcut: increase.to_shortcut(),
                            action: Action::NumberIncrease {
                                id: component.id.clone(),
                            },
                        });
                    }
                    if let Some(decrease) = &keybind.decrease {
                        bindings.push(HotkeyBinding {
                            shortcut: decrease.to_shortcut(),
                            action: Action::NumberDecrease {
                                id: component.id.clone(),
                            },
                        });
                    }
                    if let Some(reset) = &keybind.reset {
                        bindings.push(HotkeyBinding {
                            shortcut: reset.to_shortcut(),
                            action: Action::NumberReset {
                                id: component.id.clone(),
                            },
                        });
                    }
                }
                ComponentKind::Timer {
                    keybind: Some(keybind),
                    ..
                } => {
                    if let Some(start) = &keybind.start {
                        bindings.push(HotkeyBinding {
                            shortcut: start.to_shortcut(),
                            action: Action::TimerStart {
                                id: component.id.clone(),
                            },
                        });
                    }
                    if let Some(stop) = &keybind.stop {
                        bindings.push(HotkeyBinding {
                            shortcut: stop.to_shortcut(),
                            action: Action::TimerStop {
                                id: component.id.clone(),
                            },
                        });
                    }
                    if let Some(reset) = &keybind.reset {
                        bindings.push(HotkeyBinding {
                            shortcut: reset.to_shortcut(),
                            action: Action::TimerReset {
                                id: component.id.clone(),
                            },
                        });
                    }
                    if let Some(increase) = &keybind.increase {
                        bindings.push(HotkeyBinding {
                            shortcut: increase.to_shortcut(),
                            action: Action::TimerIncrease {
                                id: component.id.clone(),
                            },
                        });
                    }
                    if let Some(decrease) = &keybind.decrease {
                        bindings.push(HotkeyBinding {
                            shortcut: decrease.to_shortcut(),
                            action: Action::TimerDecrease {
                                id: component.id.clone(),
                            },
                        });
                    }
                }
                ComponentKind::ImageToggle {
                    keybind: Some(keybind),
                    ..
                } => {
                    if let Some(forward) = &keybind.forward {
                        bindings.push(HotkeyBinding {
                            shortcut: forward.to_shortcut(),
                            action: Action::ImageToggleForward {
                                id: component.id.clone(),
                            },
                        });
                    }
                    if let Some(backward) = &keybind.backward {
                        bindings.push(HotkeyBinding {
                            shortcut: backward.to_shortcut(),
                            action: Action::ImageToggleBackward {
                                id: component.id.clone(),
                            },
                        });
                    }
                }
                ComponentKind::Number { keybind: None, .. } => {}
                ComponentKind::Timer { keybind: None, .. } => {}
                ComponentKind::ImageToggle { keybind: None, .. } => {}
                ComponentKind::Label { .. } => {}
                ComponentKind::Image { .. } => {}
            }
        }

        bindings
    }

    pub fn apply_action(&mut self, action: &Action) -> bool {
        match action {
            Action::NumberIncrease { id } => {
                if let Some(value) = self.number_values.get_mut(id) {
                    *value += 1;
                    return true;
                }
            }
            Action::NumberDecrease { id } => {
                if let Some(value) = self.number_values.get_mut(id) {
                    *value = (*value - 1).max(0);
                    return true;
                }
            }
            Action::NumberReset { id } => {
                if let Some(config) = &self.config {
                    if let Some(default) = config.components.iter().find_map(|c| match &c.kind {
                        ComponentKind::Number { default, .. } if c.id == *id => Some(*default),
                        _ => None,
                    }) {
                        if let Some(value) = self.number_values.get_mut(id) {
                            *value = default;
                            return true;
                        }
                    }
                }
            }
            Action::TimerStart { id } => {
                if let Some(timer) = self.timer_values.get_mut(id) {
                    if timer.remaining_ms > 0 && !timer.running {
                        timer.running = true;
                        timer.last_tick = Some(Instant::now());
                        return true;
                    }
                }
            }
            Action::TimerStop { id } => {
                if let Some(timer) = self.timer_values.get_mut(id) {
                    if timer.running {
                        sync_timer(timer, Instant::now());
                        timer.running = false;
                        timer.last_tick = None;
                        return true;
                    }
                }
            }
            Action::TimerReset { id } => {
                if let Some(config) = &self.config {
                    if let Some(default) = config.components.iter().find_map(|c| match &c.kind {
                        ComponentKind::Timer { default_ms, .. } if c.id == *id => Some(*default_ms),
                        _ => None,
                    }) {
                        if let Some(timer) = self.timer_values.get_mut(id) {
                            let now = Instant::now();
                            if timer.running {
                                sync_timer(timer, now);
                            }
                            timer.remaining_ms = default;
                            if timer.running {
                                if timer.remaining_ms > 0 {
                                    timer.last_tick = Some(now);
                                } else {
                                    timer.running = false;
                                    timer.last_tick = None;
                                }
                            }
                            return true;
                        }
                    }
                }
            }
            Action::TimerIncrease { id } => {
                if let Some(timer) = self.timer_values.get_mut(id) {
                    let now = Instant::now();
                    if timer.running {
                        sync_timer(timer, now);
                    }
                    timer.remaining_ms += 1_000;
                    if timer.running {
                        timer.last_tick = Some(now);
                    }
                    return true;
                }
            }
            Action::TimerDecrease { id } => {
                if let Some(timer) = self.timer_values.get_mut(id) {
                    let now = Instant::now();
                    if timer.running {
                        sync_timer(timer, now);
                    }
                    timer.remaining_ms = (timer.remaining_ms - 1_000).max(0);
                    if timer.running {
                        if timer.remaining_ms > 0 {
                            timer.last_tick = Some(now);
                        } else {
                            timer.running = false;
                            timer.last_tick = None;
                        }
                    }
                    return true;
                }
            }
            Action::ImageToggleForward { id } => {
                if let Some(config) = &self.config {
                    if let Some(source_count) = config.components.iter().find_map(|c| match &c.kind {
                        ComponentKind::ImageToggle { sources, .. } if c.id == *id => Some(sources.len()),
                        _ => None,
                    }) {
                        if source_count > 0 {
                            let index = self.image_toggle_indices.entry(id.clone()).or_insert(0);
                            *index = (*index + 1) % source_count;
                            return true;
                        }
                    }
                }
            }
            Action::ImageToggleBackward { id } => {
                if let Some(config) = &self.config {
                    if let Some(source_count) = config.components.iter().find_map(|c| match &c.kind {
                        ComponentKind::ImageToggle { sources, .. } if c.id == *id => Some(sources.len()),
                        _ => None,
                    }) {
                        if source_count > 0 {
                            let index = self.image_toggle_indices.entry(id.clone()).or_insert(0);
                            *index = (*index + source_count - 1) % source_count;
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    pub fn tick_timers(&mut self) -> bool {
        let mut changed = false;
        let now = Instant::now();
        for timer in self.timer_values.values_mut() {
            if !timer.running {
                continue;
            }

            let last = timer.last_tick.unwrap_or(now);
            let elapsed_ms = now.duration_since(last).as_millis() as i64;
            if elapsed_ms <= 0 {
                continue;
            }

            timer.last_tick = Some(now);
            let new_value = (timer.remaining_ms - elapsed_ms).max(0);
            if new_value != timer.remaining_ms {
                timer.remaining_ms = new_value;
                changed = true;
            }
            if timer.remaining_ms == 0 && timer.running {
                timer.running = false;
                timer.last_tick = None;
            }
        }
        changed
    }

    pub fn snapshot(&self) -> UiSnapshot {
        let Some(config) = &self.config else {
            return UiSnapshot {
                background_color: "#000000".to_string(),
                components: Vec::new(),
            };
        };

        let components = config
            .components
            .iter()
            .map(|component| {
                let (component_type, text, source, width, height, opacity, editable) = match &component.kind {
                    ComponentKind::Number { .. } => (
                        "number".to_string(),
                        Some(
                            self.number_values
                                .get(&component.id)
                                .copied()
                                .unwrap_or_default()
                                .to_string(),
                        ),
                        None,
                        None,
                        None,
                        None,
                        false,
                    ),
                    ComponentKind::Timer { rounding, .. } => (
                        "timer".to_string(),
                        Some(format_ms(
                            self.timer_values
                                .get(&component.id)
                                .map(|t| t.remaining_ms)
                                .unwrap_or_default(),
                            rounding,
                        )),
                        None,
                        None,
                        None,
                        None,
                        false,
                    ),
                    ComponentKind::Label { edit, .. } => (
                        "label".to_string(),
                        Some(
                            self.label_values
                                .get(&component.id)
                                .cloned()
                                .unwrap_or_default(),
                        ),
                        None,
                        None,
                        None,
                        None,
                        *edit,
                    ),
                    ComponentKind::Image {
                        source,
                        width,
                        height,
                        opacity,
                    } => (
                        "image".to_string(),
                        None,
                        Some(source.clone()),
                        Some(*width),
                        Some(*height),
                        Some(*opacity),
                        false,
                    ),
                    ComponentKind::ImageToggle {
                        sources,
                        width,
                        height,
                        opacity,
                        ..
                    } => {
                        let index = self
                            .image_toggle_indices
                            .get(&component.id)
                            .copied()
                            .unwrap_or(0)
                            % sources.len();
                        (
                            "image-toggle".to_string(),
                            None,
                            Some(sources[index].clone()),
                            Some(*width),
                            Some(*height),
                            Some(*opacity),
                            false,
                        )
                    }
                };

                UiComponent {
                    id: component.id.clone(),
                    component_type,
                    x: component.position.x,
                    y: component.position.y,
                    alignment: component.alignment.as_ref().map(|alignment| match alignment {
                        ComponentAlignment::Center => "center".to_string(),
                    }),
                    font_family: component.font.family.clone(),
                    font_size: component.font.size,
                    font_color: component.font.color.clone(),
                    text,
                    source,
                    width,
                    height,
                    opacity,
                    editable,
                }
            })
            .collect();

        UiSnapshot {
            background_color: config.global.background_color.clone(),
            components,
        }
    }
}

fn format_ms(ms: i64, rounding: &TimerRounding) -> String {
    match rounding {
        TimerRounding::Standard => format_ms_standard(ms),
        TimerRounding::Basketball => format_ms_basketball(ms),
    }
}

fn sync_timer(timer: &mut TimerRuntime, now: Instant) {
    if !timer.running {
        return;
    }

    let last = timer.last_tick.unwrap_or(now);
    let elapsed_ms = now.duration_since(last).as_millis() as i64;
    if elapsed_ms > 0 {
        timer.remaining_ms = (timer.remaining_ms - elapsed_ms).max(0);
    }
    if timer.remaining_ms > 0 {
        timer.last_tick = Some(now);
    } else {
        timer.running = false;
        timer.last_tick = None;
    }
}

fn format_ms_standard(ms: i64) -> String {
    let total_seconds = ms.max(0) / 1000;
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    if hours > 0 {
        format!("{hours:02}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes:02}:{seconds:02}")
    }
}

fn format_ms_basketball(ms: i64) -> String {
    let clamped_ms = ms.max(0);

    if clamped_ms < 60_000 {
        let tenths_total = ((clamped_ms + 50) / 100) as i64;
        let seconds = tenths_total / 10;
        let tenths = tenths_total % 10;
        return format!("{seconds}.{tenths}");
    }

    let rounded_seconds = ((clamped_ms + 500) / 1000) as i64;
    let hours = rounded_seconds / 3600;
    let minutes = (rounded_seconds % 3600) / 60;
    let seconds = rounded_seconds % 60;
    if hours > 0 {
        format!("{hours}:{minutes}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}
