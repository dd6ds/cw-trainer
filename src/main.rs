use clap::Parser;
use std::f32::consts::PI;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Command;
use std::fs;

// ── CLI ───────────────────────────────────────────────────────────────────────
#[derive(Parser, Debug)]
#[command(name = "cw-trainer", about = "Morse code (CW) training tool\n\
    Generates random sentences, plays them as CW, and checks your copy.")]
struct Args {
    /// Character speed in WPM (5-150)
    #[arg(long, default_value_t = 20)]
    wpm: u32,

    /// Effective (Farnsworth) speed in WPM — controls letter/word spacing
    #[arg(long, default_value_t = 15)]
    eff: u32,

    /// Tone frequency in Hz (250-990)
    #[arg(long, default_value_t = 600)]
    tone: u32,

    /// Language: en, de, or both
    #[arg(long, default_value = "en")]
    lang: String,

    /// Number of words (or callsigns, in --callsigns mode) per round
    #[arg(long, default_value_t = 5)]
    words: usize,

    /// Practice amateur-radio callsigns instead of random sentences.
    /// Entries are read from `callsigns.txt` (see --callsigns-file).
    #[arg(long, default_value_t = false)]
    callsigns: bool,

    /// Path to the callsigns list file. Default: `callsigns.txt` in the
    /// current directory, then next to the executable.
    #[arg(long)]
    callsigns_file: Option<PathBuf>,

    /// Your own callsign. When set, it is randomly mixed into each round
    /// alongside the callsigns from the list (roughly one slot in three).
    /// Implies --callsigns.
    #[arg(long)]
    mycall: Option<String>,

    /// Practice random character groups instead of sentences or callsigns.
    /// Each "word" is a random group of letters (and optionally digits).
    #[arg(long, default_value_t = false)]
    chars: bool,

    /// Number of characters per group when using --chars (1-6).
    #[arg(long, default_value_t = 1)]
    group_size: usize,

    /// Include digits 0-9 alongside letters in --chars mode.
    /// Without this flag, only letters A-Z are used.
    #[arg(long, default_value_t = false)]
    with_numbers: bool,
}

// ── Morse table ───────────────────────────────────────────────────────────────
fn morse(c: char) -> Option<&'static str> {
    match c.to_ascii_uppercase() {
        'A' => Some(".-"),    'B' => Some("-..."),  'C' => Some("-.-."),
        'D' => Some("-.."),   'E' => Some("."),      'F' => Some("..-."),
        'G' => Some("--."),   'H' => Some("...."),   'I' => Some(".."),
        'J' => Some(".---"),  'K' => Some("-.-"),    'L' => Some(".-.."),
        'M' => Some("--"),    'N' => Some("-."),     'O' => Some("---"),
        'P' => Some(".--."),  'Q' => Some("--.-"),   'R' => Some(".-."),
        'S' => Some("..."),   'T' => Some("-"),      'U' => Some("..-"),
        'V' => Some("...-"),  'W' => Some(".--"),    'X' => Some("-..-"),
        'Y' => Some("-.--"),  'Z' => Some("--.."),
        '0' => Some("-----"), '1' => Some(".----"),  '2' => Some("..---"),
        '3' => Some("...--"), '4' => Some("....-"),  '5' => Some("....."),
        '6' => Some("-...."), '7' => Some("--..."),  '8' => Some("---.." ),
        '9' => Some("----."),
        '.' => Some(".-.-.-"), ',' => Some("--..--"), '?' => Some("..--.."),
        '/' => Some("-..-."),  '+' => Some(".-.-."),  '=' => Some("-...-"),
        _ => None,
    }
}

