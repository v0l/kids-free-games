use std::cell::RefCell;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Sfx {
    Tap,
    Pop,
    Coin,
    Score,
    Jump,
    Flap,
    Swish,
    Slice,
    Bounce,
    Hit,
    Boom,
    Wrong,
    Right,
    Win,
    LevelUp,
    Whoosh,
    Splash,
    Crunch,
    Click,
    Tick,
    Lose,
    Magic,
    Card,
}

impl Sfx {
    pub const ALL: [Sfx; 23] = [
        Sfx::Tap,
        Sfx::Pop,
        Sfx::Coin,
        Sfx::Score,
        Sfx::Jump,
        Sfx::Flap,
        Sfx::Swish,
        Sfx::Slice,
        Sfx::Bounce,
        Sfx::Hit,
        Sfx::Boom,
        Sfx::Wrong,
        Sfx::Right,
        Sfx::Win,
        Sfx::LevelUp,
        Sfx::Whoosh,
        Sfx::Splash,
        Sfx::Crunch,
        Sfx::Click,
        Sfx::Tick,
        Sfx::Lose,
        Sfx::Magic,
        Sfx::Card,
    ];

    pub fn tier(self) -> Tier {
        match self {
            Sfx::Tick | Sfx::Click | Sfx::Tap => Tier::Ui,
            Sfx::Swish | Sfx::Whoosh | Sfx::Flap | Sfx::Jump | Sfx::Bounce | Sfx::Card => {
                Tier::Small
            }
            Sfx::Boom | Sfx::Win | Sfx::LevelUp | Sfx::Lose => Tier::Large,
            _ => Tier::Medium,
        }
    }

