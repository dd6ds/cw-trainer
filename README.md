# cw-trainer

A command-line Morse code (CW) training tool in the style of [lcwo.net](https://lcwo.net).  
It generates random sentences, plays them as audio CW, and scores your copy.

---

## Features

- Generates random sentences in **English**, **German**, or both
- Plays CW audio directly via `aplay` (no external dependencies)
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

Requires [Rust](https://rustup.rs/) and `aplay` (part of `alsa-utils` on most Linux distros).

```bash
cargo build --release
./target/release/cw-trainer
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
| `--lang <LANG>` | `en` | Language: `en`, `de`, or `both` |
| `--words <N>` | `5` | Number of words per sentence |

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

- **Linux / macOS**: `aplay` must be installed (`sudo apt install alsa-utils` on Debian/Ubuntu)
- **Windows**: audio playback via `aplay` is not natively available — WSL or a ported build is recommended

---

## License

MIT
