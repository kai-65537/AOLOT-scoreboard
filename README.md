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
- `label`
- `image` with `source`, `size.width`, `size.height`, optional `opacity`

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

## Build Portable EXE

```powershell
cd src-tauri
cargo tauri build --bundles none
```

The portable executable is produced under `src-tauri/target/release/`.