    pub fn ladder(self) -> bool {
        matches!(self, Sfx::Pop | Sfx::Right | Sfx::Coin | Sfx::Score)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tier {
    Ui,
    Small,
    Medium,
    Large,
}

impl Tier {
    pub fn loudness_db(self) -> f32 {
        match self {
            Tier::Ui => -24.0,
            Tier::Small => -21.0,
            Tier::Medium => -18.0,
            Tier::Large => -15.0,
        }
    }

    pub fn ducks_music(self) -> bool {
        self == Tier::Large
    }
}

pub const PEAK_DB: f32 = -1.0;

pub fn db_to_gain(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

pub fn gain_to_db(g: f32) -> f32 {
    20.0 * g.max(1e-6).log10()
}

pub fn loudness_db(x: &[f32]) -> f32 {
    let win = 44100 / 50;
    let loudest = x
        .chunks(win)
        .map(|c| (c.iter().map(|v| v * v).sum::<f32>() / c.len() as f32).sqrt())
        .fold(0.0f32, f32::max);
    gain_to_db(loudest)
}

pub fn peak_db(x: &[f32]) -> f32 {
    gain_to_db(x.iter().fold(0.0f32, |m, v| m.max(v.abs())))
}

pub const LADDER: [f32; 8] = [0.0, 2.0, 4.0, 5.0, 7.0, 9.0, 11.0, 12.0];

pub fn semitones(n: f32) -> f32 {
    2f32.powf(n / 12.0)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mood {
    Menu,
    Arcade,
    Learn,
}

impl Mood {
    pub const ALL: [Mood; 3] = [Mood::Menu, Mood::Arcade, Mood::Learn];
}

pub const MIN_GAP: f64 = 0.045;

#[derive(Default)]
pub struct Limiter {
    last: Vec<(Sfx, f64)>,
}

impl Limiter {
    pub fn allow(&mut self, s: Sfx, now: f64) -> bool {
        match self.last.iter_mut().find(|(k, _)| *k == s) {
            Some((_, t)) if now - *t < MIN_GAP => false,
            Some((_, t)) => {
                *t = now;
                true
            }
            None => {
                self.last.push((s, now));
                true
            }
        }
    }
}

pub const DUCK_DB: f32 = -9.0;
pub const DUCK_HOLD: f32 = 0.5;
pub const DUCK_RELEASE: f32 = 0.45;
pub const MUSIC_BUS_DB: f32 = -3.0;
pub const SFX_BUS_DB: f32 = 0.0;
pub const MUSIC_FADE: f32 = 0.35;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mix {
    pub duck: f32,
    pub fade_in: f32,
}

impl Default for Mix {
    fn default() -> Self {
        Self {
            duck: f32::INFINITY,
            fade_in: 0.0,
        }
    }
}

impl Mix {
    pub fn ducked(&mut self) {
        self.duck = 0.0;
    }

    pub fn step(&mut self, dt: f32) {
        self.duck += dt;
        self.fade_in = (self.fade_in + dt / MUSIC_FADE).min(1.0);
    }

    pub fn duck_db(&self) -> f32 {
        if self.duck < DUCK_HOLD {
            DUCK_DB
        } else {
            let k = ((self.duck - DUCK_HOLD) / DUCK_RELEASE).clamp(0.0, 1.0);
            DUCK_DB * (1.0 - k * k * (3.0 - 2.0 * k))
        }
    }

    pub fn music_gain(&self) -> f32 {
        db_to_gain(MUSIC_BUS_DB + self.duck_db()) * self.fade_in
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pitch {
    Vary,
    Step(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cue {
    pub sfx: Sfx,
    pub pitch: Pitch,
}

pub static PACK: &[u8] = include_bytes!("../../../assets/sound.kgs");

pub fn unpack(bytes: &[u8]) -> Option<Vec<(&str, &[u8])>> {
    let u32_at = |at: usize| -> Option<usize> {
        Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?) as usize)
    };
    if bytes.get(0..4)? != b"KGS1" {
        return None;
    }
    let n = u32_at(4)?;
    let mut at = 8;
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let nl = u32_at(at)?;
        let name = std::str::from_utf8(bytes.get(at + 4..at + 4 + nl)?).ok()?;
        at += 4 + nl;
        let dl = u32_at(at)?;
        out.push((name, bytes.get(at + 4..at + 4 + dl)?));
        at += 4 + dl;
    }
    Some(out)
}

pub fn decode(ogg: &[u8]) -> Option<(u32, Vec<i16>)> {
    let mut r = lewton::inside_ogg::OggStreamReader::new(std::io::Cursor::new(ogg)).ok()?;
    let rate = r.ident_hdr.audio_sample_rate;
    let ch = r.ident_hdr.audio_channels as usize;
    let mut out = vec![];
    while let Some(p) = r.read_dec_packet_itl().ok()? {
        out.extend(p.chunks(ch.max(1)).map(|c| c[0]));
    }
    let end = out
        .iter()
        .rposition(|s| s.unsigned_abs() > 16)
        .map_or(0, |i| i + 1);
    out.truncate(end);
    Some((rate, out))
}

pub fn wav_i16(rate: u32, samples: &[i16]) -> Vec<u8> {
    let data_len = samples.len() as u32 * 2;
    let mut b = Vec::with_capacity(44 + data_len as usize);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data_len).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&rate.to_le_bytes());
    b.extend_from_slice(&(rate * 2).to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        b.extend_from_slice(&s.to_le_bytes());
    }
    b
}

impl Sfx {
    pub fn key(self) -> &'static str {
        match self {
            Sfx::Tap => "tap",
            Sfx::Pop => "pop",
            Sfx::Coin => "coin",
            Sfx::Score => "score",
            Sfx::Jump => "jump",
            Sfx::Flap => "flap",
            Sfx::Swish => "swish",
            Sfx::Slice => "slice",
            Sfx::Bounce => "bounce",
            Sfx::Hit => "hit",
            Sfx::Boom => "boom",
            Sfx::Wrong => "wrong",
            Sfx::Right => "right",
            Sfx::Win => "win",
            Sfx::LevelUp => "levelup",
            Sfx::Whoosh => "whoosh",
            Sfx::Splash => "splash",
            Sfx::Crunch => "crunch",
            Sfx::Click => "click",
            Sfx::Tick => "tick",
            Sfx::Lose => "lose",
            Sfx::Magic => "magic",
            Sfx::Card => "card",
        }
    }
}

impl Mood {
    pub fn key(self) -> &'static str {
        match self {
            Mood::Menu => "music_menu",
            Mood::Arcade => "music_arcade",
            Mood::Learn => "music_learn",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Say {
    Correct,
    Great,
    Congrats,
    Win,
    LevelUp,
    GameOver,
    Ready,
    Go,
    Best,
    Number(u8),
}

impl Say {
    pub fn key(self) -> String {
        match self {
            Say::Correct => "say_correct".into(),
            Say::Great => "say_great".into(),
            Say::Congrats => "say_congrats".into(),
            Say::Win => "say_win".into(),
            Say::LevelUp => "say_levelup".into(),
            Say::GameOver => "say_gameover".into(),
            Say::Ready => "say_ready".into(),
            Say::Go => "say_go".into(),
            Say::Best => "say_best".into(),
            Say::Number(n) => format!("say_{}", n.clamp(1, 10)),
        }
    }
}

thread_local! {
    static PENDING: RefCell<Vec<Cue>> = const { RefCell::new(Vec::new()) };
    static VOICE: RefCell<Option<Say>> = const { RefCell::new(None) };
    static WANT: std::cell::Cell<Option<Mood>> = const { std::cell::Cell::new(None) };
    static SFX_ON: std::cell::Cell<bool> = const { std::cell::Cell::new(true) };
    static MUSIC_ON: std::cell::Cell<bool> = const { std::cell::Cell::new(true) };
}

pub fn find<'a>(pack: &'a [(&str, &[u8])], key: &str) -> Option<&'a [u8]> {
    pack.iter().find(|(k, _)| *k == key).map(|(_, d)| *d)
}

pub fn takes_of<'a>(pack: &'a [(&'a str, &'a [u8])], s: Sfx) -> Vec<&'a [u8]> {
    let pre = format!("{}.", s.key());
    pack.iter()
        .filter(|(k, _)| k.starts_with(&pre) && !k.contains('+'))
        .map(|(_, d)| *d)
        .collect()
}