// ── CW audio generator → WAV bytes ───────────────────────────────────────────
fn add_tone(buf: &mut Vec<f32>, n: usize, freq: u32, rate: u32) {
    let ramp = (n / 8).max(1);
    for i in 0..n {
        let t = i as f32 / rate as f32;
        let mut s = (2.0 * PI * freq as f32 * t).sin() * 0.45;
        if i < ramp        { s *= i as f32 / ramp as f32; }
        else if i >= n - ramp { s *= (n - i) as f32 / ramp as f32; }
        buf.push(s);
    }
}

fn add_silence(buf: &mut Vec<f32>, n: usize) {
    buf.extend(std::iter::repeat(0.0f32).take(n));
}

fn pcm_to_wav(pcm: &[f32], rate: u32) -> Vec<u8> {
    let samples: Vec<i16> = pcm.iter().map(|&s| (s * 32767.0) as i16).collect();
    let data_len = samples.len() * 2;
    let mut w = Vec::with_capacity(44 + data_len);
    w.extend_from_slice(b"RIFF");
    w.extend_from_slice(&((36 + data_len) as u32).to_le_bytes());
    w.extend_from_slice(b"WAVE");
    w.extend_from_slice(b"fmt ");
    w.extend_from_slice(&16u32.to_le_bytes());
    w.extend_from_slice(&1u16.to_le_bytes());          // PCM
    w.extend_from_slice(&1u16.to_le_bytes());          // mono
    w.extend_from_slice(&rate.to_le_bytes());
    w.extend_from_slice(&(rate * 2).to_le_bytes());    // byte rate
    w.extend_from_slice(&2u16.to_le_bytes());          // block align
    w.extend_from_slice(&16u16.to_le_bytes());         // bits/sample
    w.extend_from_slice(b"data");
    w.extend_from_slice(&(data_len as u32).to_le_bytes());
    for s in &samples { w.extend_from_slice(&s.to_le_bytes()); }
    w
}

fn generate_wav(text: &str, wpm: u32, eff: u32, freq: u32) -> Vec<u8> {
    let rate = 44100u32;
    let dit  = (1200.0 / wpm as f32 * rate as f32 / 1000.0) as usize;
    let dah  = dit * 3;
    let eff_unit = (1200.0 / eff as f32 * rate as f32 / 1000.0) as usize;
    let char_gap = eff_unit * 3;  // Farnsworth: stretched letter spacing
    let word_gap = eff_unit * 7;  // Farnsworth: stretched word spacing

    let mut buf: Vec<f32> = Vec::new();
    add_silence(&mut buf, rate as usize / 4);  // leading silence

    let words: Vec<&str> = text.split_whitespace().collect();
    for (wi, word) in words.iter().enumerate() {
        let chars: Vec<char> = word.chars().collect();
        for (ci, &ch) in chars.iter().enumerate() {
            if let Some(code) = morse(ch) {
                let elems: Vec<char> = code.chars().collect();
                for (ei, &el) in elems.iter().enumerate() {
                    match el {
                        '.' => add_tone(&mut buf, dit, freq, rate),
                        '-' => add_tone(&mut buf, dah, freq, rate),
                        _   => {}
                    }
                    if ei < elems.len() - 1 { add_silence(&mut buf, dit); }
                }
                if ci < chars.len() - 1 {
                    add_silence(&mut buf, char_gap.saturating_sub(dit));
                }
            }
        }
        if wi < words.len() - 1 {
            add_silence(&mut buf, word_gap.saturating_sub(char_gap));
        }
    }
    add_silence(&mut buf, rate as usize / 2);  // trailing silence
    pcm_to_wav(&buf, rate)
}

// ── Sentence generator ────────────────────────────────────────────────────────
fn pick<T: Copy>(arr: &[T]) -> T {
    arr[fastrand::usize(0..arr.len())]
}

