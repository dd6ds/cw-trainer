# cw-trainer

A command-line Morse code (CW) training tool in the style of [lcwo.net](https://lcwo.net).  
It generates random sentences, plays them as audio CW, and scores your copy.

---

## Features

- Generates random sentences in **English**, **German**, or both
- **Callsign mode** — practice amateur-radio callsigns from a user-editable list
- Cross-platform audio playback — works on Linux, macOS, and Windows out of the box
- Farnsworth timing — character speed and effective speed are independent
- Configurable tone frequency, words per sentence, and WPM
- Scores each attempt by **word accuracy** and **character accuracy**
- Tracks session average across rounds

---

## Quick Start

### Pre-built binaries

Download the binary for your platform from the `releases/` folder:

| Platform | Binary |
|---|---|
| Linux x86-64 | `releases/cw-trainer-linux-amd64` |
| Linux i686 | `releases/cw-trainer-linux-i686` |
| macOS Apple Silicon | `releases/cw-trainer-macos-aarch64` |
| macOS Intel | `releases/cw-trainer-macos-x86_64` |
| Windows x86-64 | `releases/cw-trainer-windows-amd64.exe` |

```bash
chmod +x releases/cw-trainer-linux-amd64
./releases/cw-trainer-linux-amd64
```

### Build from source

Requires [Rust](https://rustup.rs/). On Linux you also need `aplay` (part of
`alsa-utils` on most distros) or `paplay` (PulseAudio). macOS and Windows have
the necessary audio tooling built in.

```bash
cargo build --release
./target/release/cw-trainer
```

### Cross-compile release binaries

`build-releases.sh` cross-compiles for all supported platforms (Linux, macOS,
Windows — 32- and 64-bit) and drops the binaries into `releases/`. It
auto-installs `cross`, `cargo-zigbuild`, and `zig` if they're missing.

Requirements on the build host:

- Linux (WSL works)
- [Rust / rustup](https://rustup.rs/)
- Docker daemon running (used by `cross` for the Windows and Linux targets)

```bash
./build-releases.sh
```

---

## Usage

```
cw-trainer [OPTIONS]
```

### Options

| Option | Default | Description |
|---|---|---|
| `--wpm <WPM>` | `20` | Character speed in WPM (5–150) |
| `--eff <EFF>` | `15` | Effective (Farnsworth) speed — controls letter/word spacing |
| `--tone <HZ>` | `600` | Tone frequency in Hz (250–990) |
| `--lang <LANG>` | `en` | Language: `en`, `de`, or `both` (ignored in `--callsigns` mode) |
| `--words <N>` | `5` | Number of words (or callsigns) per round |
| `--callsigns` | off | Practice amateur-radio callsigns instead of random sentences |
| `--callsigns-file <PATH>` | auto | Path to the callsigns list; default is `callsigns.txt` in CWD, then next to the binary. Passing this flag also implies `--callsigns`. |
| `--mycall <CALL>` | — | Your own callsign. Mixed into each round (~1 slot in 3) so you train instant recognition of your own call. Implies `--callsigns`. |

### Examples

```bash
# Default settings (20 WPM, 5 words, English)
./cw-trainer

# Slow Farnsworth practice: fast characters, slow spacing
./cw-trainer --wpm 20 --eff 8

# 2-word German sentences at 600 Hz
./cw-trainer --lang de --words 2 --tone 600

# Fast session, English and German mixed
./cw-trainer --wpm 30 --eff 25 --lang both --words 6

# Callsign practice — 3 random callsigns per round at 25 WPM, 6 WPM effective
./cw-trainer --callsigns --words 3 --wpm 25 --eff 6

# Use your own callsigns list
./cw-trainer --callsigns --callsigns-file ~/lists/dx-contest.txt

# Include your own callsign in the mix (implies --callsigns)
./cw-trainer --mycall DL1XYZ --wpm 25 --eff 6
```

### Callsigns file format

`callsigns.txt` is a plain-text file containing double-quoted callsigns
separated by commas. Whitespace, blank lines, and lines starting with `#` or
`//` are ignored. Callsigns are case-insensitive.

```
"K1ABC","K2BCD","K3CDE"
"DL1ABC","DL2BCD"
# Contest list
"VE1ABC","G4DEF"
```

---

## Session Flow

1. A random sentence is generated and played as CW.
2. You are prompted to enter a command:
   - `r` — repeat the audio
   - `s` — reveal the text
   - `Enter` — proceed to enter your copy
3. Type what you heard and press Enter.
4. Your copy is scored and compared word-by-word against the original.
5. Choose `Y` to continue to the next sentence or `n` to end the session.

At the end, a final average word accuracy is shown across all rounds.

---

## Scoring

- **Word accuracy** — percentage of words copied correctly (exact match, case-insensitive)
- **Char accuracy** — percentage of characters matched position-by-position against the original

Missed or wrong words are shown in `[brackets]` in the diff; missing words are shown as `(word?)`.

---

## Requirements

Audio playback is handled per-platform, with no extra setup on macOS or Windows:

- **Linux**: `aplay` (ALSA) or `paplay` (PulseAudio). On Debian/Ubuntu:
  `sudo apt install alsa-utils` or `pulseaudio-utils`.
- **macOS**: uses the built-in `afplay` — nothing to install.
- **Windows**: uses PowerShell's `System.Media.SoundPlayer` — nothing to install.

---

## License

MIT