pub fn ladder_of<'a>(pack: &'a [(&'a str, &'a [u8])], s: Sfx) -> Vec<&'a [u8]> {
    LADDER
        .iter()
        .filter_map(|n| find(pack, &format!("{}.0+{}", s.key(), *n as i32)))
        .collect()
}

pub fn voice_line(key: &str) -> &str {
    match key.rsplit_once('.') {
        Some((line, n)) if n.chars().all(|c| c.is_ascii_digit()) => line,
        _ => key,
    }
}

pub fn pick_take(count: usize, last: Option<usize>, roll: usize) -> usize {
    match (count, last) {
        (0 | 1, _) => 0,
        (n, Some(l)) if l < n => (l + 1 + roll % (n - 1)) % n,
        (n, _) => roll % n,
    }
}

pub fn say(s: Say) {
    VOICE.with(|v| *v.borrow_mut() = Some(s));
}

pub fn take_voice() -> Option<Say> {
    VOICE.with(|v| v.borrow_mut().take())
}

pub const VOICE_GAP: f64 = 0.9;

const MAX_PENDING: usize = 16;

fn queue(c: Cue) {
    PENDING.with(|p| {
        let mut p = p.borrow_mut();
        let full = p.len() >= MAX_PENDING;
        match p.iter_mut().find(|q| q.sfx == c.sfx) {
            Some(q) => {
                if let Pitch::Step(_) = c.pitch {
                    q.pitch = c.pitch;
                }
            }
            None if !full => p.push(c),
            None => {}
        }
    });
}

pub fn play(s: Sfx) {
    queue(Cue {
        sfx: s,
        pitch: Pitch::Vary,
    });
}

pub fn play_step(s: Sfx, step: usize) {
    queue(Cue {
        sfx: s,
        pitch: Pitch::Step(step.min(LADDER.len() - 1)),
    });
}

pub fn take_pending() -> Vec<Sfx> {
    take_cues().into_iter().map(|c| c.sfx).collect()
}

pub fn take_cues() -> Vec<Cue> {
    PENDING.with(|p| std::mem::take(&mut *p.borrow_mut()))
}

pub fn music(m: Mood) {
    WANT.with(|w| w.set(Some(m)));
}

pub fn wanted_music() -> Option<Mood> {
    WANT.with(|w| w.get())
}

pub fn muted() -> bool {
    !SFX_ON.with(|s| s.get())
}

pub fn toggle_mute() {
    SFX_ON.with(|s| s.set(!s.get()));
}

