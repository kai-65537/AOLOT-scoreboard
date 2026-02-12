use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub const CANVAS_WIDTH: i32 = 640;
pub const CANVAS_HEIGHT: i32 = 480;

#[derive(Debug, Clone, Serialize)]
pub struct ScoreboardConfig {
    pub global: GlobalSettings,
    pub components: Vec<ComponentConfig>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GlobalSettings {
    pub background_color: String,
    pub font: Font,
}

#[derive(Debug, Clone, Serialize)]
pub struct ComponentConfig {
    pub id: String,
    pub position: Position,
    pub font: Font,
    pub kind: ComponentKind,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ComponentKind {
    Number {
        default: i32,
        keybind: NumberKeybind,
    },
    Timer {
        default_ms: i64,
        keybind: TimerKeybind,
        rounding: TimerRounding,
    },
    Label {
        default: String,
        edit: bool,
    },
    Image {
        source: String,
        width: i32,
        height: i32,
        opacity: f32,
    },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TimerRounding {
    Standard,
    Basketball,
}

#[derive(Debug, Clone, Serialize)]
pub struct NumberKeybind {
    pub increase: KeybindSpec,
    pub decrease: KeybindSpec,
    pub reset: KeybindSpec,
}

#[derive(Debug, Clone, Serialize)]
pub struct TimerKeybind {
    pub start: KeybindSpec,
    pub stop: KeybindSpec,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeybindSpec {
    pub key: String,
    #[serde(default)]
    pub ctrl: bool,
    #[serde(default)]
    pub alt: bool,
    #[serde(default)]
    pub shift: bool,
    #[serde(default)]
    pub win: bool,
}

impl KeybindSpec {
    pub fn to_shortcut(&self) -> String {
        let mut parts: Vec<&str> = Vec::new();
        if self.ctrl {
            parts.push("Ctrl");
        }
        if self.alt {
            parts.push("Alt");
        }
        if self.shift {
            parts.push("Shift");
        }
        if self.win {
            parts.push("Super");
        }
        parts.push(self.key.trim());
        parts.join("+")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Font {
    pub family: String,
    pub size: i32,
    pub color: String,
}

#[derive(Debug, Clone, Deserialize)]
struct FontOverride {
    family: Option<String>,
    size: Option<i32>,
    color: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct RawGlobal {
    background_color: Option<String>,
    font: Option<FontOverride>,
}

#[derive(Debug, Clone, Deserialize)]
struct RawComponent {
    #[serde(rename = "type")]
    component_type: toml::Value,
    default: Option<toml::Value>,
    position: Position,
    font: Option<FontOverride>,
    keybind: Option<BTreeMap<String, KeybindSpec>>,
    source: Option<String>,
    size: Option<ImageSize>,
    opacity: Option<f32>,
    rounding: Option<String>,
    edit: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
struct ImageSize {
    width: i32,
    height: i32,
}

pub fn load_config_from_path(path: &Path) -> Result<ScoreboardConfig, String> {
    let content = fs::read_to_string(path)
        .map_err(|e| format!("Failed reading config {}: {e}", path.display()))?;
    let base_dir = path.parent().unwrap_or_else(|| Path::new("."));
    load_config_from_str_with_base(&content, base_dir)
}

pub fn load_config_from_str(content: &str) -> Result<ScoreboardConfig, String> {
    let base = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    load_config_from_str_with_base(content, &base)
}

fn load_config_from_str_with_base(content: &str, base_dir: &Path) -> Result<ScoreboardConfig, String> {
    let root: toml::Value = toml::from_str(content).map_err(|e| format!("TOML parse error: {e}"))?;
    let table = root
        .as_table()
        .ok_or_else(|| "Config root must be a TOML table".to_string())?;

    let global = parse_global_settings(table.get("global"))?;

    let mut components: Vec<ComponentConfig> = Vec::new();
    for (id, value) in table {
        if id == "global" {
            continue;
        }

        let raw: RawComponent = value
            .clone()
            .try_into()
            .map_err(|e| format!("Invalid component '{id}': {e}"))?;
        let font = resolve_font(&global.font, raw.font.as_ref())?;
        validate_id(id)?;
        validate_position(id, &raw.position)?;
        validate_font(id, &font)?;

        let (component_type, type_rounding) = parse_component_type(id, &raw.component_type)?;
        let kind = match component_type.as_str() {
            "number" => {
                if raw.edit.is_some() {
                    return Err(format!("'{id}' edit is only supported for label components"));
                }
                let default = raw
                    .default
                    .as_ref()
                    .and_then(|v| v.as_integer())
                    .ok_or_else(|| format!("'{id}' default must be an integer"))?
                    as i32;

                let binds = raw
                    .keybind
                    .ok_or_else(|| format!("'{id}' number requires keybind section"))?;

                ComponentKind::Number {
                    default,
                    keybind: NumberKeybind {
                        increase: parse_keybind(id, &binds, "increase")?,
                        decrease: parse_keybind(id, &binds, "decrease")?,
                        reset: parse_keybind(id, &binds, "reset")?,
                    },
                }
            }
            "timer" => {
                if raw.edit.is_some() {
                    return Err(format!("'{id}' edit is only supported for label components"));
                }
                let raw_default = raw
                    .default
                    .as_ref()
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| format!("'{id}' default must be a timer string HH:MM:SS"))?;

                let binds = raw
                    .keybind
                    .ok_or_else(|| format!("'{id}' timer requires keybind section"))?;

                let rounding = parse_timer_rounding(id, type_rounding.as_deref(), raw.rounding.as_deref())?;
                ComponentKind::Timer {
                    default_ms: parse_timer_default(raw_default)?,
                    keybind: TimerKeybind {
                        start: parse_keybind(id, &binds, "start")?,
                        stop: parse_keybind(id, &binds, "stop")?,
                    },
                    rounding,
                }
            }
            "label" => {
                let default = raw
                    .default
                    .as_ref()
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| format!("'{id}' default must be a string"))?
                    .to_string();
                ComponentKind::Label {
                    default,
                    edit: raw.edit.unwrap_or(false),
                }
            }
            "image" => {
                if raw.edit.is_some() {
                    return Err(format!("'{id}' edit is only supported for label components"));
                }
                let source = raw
                    .source
                    .as_ref()
                    .ok_or_else(|| format!("'{id}' image requires source"))?;
                let size = raw
                    .size
                    .as_ref()
                    .ok_or_else(|| format!("'{id}' image requires size.width and size.height"))?;
                if size.width <= 0 || size.height <= 0 {
                    return Err(format!("'{id}' image size must be > 0"));
                }
                let opacity = raw.opacity.unwrap_or(1.0);
                if !(0.0..=1.0).contains(&opacity) {
                    return Err(format!("'{id}' opacity must be between 0.0 and 1.0"));
                }

                let source_path = resolve_image_source(base_dir, source);
                ComponentKind::Image {
                    source: source_path,
                    width: size.width,
                    height: size.height,
                    opacity,
                }
            }
            other => return Err(format!("'{id}' has unsupported type '{other}'")),
        };

        components.push(ComponentConfig {
            id: id.to_string(),
            position: raw.position,
            font,
            kind,
        });
    }

    components.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(ScoreboardConfig { global, components })
}

fn parse_component_type(id: &str, raw_type: &toml::Value) -> Result<(String, Option<String>), String> {
    if let Some(component_type) = raw_type.as_str() {
        return Ok((component_type.to_string(), None));
    }

    let table = raw_type
        .as_table()
        .ok_or_else(|| format!("'{id}' type must be a string or table"))?;

    let component_type = table
        .get("name")
        .or_else(|| table.get("kind"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("'{id}' type table requires 'name' or 'kind' as a string"))?;

    let rounding = table
        .get("rounding")
        .and_then(|v| v.as_str())
        .map(|v| v.to_string());

    Ok((component_type.to_string(), rounding))
}

fn parse_timer_rounding(
    id: &str,
    type_rounding: Option<&str>,
    component_rounding: Option<&str>,
) -> Result<TimerRounding, String> {
    let rounding = type_rounding.or(component_rounding).unwrap_or("standard");
    match rounding.to_ascii_lowercase().as_str() {
        "standard" => Ok(TimerRounding::Standard),
        "basketball" => Ok(TimerRounding::Basketball),
        other => Err(format!(
            "'{id}' has unsupported timer rounding '{other}' (expected 'standard' or 'basketball')"
        )),
    }
}

fn parse_global_settings(raw_global: Option<&toml::Value>) -> Result<GlobalSettings, String> {
    let fallback_font = Font {
        family: "Segoe UI".to_string(),
        size: 28,
        color: "#FFFFFF".to_string(),
    };
    let fallback_bg = "#000000".to_string();

    let parsed = match raw_global {
        Some(value) => value
            .clone()
            .try_into::<RawGlobal>()
            .map_err(|e| format!("Invalid [global] section: {e}"))?,
        None => RawGlobal {
            background_color: None,
            font: None,
        },
    };

    let font = resolve_font(&fallback_font, parsed.font.as_ref())?;
    validate_font("global.font", &font)?;

    let background_color = parsed.background_color.unwrap_or(fallback_bg);
    validate_color("global.background_color", &background_color)?;

    Ok(GlobalSettings {
        background_color,
        font,
    })
}

fn resolve_font(base: &Font, override_font: Option<&FontOverride>) -> Result<Font, String> {
    let family = override_font
        .and_then(|f| f.family.clone())
        .unwrap_or_else(|| base.family.clone());
    let size = override_font.and_then(|f| f.size).unwrap_or(base.size);
    let color = override_font
        .and_then(|f| f.color.clone())
        .unwrap_or_else(|| base.color.clone());

    Ok(Font { family, size, color })
}

fn parse_keybind(
    id: &str,
    binds: &BTreeMap<String, KeybindSpec>,
    key: &str,
) -> Result<KeybindSpec, String> {
    let spec = binds
        .get(key)
        .ok_or_else(|| format!("'{id}' keybind.{key} is required"))?;
    if spec.key.trim().is_empty() {
        return Err(format!("'{id}' keybind.{key}.key cannot be empty"));
    }
    Ok(spec.clone())
}

fn resolve_image_source(base_dir: &Path, source: &str) -> String {
    let p = PathBuf::from(source);
    if p.is_absolute() {
        return p.to_string_lossy().to_string();
    }
    base_dir.join(p).to_string_lossy().to_string()
}

fn validate_id(id: &str) -> Result<(), String> {
    if id.trim().is_empty() {
        return Err("Component id cannot be empty".to_string());
    }
    Ok(())
}

fn validate_position(id: &str, p: &Position) -> Result<(), String> {
    if p.x < 0 || p.x >= CANVAS_WIDTH || p.y < 0 || p.y >= CANVAS_HEIGHT {
        return Err(format!(
            "'{id}' position ({}, {}) is outside {}x{}",
            p.x, p.y, CANVAS_WIDTH, CANVAS_HEIGHT
        ));
    }
    Ok(())
}

fn validate_font(id: &str, font: &Font) -> Result<(), String> {
    if font.family.trim().is_empty() {
        return Err(format!("'{id}' font.family cannot be empty"));
    }
    if font.size <= 0 {
        return Err(format!("'{id}' font.size must be > 0"));
    }
    validate_color(&format!("{id}.font.color"), &font.color)?;
    Ok(())
}

fn validate_color(name: &str, color: &str) -> Result<(), String> {
    let trimmed = color.trim();
    if !(trimmed.len() == 7 && trimmed.starts_with('#')) {
        return Err(format!("'{name}' must be #RRGGBB"));
    }
    if !trimmed.chars().skip(1).all(|c| c.is_ascii_hexdigit()) {
        return Err(format!("'{name}' must be #RRGGBB"));
    }
    Ok(())
}

fn parse_timer_default(value: &str) -> Result<i64, String> {
    let parts: Vec<&str> = value.split(':').collect();
    if parts.len() != 3 {
        return Err(format!("Timer default '{value}' must be HH:MM:SS"));
    }
    let h: i64 = parts[0]
        .parse()
        .map_err(|_| format!("Timer default '{value}' has invalid hours"))?;
    let m: i64 = parts[1]
        .parse()
        .map_err(|_| format!("Timer default '{value}' has invalid minutes"))?;
    let s: i64 = parts[2]
        .parse()
        .map_err(|_| format!("Timer default '{value}' has invalid seconds"))?;
    if !(0..60).contains(&m) || !(0..60).contains(&s) || h < 0 {
        return Err(format!("Timer default '{value}' must be HH:MM:SS"));
    }
    Ok(((h * 3600) + (m * 60) + s) * 1000)
}
