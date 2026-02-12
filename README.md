# AOL Scoreboard (Windows, Tauri + Rust)

Custom livestream scoreboard app with:

- fixed window size `640x480`
- `File -> Load Config...` menu action
- TOML-driven layout/behavior
- global hotkeys that work when the app is out of focus

## Config

Use `basketball.toml` as the minimal template.

Global defaults:

- `[global].background_color`
- `[global].font.family`
- `[global].font.size`
- `[global].font.color`

Supported component `type` values:

- `number` with `keybind.increase`, `keybind.decrease`, `keybind.reset`
- `timer` with `keybind.start`, `keybind.stop` and `default = "HH:MM:SS"`
- `label` with optional `edit = true` for runtime text editing
- `image` with `source`, `size.width`, `size.height`, optional `opacity`

`keybind` sections for `number` and `timer` are optional. If omitted, that component is immutable at runtime.

Timer rounding modes:

- `rounding = "standard"` (default): `MM:SS` or `HH:MM:SS`, zero-padded
- `rounding = "basketball"`: rounded to whole seconds at `>= 1:00` and shown as `M:SS` (seconds zero-padded), then `s.d` below 1 minute with no leading zeros

You can set timer rounding either as `rounding = "basketball"` on the timer component, or with a type table:

```toml
type.kind = "timer"
type.rounding = "basketball"
```

Editable labels (`edit = true`) can be clicked while the app is running to open an input dialog and update label text in memory only (the config file is not modified). While this dialog is open, global scoreboard hotkeys are paused and restored when it closes.

Keybind format is Windows-oriented and structured:

```toml
keybind.increase.key = "Q"
keybind.increase.ctrl = true
keybind.increase.alt = false
keybind.increase.shift = false
keybind.increase.win = false
```

Only `key` is required. Modifier flags default to `false`.

## Run

```powershell
cd src-tauri
cargo tauri dev
```

On startup, if `basketball.toml` exists in the current working directory, it is loaded automatically.
Relative file paths inside a loaded config (for example image `source`) are resolved relative to that config file's directory.

## Build Portable EXE

```powershell
cd src-tauri
cargo tauri build --bundles none
```

The portable executable is produced under `src-tauri/target/release/`.
