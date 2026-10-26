use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Word {
    pub text: &'static str,
    pub band: u8,
}

pub const BAND_NAMES: [&str; 5] = [
    "short vowels",
    "blends",
    "digraphs",
    "long vowels",
    "tricky patterns",
];

static LIST: OnceLock<Vec<Word>> = OnceLock::new();

pub fn words() -> &'static [Word] {
    LIST.get_or_init(|| {
        let mut band = 0u8;
        let mut out = vec![];
        for line in include_str!("spell_words.txt").lines() {
            let line = line.trim();
            if let Some(b) = line.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
                band = b.parse().unwrap_or(0);
            } else {
                out.extend(line.split_whitespace().map(|text| Word { text, band }));
            }
        }
        out
    })
}

pub fn band_of(word: &str) -> Option<u8> {
    words().iter().find(|w| w.text == word).map(|w| w.band)
}