fn gen_sentence_en(n_words: usize) -> String {
    let nouns    = ["cat","dog","mice","house","tree","car","sun","river","book",
                    "man","woman","girl","child","bird","ship","road","town","field"];
    let verbs    = ["see","hear","find","have","make","take","know","like",
                    "want","need","show","give","call","keep","tell","try"];
    let adjs     = ["big","small","old","new","good","fast","slow","red",
                    "blue","dark","long","short","warm","cold","wet","bright","quiet"];
    let articles = ["the", "a"];

    let mut words: Vec<&str> = Vec::new();
    while words.len() < n_words {
        let remaining = n_words - words.len();
        match remaining {
            1 => words.push(pick(&nouns)),
            2 => { words.push(pick(&articles)); words.push(pick(&nouns)); }
            _ => match fastrand::u32(0..4) {
                0 => words.push(pick(&verbs)),
                1 => { words.push(pick(&articles)); words.push(pick(&nouns)); }
                2 => { words.push(pick(&adjs)); words.push(pick(&nouns)); }
                _ => { words.push(pick(&articles)); words.push(pick(&adjs)); words.push(pick(&nouns)); }
            },
        }
    }
    words.join(" ")
}

fn gen_sentence_de(n_words: usize) -> String {
    let nouns_m  = ["hund","baum","mann","zug","wind","berg","fluss","brief","dl1tsw","dd6ds"];
    let nouns_f  = ["katze","maus","vogel","frau","sonne","stadt","nacht","schule","bahn"];
    let nouns_n  = ["haus","auto","kind","buch","boot","licht","geld","bild","fahrrad"];
    let verbs    = ["sieht","hoert","hat","macht","nimmt","kennt","liebt",
                    "sucht","baut","zeigt","findet","sendet","braucht"];
    let adjs     = ["gross","klein","alt","neu","gut","schnell","langsam","ruhig","laut","leise",
                    "warm","kalt","nass","lang","kurz","stark","schwach","schoen","klar"];
    let articles = ["der","die","das","ein","eine"];

    let mut words: Vec<&str> = Vec::new();
    while words.len() < n_words {
        let remaining = n_words - words.len();
        match remaining {
            1 => words.push(pick(&nouns_n)),
            2 => { words.push(pick(&articles)); words.push(pick(&nouns_m)); }
            _ => match fastrand::u32(0..4) {
                0 => words.push(pick(&verbs)),
                1 => { words.push(pick(&articles)); words.push(pick(&nouns_f)); }
                2 => { words.push(pick(&adjs)); words.push(pick(&nouns_n)); }
                _ => { words.push(pick(&articles)); words.push(pick(&adjs)); words.push(pick(&nouns_m)); }
            },
        }
    }
    words.join(" ")
}

fn gen_sentence(lang: &str, n: usize) -> String {
    match lang {
        "de"   => gen_sentence_de(n),
        "both" => if fastrand::bool() { gen_sentence_en(n) } else { gen_sentence_de(n) },
        _      => gen_sentence_en(n),
    }
}

// ── Random character / digit groups ──────────────────────────────────────────
/// Build the pool of characters used for char-drill mode.
/// Always includes A-Z; includes 0-9 when `with_numbers` is true.
fn char_pool(with_numbers: bool) -> Vec<char> {
    let mut pool: Vec<char> = (b'A'..=b'Z').map(|b| b as char).collect();
    if with_numbers {
        pool.extend((b'0'..=b'9').map(|b| b as char));
    }
    pool
}

/// Generate `n_groups` random groups, each of `group_size` characters,
/// drawn uniformly from the pool. Groups are separated by single spaces
/// so the existing playback / scoring code treats each group as one "word".
fn gen_char_groups(n_groups: usize, group_size: usize, with_numbers: bool) -> String {
    let pool = char_pool(with_numbers);
    let mut groups: Vec<String> = Vec::with_capacity(n_groups);
    for _ in 0..n_groups {
        let mut g = String::with_capacity(group_size);
        for _ in 0..group_size {
            g.push(pool[fastrand::usize(0..pool.len())]);
        }
        groups.push(g);
    }
    groups.join(" ")
}