pub fn music_on() -> bool {
    MUSIC_ON.with(|m| m.get())
}

pub fn toggle_music() {
    MUSIC_ON.with(|m| m.set(!m.get()));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pack() -> Vec<(&'static str, &'static [u8])> {
        unpack(PACK).expect("sound pack parses")
    }

    fn pcm(ogg: &[u8]) -> Vec<f32> {
        decode(ogg)
            .unwrap()
            .1
            .iter()
            .map(|s| *s as f32 / 32768.0)
            .collect()
    }

    #[test]
    fn every_sound_has_at_least_one_take_that_decodes() {
        let p = pack();
        for s in Sfx::ALL {
            let t = takes_of(&p, s);
            assert!(!t.is_empty(), "{s:?} has no takes in sound.txt");
            for d in t {
                let (rate, x) = decode(d).unwrap_or_else(|| panic!("{s:?} does not decode"));
                assert_eq!(rate, 44100);
                let secs = x.len() as f32 / rate as f32;
                assert!(secs > 0.005 && secs < 3.0, "{s:?} is {secs}s");
            }
        }
    }

    #[test]
    fn ladder_sounds_have_a_full_scale() {
        let p = pack();
        for s in Sfx::ALL.into_iter().filter(|s| s.ladder()) {
            assert_eq!(
                ladder_of(&p, s).len(),
                LADDER.len(),
                "{s:?} ladder incomplete"
            );
        }
    }

    #[test]
    fn ladder_steps_rise_in_pitch_to_an_octave() {
        let p = pack();
        for s in Sfx::ALL.into_iter().filter(|s| s.ladder()) {
            let lens: Vec<f32> = ladder_of(&p, s)
                .iter()
                .map(|d| pcm(d).len() as f32)
                .collect();
            assert!(lens.windows(2).all(|w| w[1] < w[0]), "{s:?} {lens:?}");
            let ratio = lens[0] / lens[lens.len() - 1];
            assert!(
                (ratio - 2.0).abs() < 0.15,
                "{s:?} top step is {ratio}x, want an octave"
            );
        }
    }

    #[test]
    fn tiers_land_at_their_loudness_with_headroom() {
        let p = pack();
        for s in Sfx::ALL {
            for d in takes_of(&p, s) {
                let x = pcm(d);
                let l = loudness_db(&x);
                let want = s.tier().loudness_db();
                assert!((l - want).abs() < 4.0, "{s:?} at {l:.1} dB, want {want}");
                assert!(
                    peak_db(&x) <= PEAK_DB + 0.5,
                    "{s:?} peaks at {}",
                    peak_db(&x)
                );
            }
        }
    }

    #[test]
    fn praise_has_many_takes() {
        let p = pack();
        for v in [
            Say::Correct,
            Say::Great,
            Say::Congrats,
            Say::Win,
            Say::LevelUp,
            Say::Best,
            Say::GameOver,
        ] {
            let n = p.iter().filter(|(k, _)| voice_line(k) == v.key()).count();
            assert!(n >= 3, "{v:?} has {n} takes");
        }
    }

    #[test]
    fn a_voice_never_says_the_same_take_twice_in_a_row() {
        let mut last = None;
        let mut seen = std::collections::HashSet::new();
        for roll in 0..200usize {
            let i = pick_take(8, last, roll.wrapping_mul(2_654_435_761) >> 7);
            assert_ne!(Some(i), last);
            seen.insert(i);
            last = Some(i);
        }
        assert_eq!(seen.len(), 8);
        assert_eq!(pick_take(1, Some(0), 5), 0);
        assert_eq!(voice_line("say_great.3"), "say_great");
        assert_eq!(voice_line("say_10"), "say_10");
    }

    #[test]
    fn every_voice_line_is_packed() {
        let p = pack();
        let mut lines = vec![
            Say::Correct,
            Say::Great,
            Say::Congrats,
            Say::Win,
            Say::LevelUp,
            Say::GameOver,
            Say::Ready,
            Say::Go,
            Say::Best,
        ];
        lines.extend((1..=10).map(Say::Number));
        for v in lines {
            let takes: Vec<_> = p.iter().filter(|(k, _)| voice_line(k) == v.key()).collect();
            assert!(!takes.is_empty(), "{v:?} missing");
            for (_, d) in takes {
                assert!(decode(d).is_some());
            }
        }
        assert_eq!(Say::Number(0).key(), "say_1");
        assert_eq!(Say::Number(42).key(), "say_10");
    }

    #[test]
    fn music_loops_are_long_and_sit_under_the_effects() {
        let p = pack();
        for m in Mood::ALL {
            let x = pcm(find(&p, m.key()).unwrap_or_else(|| panic!("{m:?} missing")));
            assert!(x.len() as f32 / 44100.0 > 15.0, "{m:?} loop too short");
            let music = loudness_db(&x) + MUSIC_BUS_DB;
            assert!(
                music + 6.0 <= Tier::Small.loudness_db(),
                "{m:?} music at {music:.1} dB crowds the effects"
            );
        }
    }

    #[test]
    fn wav_header_is_valid_pcm() {
        let b = wav_i16(44100, &[0, 1, -1]);
        assert_eq!(&b[0..4], b"RIFF");
        assert_eq!(
            u32::from_le_bytes(b[4..8].try_into().unwrap()) as usize,
            b.len() - 8
        );
        assert_eq!(u32::from_le_bytes(b[24..28].try_into().unwrap()), 44100);
        assert_eq!(u32::from_le_bytes(b[40..44].try_into().unwrap()), 6);
    }

    #[test]
    fn decoding_the_whole_pack_is_quick() {
        let t = std::time::Instant::now();
        for (_, d) in pack() {
            decode(d).unwrap();
        }
        eprintln!("decoded pack in {:?}", t.elapsed());
        let limit = if cfg!(debug_assertions) { 20.0 } else { 1.5 };
        assert!(t.elapsed().as_secs_f32() < limit, "took {:?}", t.elapsed());
    }

    #[test]
    fn unpack_rejects_garbage() {
        assert!(unpack(b"nope").is_none());
        assert!(unpack(b"KGS1\x02\x00\x00\x00").is_none());
    }

    #[test]
    fn same_sound_spam_is_limited() {
        let mut l = Limiter::default();
        assert!(l.allow(Sfx::Pop, 0.0));
        assert!(!l.allow(Sfx::Pop, 0.01));
        assert!(l.allow(Sfx::Coin, 0.01));
        assert!(l.allow(Sfx::Pop, 0.06));
    }

    #[test]
    fn big_moments_duck_the_music_then_it_recovers() {
        let mut mix = Mix {
            fade_in: 1.0,
            ..Default::default()
        };
        let full = mix.music_gain();
        mix.ducked();
        mix.step(0.1);
        assert!((gain_to_db(mix.music_gain() / full) - DUCK_DB).abs() < 0.1);
        let mut prev = mix.music_gain();
        for _ in 0..60 {
            mix.step(1.0 / 60.0);
            assert!(mix.music_gain() >= prev - 1e-6);
            prev = mix.music_gain();
        }
        assert!((mix.music_gain() - full).abs() < 1e-4);
    }

    #[test]
    fn play_step_clamps_to_the_ladder() {
        take_cues();
        play_step(Sfx::Pop, 99);
        assert_eq!(
            take_cues(),
            vec![Cue {
                sfx: Sfx::Pop,
                pitch: Pitch::Step(LADDER.len() - 1)
            }]
        );
    }

    #[test]
    fn a_ladder_step_overrides_a_plain_cue_for_the_same_sound() {
        take_cues();
        play(Sfx::Right);
        play_step(Sfx::Right, 3);
        play(Sfx::Right);
        assert_eq!(
            take_cues(),
            vec![Cue {
                sfx: Sfx::Right,
                pitch: Pitch::Step(3)
            }]
        );
    }

    #[test]
    fn the_latest_voice_line_wins() {
        take_voice();
        say(Say::Correct);
        say(Say::LevelUp);
        assert_eq!(take_voice(), Some(Say::LevelUp));
        assert_eq!(take_voice(), None);
    }

    #[test]
    fn play_queues_until_flushed() {
        take_pending();
        play(Sfx::Tap);
        play(Sfx::Win);
        play(Sfx::Tap);
        assert_eq!(take_pending(), vec![Sfx::Tap, Sfx::Win]);
        assert!(take_pending().is_empty());
    }
}
