use clap::Parser;
use std::f32::consts::PI;
use std::io::{self, Write};
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

    /// Number of words per sentence
    #[arg(long, default_value_t = 5)]
    words: usize,
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
    let nouns    = ["cat","dog","house","tree","car","sun","river","book",
                    "man","woman","child","bird","ship","road","town","field"];
    let verbs    = ["see","hear","find","have","make","take","know","like",
                    "want","need","show","give","call","keep","tell","try"];
    let adjs     = ["big","small","old","new","good","fast","slow","red",
                    "blue","dark","long","short","warm","cold","bright","quiet"];
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
    let nouns_m  = ["hund","baum","mann","zug","wind","berg","fluss","brief"];
    let nouns_f  = ["katze","frau","sonne","stadt","nacht","schule","bahn"];
    let nouns_n  = ["haus","auto","kind","buch","boot","licht","geld","bild"];
    let verbs    = ["sieht","hoert","hat","macht","nimmt","kennt","liebt",
                    "sucht","baut","zeigt","findet","sendet","braucht"];
    let adjs     = ["gross","klein","alt","neu","gut","schnell","ruhig",
                    "warm","kalt","lang","kurz","stark","schoen","klar"];
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

fn play_wav(path: &str) {
    let status = Command::new("aplay")
        .args(["-q", path])
        .status();
    if let Err(e) = status {
        eprintln!("  [aplay error: {}]", e);
    }
}

// ── Main ──────────────────────────────────────────────────────────────────────
fn main() {
    let args = Args::parse();

    // Clamp eff to <= wpm
    let eff = args.eff.min(args.wpm);

    println!();
    println!("╔══════════════════════════════════════╗");
    println!("║        CW Trainer — lcwo style       ║");
    println!("╚══════════════════════════════════════╝");
    println!("  Speed : {} WPM  |  Eff : {} WPM  |  Tone : {} Hz  |  Lang : {}",
        args.wpm, eff, args.tone, args.lang);
    println!("  Commands after playback: [r]epeat  [s]how text  [Enter] = enter copy");
    println!();

    let tmp_path = "/tmp/cw_trainer.wav";
    let mut total_rounds  = 0u32;
    let mut total_word_acc = 0.0f32;

    loop {
        total_rounds += 1;
        let sentence = gen_sentence(&args.lang, args.words);
        println!("── Round {} ─────────────────────────────", total_rounds);
        println!("  (generating CW…)");

        let wav = generate_wav(&sentence, args.wpm, eff, args.tone);
        fs::write(tmp_path, &wav).expect("Failed to write temp WAV");

        // Playback loop
        loop {
            play_wav(tmp_path);
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
    let _ = fs::remove_file(tmp_path);
}