// ── Callsigns ─────────────────────────────────────────────────────────────────
/// Resolve the callsigns file path. Preference order:
///   1. explicit `--callsigns-file <PATH>`
///   2. `callsigns.txt` in the current working directory
///   3. `callsigns.txt` next to the executable
fn resolve_callsigns_path(override_path: Option<&PathBuf>) -> Option<PathBuf> {
    if let Some(p) = override_path {
        return Some(p.clone());
    }
    let cwd_candidate = PathBuf::from("callsigns.txt");
    if cwd_candidate.is_file() {
        return Some(cwd_candidate);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let c = dir.join("callsigns.txt");
            if c.is_file() {
                return Some(c);
            }
        }
    }
    None
}

/// Parse a file of quoted, comma-separated callsigns, e.g.
///   "K1ABC","K2BCD","DL1ABC",
/// Whitespace, blank lines, and `#` / `//` comment lines are ignored.
/// Returns every string found between matching double quotes, trimmed and uppercased.
fn parse_callsigns(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
            continue;
        }
        let mut in_quote = false;
        let mut cur = String::new();
        for ch in line.chars() {
            if ch == '"' {
                if in_quote {
                    let token = cur.trim().to_uppercase();
                    if !token.is_empty() {
                        out.push(token);
                    }
                    cur.clear();
                }
                in_quote = !in_quote;
            } else if in_quote {
                cur.push(ch);
            }
        }
    }
    out
}

fn load_callsigns(path: &std::path::Path) -> Result<Vec<String>, String> {
    let text = fs::read_to_string(path)
        .map_err(|e| format!("cannot read callsigns file {}: {}", path.display(), e))?;
    let list = parse_callsigns(&text);
    if list.is_empty() {
        return Err(format!(
            "no callsigns found in {} — expected quoted entries like \"K1ABC\"",
            path.display()
        ));
    }
    Ok(list)
}

