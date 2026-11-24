<div align="center">
  <h1>termcpy</h1>
  <p>
    Mirror and control Android devices in your terminal
  </p>

  <p>
    <a href="https://github.com/cesarferreira/termcpy/actions/workflows/rust-tests.yml"><img alt="CI" src="https://github.com/cesarferreira/termcpy/actions/workflows/rust-tests.yml/badge.svg"></a>
    <a href="https://crates.io/crates/termcpy"><img alt="Crates.io" src="https://img.shields.io/crates/v/termcpy"></a>
    <img alt="Rust" src="https://img.shields.io/badge/rust-1.85%2B-orange">
    <img alt="Edition" src="https://img.shields.io/badge/edition-2024-blue">
    <img alt="scrcpy" src="https://img.shields.io/badge/scrcpy-4.x%20%7C%205.x-3ddc84">
    <img alt="License" src="https://img.shields.io/badge/license-MIT-green">
  </p>
</div>

## Install

From [crates.io](https://crates.io/crates/termcpy):

```bash
cargo install termcpy --locked
```

Prebuilt binaries for macOS, Linux, and Windows are on the [releases page](https://github.com/cesarferreira/termcpy/releases).

Install the runtime tools. On macOS:

```bash
brew install scrcpy ffmpeg
```

## Quick start

```bash
# Mirror the only connected Android device
termcpy

# Choose a device when several are connected
termcpy --list-devices
termcpy --serial emulator-5554

# Coloured blocks, literal ASCII, or a smaller capture at 15 FPS
termcpy --render blocks
termcpy --render ascii --no-color
termcpy --max-size 640 --max-fps 15

# Try the renderer without a phone
termcpy --demo

# Print one frame without entering the interactive terminal
termcpy --snapshot --render ascii --no-color --cols 120 --rows 50
```

## Options

| Flag | Description |
| --- | --- |
| `-s, --serial <SERIAL>` | Select an Android device by its adb serial |
| `--list-devices` | Show connected adb devices and exit |
| `--demo` | Render a synthetic phone screen without external tools |
| `--snapshot` | Print one character frame and exit (works without a TTY) |
| `--render <MODE>` | `graphics`, `blocks`, or `ascii` (default: graphics when supported, otherwise blocks) |
| `--no-color` | Monochrome rendering (ASCII output contains no ANSI colours) |
| `--invert` | Reverse the brightness mapping |
| `--ramp <CHARS>` | ASCII characters from dark to bright (default `" .:-=+*#%@"`) |
| `--char-aspect <RATIO>` | Character width divided by height (default `0.5`) |
| `-m, --max-size <PX>` | Longest captured edge in pixels (default 2048, or 1280 over SSH) |
| `--max-fps <FPS>` | Capture and display frame rate (default 30, or 15 over SSH) |
| `--cols <N>`, `--rows <N>` | Snapshot canvas size in characters (default 120×50) |
| `--server <PATH>` | Path to `scrcpy-server` |
| `--server-version <VER>` | Exact server version |
| `--adb`, `--scrcpy`, `--ffmpeg` | Executable paths (default: found on `PATH`) |

## Controls

| Input | Action |
| --- | --- |
| Left click | Tap at that location |
| Left drag | Swipe; holding the mouse down also supports long presses |
| Mouse wheel | Scroll |
| Right click / `b` / Escape | Back |
| Middle click / `h` / Home | Home |
| `r` | Recent apps |
| `p` | Power button |
| Arrow keys | Android directional keys |
| Enter / Backspace | Android Enter / Delete |
| `t` | Text mode: type or paste, Enter sends, Escape cancels |
| `a` | Cycle graphics (when enabled), ASCII, and blocks |
| `c` | Toggle colour (grayscale when disabled) |
| `i` | Invert brightness |
| `q` / Ctrl+C | Quit |

Clicks map to the centre of each character cell. The view preserves the phone's proportions and follows terminal resizes and device rotation.

Text mode uses scrcpy's text injection. Android input-method limits apply: arbitrary Unicode, emoji, and composing input are not guaranteed.

## Renderers

- **graphics** (default): full pixels through the [Kitty graphics protocol](https://sw.kovidgoyal.net/kitty/graphics-protocol/), so phone text stays readable. Each frame is scaled to the pixels the terminal displays and sent as zlib-compressed RGB. A frame identical to the one on screen is not sent again.
- **blocks**: coloured half-block characters, two pixels per cell.
- **ascii**: literal ASCII. It suppresses dark background speckle and lifts dim foreground colours.

termcpy probes the terminal before connecting. If the terminal does not confirm Kitty graphics support, it uses blocks. With an explicit `--render graphics`, a failed probe is an error. `--snapshot` uses blocks unless you choose `--render ascii`. Text modes redraw only the lines that changed.

> [!TIP]
> Maximize the window for more displayed pixels, or use `--max-size 4096` for more source detail. In text modes, a smaller font gives more detail.

ASCII and block modes cannot keep small UI text readable; graphics shows the actual pixels. Audio, clipboard sync, and multitouch are not supported yet.

## Over SSH

Every frame crosses the network. When `SSH_CONNECTION` or `SSH_TTY` is set, the defaults change to a 1280-pixel capture at 15 FPS. Locally they are 2048 pixels at 30 FPS. `--max-size` and `--max-fps` override either profile.

Force the choice where SSH is hidden, for example inside a multiplexer:

```bash
TERMCPY_REMOTE=1 termcpy
```

The header shows two rates: `phone` is frames decoded from the device, and `sent` is frames written to the terminal.

## Server discovery

termcpy runs `scrcpy --version` and finds the matching server beside the scrcpy executable or in common Homebrew and Linux locations. `SCRCPY_SERVER_PATH` is also supported. For a custom install:

```bash
termcpy --server /path/to/scrcpy-server --server-version 5.0
```

The server version must match the server file exactly. ADB-over-network devices work when `adb devices` already lists them.

## How it works

```text
Android screen → scrcpy H264 stream → FFmpeg → RGB frames → terminal (Kitty graphics, blocks or ASCII)
Android touch  ← scrcpy control socket ← terminal mouse coordinates
```

[scrcpy](https://github.com/Genymobile/scrcpy) supplies screen capture and Android input. A rotation restarts the decoder, so touch coordinates keep matching the screen. The ASCII renderer uses the brightness mapping, per-cell colour, and character-aspect approach of [ascii-cam](https://github.com/cesarferreira/ascii-cam).

Each session starts its own server and ADB tunnel. Normal exit, Ctrl+C, SIGTERM, and (on Unix) SIGHUP restore the terminal and remove them.

## Requirements

- `adb`, `scrcpy` **4.x or 5.x**, and `ffmpeg` on your `PATH` (tested with scrcpy 5.0)
- An Android device or emulator with USB debugging enabled and this computer authorized
- For graphics: a Unix terminal with Kitty graphics support, such as kitty, Ghostty, or WezTerm
- For text modes: mouse reporting and truecolour support
- Rust **1.85+** to build from source

## Troubleshooting

- **Blocks instead of graphics:** the terminal did not confirm Kitty graphics support. Multiplexers such as tmux need graphics passthrough. Run `termcpy --render graphics` to see the probe error.
- **Laggy picture:** check the header rates. A low `phone` rate means the device or emulator is slow. If `sent` keeps up but the picture still lags, the terminal or the link to it is the bottleneck. Try `--max-size 800 --max-fps 10`.
- **No authorized Android device:** check `adb devices` and approve USB debugging on the device.
- **Multiple devices connected:** pick one with `--serial`.
- **scrcpy server not found:** pass `--server` and `--server-version`, or set `SCRCPY_SERVER_PATH`.

## License

MIT. See `LICENSE`.