/// Pick `n` callsigns for one round.
///
/// If `mycall` is `Some`, each slot has a ~1-in-3 chance of being the user's
/// own callsign — so they hear it often enough to train instant recognition,
/// but it doesn't dominate the round.
fn pick_callsigns(list: &[String], mycall: Option<&str>, n: usize) -> String {
    (0..n)
        .map(|_| {
            if let Some(mc) = mycall {
                if fastrand::u32(0..3) == 0 {
                    return mc.to_string();
                }
            }
            list[fastrand::usize(0..list.len())].clone()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

// ── Comparison ────────────────────────────────────────────────────────────────
fn compare(original: &str, attempt: &str) -> (f32, f32) {
    let orig_words: Vec<&str> = original.split_whitespace().collect();
    let att_words:  Vec<&str> = attempt.split_whitespace().collect();

    // Word accuracy
    let matched_words = orig_words.iter().zip(att_words.iter())
        .filter(|(a, b)| a.to_lowercase() == b.to_lowercase())
        .count();
    let word_acc = if orig_words.is_empty() { 100.0 }
        else { matched_words as f32 / orig_words.len() as f32 * 100.0 };

    // Character accuracy (over original length)
    let orig_chars: Vec<char> = original.to_lowercase().chars().collect();
    let att_chars:  Vec<char> = attempt.to_lowercase().chars().collect();
    let matched_chars = orig_chars.iter().zip(att_chars.iter())
        .filter(|(a, b)| a == b).count();
    let char_acc = if orig_chars.is_empty() { 100.0 }
        else { matched_chars as f32 / orig_chars.len() as f32 * 100.0 };

    (word_acc, char_acc)
}

fn print_diff(original: &str, attempt: &str) {
    let orig_words: Vec<&str> = original.split_whitespace().collect();
    let att_words:  Vec<&str> = attempt.split_whitespace().collect();
    print!("  Original : ");
    for w in &orig_words { print!("{} ", w); }
    println!();
    print!("  Your copy: ");
    for (i, w) in att_words.iter().enumerate() {
        let ok = orig_words.get(i)
            .map(|o| o.to_lowercase() == w.to_lowercase())
            .unwrap_or(false);
        if ok { print!("{} ", w); }
        else  { print!("[{}] ", w); }
    }
    // show missing words
    if att_words.len() < orig_words.len() {
        for w in &orig_words[att_words.len()..] { print!("({}?) ", w); }
    }
    println!();
}

// ── Helpers ───────────────────────────────────────────────────────────────────
fn prompt(msg: &str) -> String {
    print!("{}", msg);
    io::stdout().flush().unwrap();
    let mut line = String::new();
    io::stdin().read_line(&mut line).unwrap();
    line.trim().to_string()
}

/// Cross-platform synchronous WAV playback.
///
/// Uses:
///   - Windows: PowerShell's System.Media.SoundPlayer (PlaySync)
///   - macOS:   `afplay`
///   - Linux/*BSD: tries `aplay` (ALSA), then falls back to `paplay` (PulseAudio)
fn play_wav(path: &str) {
    #[cfg(target_os = "windows")]
    {
        // Use .PlaySync() so the call blocks until playback finishes.
        // Escape single quotes in the path for the PowerShell string literal.
        let ps_path = path.replace('\'', "''");
        let script = format!(
            "(New-Object System.Media.SoundPlayer '{}').PlaySync()",
            ps_path
        );
        let status = Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .status();
        if let Err(e) = status {
            eprintln!("  [playback error (powershell): {}]", e);
        }
        return;
    }

    #[cfg(target_os = "macos")]
    {
        let status = Command::new("afplay").arg(path).status();
        if let Err(e) = status {
            eprintln!("  [playback error (afplay): {}]", e);
        }
        return;
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        // Try aplay first (ALSA), then paplay (PulseAudio) as a fallback.
        if Command::new("aplay").args(["-q", path]).status().is_ok() {
            return;
        }
        if let Err(e) = Command::new("paplay").arg(path).status() {
            eprintln!("  [playback error: neither aplay nor paplay available ({})]", e);
        }
    }
}

// ── Main ──────────────────────────────────────────────────────────────────────
fn main() {
    let args = Args::parse();

    // Clamp eff to <= wpm
    let eff = args.eff.min(args.wpm);

    // Normalise --mycall: trim, uppercase, treat empty as "not set".
    let mycall: Option<String> = args.mycall.as_ref()
        .map(|s| s.trim().to_uppercase())
        .filter(|s| !s.is_empty());

    // Convenience: passing --callsigns-file or --mycall implies --callsigns.
    // Otherwise those flags would be silently ignored, which is surprising.
    let callsigns_mode = args.callsigns || args.callsigns_file.is_some() || mycall.is_some();

    // --chars / --callsigns are mutually exclusive — they describe different
    // training drills and would silently shadow each other otherwise.
    if args.chars && callsigns_mode {
        eprintln!("error: --chars cannot be combined with --callsigns / --callsigns-file / --mycall.");
        std::process::exit(2);
    }

    // Validate group size when --chars is in use. Range chosen to match the
    // user-facing options (1-6 characters per group).
    if args.chars && !(1..=6).contains(&args.group_size) {
        eprintln!(
            "error: --group-size must be between 1 and 6 (got {}).",
            args.group_size
        );
        std::process::exit(2);
    }

    // Load the callsigns list up front (fail fast if the file is missing).
    let callsigns_list: Option<Vec<String>> = if callsigns_mode {
        let path = resolve_callsigns_path(args.callsigns_file.as_ref())
            .unwrap_or_else(|| {
                eprintln!(
                    "error: --callsigns was given but no callsigns file was found.\n\
                     Place `callsigns.txt` next to the binary or in the current directory,\n\
                     or pass --callsigns-file <PATH>."
                );
                std::process::exit(2);
            });
        match load_callsigns(&path) {
            Ok(list) => {
                println!("  (loaded {} callsigns from {})", list.len(), path.display());
                Some(list)
            }
            Err(msg) => {
                eprintln!("error: {}", msg);
                std::process::exit(2);
            }
        }
    } else {
        None
    };

    let chars_mode_label: String = if args.chars {
        if args.with_numbers {
            format!("chars+digits x{}", args.group_size)
        } else {
            format!("chars x{}", args.group_size)
        }
    } else {
        String::new()
    };
    let mode_label: &str = if args.chars {
        chars_mode_label.as_str()
    } else if callsigns_mode {
        "callsigns"
    } else {
        args.lang.as_str()
    };
    let per_round_label = if args.chars {
        "Groups"
    } else if callsigns_mode {
        "Calls"
    } else {
        "Words"
    };

    println!();
    println!("╔══════════════════════════════════════╗");
    println!("║        CW Trainer — lcwo style       ║");
    println!("╚══════════════════════════════════════╝");
    println!("  Speed : {} WPM  |  Eff : {} WPM  |  Tone : {} Hz  |  Mode : {}  |  {} : {}",
        args.wpm, eff, args.tone, mode_label, per_round_label, args.words);
    if let Some(mc) = &mycall {
        println!("  MyCall: {}  (mixed into ~1 in 3 callsigns per round)", mc);
    }
    println!("  Commands after playback: [r]epeat  [s]how text  [Enter] = enter copy");
    println!();

    // Use the OS temp dir so this works on Linux, macOS, and Windows.
    let tmp_path: PathBuf = std::env::temp_dir().join("cw_trainer.wav");
    let tmp_path_str = tmp_path.to_string_lossy().into_owned();
    let mut total_rounds  = 0u32;
    let mut total_word_acc = 0.0f32;

    loop {
        total_rounds += 1;
        let sentence = if args.chars {
            gen_char_groups(args.words, args.group_size, args.with_numbers)
        } else {
            match &callsigns_list {
                Some(list) => pick_callsigns(list, mycall.as_deref(), args.words),
                None       => gen_sentence(&args.lang, args.words),
            }
        };
        println!("── Round {} ─────────────────────────────", total_rounds);
        println!("  (generating CW…)");

        let wav = generate_wav(&sentence, args.wpm, eff, args.tone);
        fs::write(&tmp_path, &wav)
            .unwrap_or_else(|e| panic!("Failed to write temp WAV to {}: {}", tmp_path.display(), e));

        // Playback loop
        loop {
            play_wav(&tmp_path_str);
            let cmd = prompt("  [r]epeat / [s]how / [Enter] to copy > ");
            match cmd.to_lowercase().as_str() {
                "r" | "repeat" => continue,
                "s" | "show"   => {
                    println!("  Text: \"{}\"", sentence);
                    continue;
                }
                _ => break,
            }
        }

        // Get copy attempt
        let attempt = prompt("  Your copy: ");
        if attempt.is_empty() {
            println!("  (skipped)");
        } else {
            print_diff(&sentence, &attempt);
            let (wa, ca) = compare(&sentence, &attempt);
            total_word_acc += wa;
            println!("  Word accuracy : {:.0}%  |  Char accuracy : {:.0}%",
                wa, ca);
        }
        println!("  Correct text  : \"{}\"", sentence);

        // Session average
        if total_rounds > 1 {
            println!("  Session avg   : {:.0}% word accuracy over {} rounds",
                total_word_acc / total_rounds as f32, total_rounds);
        }
        println!();

        let again = prompt("  Next sentence? [Y/n] > ");
        if again.to_lowercase().starts_with('n') { break; }
        println!();
    }

    println!("73 de cw-trainer!  Final avg: {:.0}% over {} round(s).",
        if total_rounds > 0 { total_word_acc / total_rounds as f32 } else { 0.0 },
        total_rounds);
    let _ = fs::remove_file(&tmp_path);
}
