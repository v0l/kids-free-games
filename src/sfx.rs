use std::cell::RefCell;
use std::f32::consts::TAU;

pub const RATE: u32 = 22050;

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
}

impl Sfx {
    pub const ALL: [Sfx; 20] = [
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
    ];

    pub fn tier(self) -> Tier {
        match self {
            Sfx::Tick | Sfx::Click | Sfx::Tap => Tier::Ui,
            Sfx::Swish | Sfx::Whoosh | Sfx::Flap | Sfx::Jump | Sfx::Bounce => Tier::Small,
            Sfx::Boom | Sfx::Win | Sfx::LevelUp => Tier::Large,
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
    let win = (RATE as usize / 50).max(1);
    let loudest = x
        .chunks(win)
        .map(|c| (c.iter().map(|v| v * v).sum::<f32>() / c.len() as f32).sqrt())
        .fold(0.0f32, f32::max);
    gain_to_db(loudest)
}

pub fn peak_db(x: &[f32]) -> f32 {
    gain_to_db(x.iter().fold(0.0f32, |m, v| m.max(v.abs())))
}

pub fn normalize(x: &mut [f32], target_db: f32) {
    let k = db_to_gain(target_db - loudness_db(x));
    let ceiling = db_to_gain(PEAK_DB);
    for v in x.iter_mut() {
        let y = *v * k;
        *v = if y.abs() > ceiling * 0.8 {
            let over = (y.abs() - ceiling * 0.8) / (ceiling * 0.2);
            y.signum() * ceiling * (0.8 + 0.2 * over.tanh())
        } else {
            y
        };
    }
}

pub const VARIANTS: [f32; 3] = [0.95, 1.0, 1.05];
pub const LADDER: [f32; 8] = [0.0, 2.0, 4.0, 5.0, 7.0, 9.0, 11.0, 12.0];

pub fn semitones(n: f32) -> f32 {
    2f32.powf(n / 12.0)
}

pub fn repitch(x: &[f32], ratio: f32) -> Vec<f32> {
    let n = ((x.len() as f32) / ratio).floor() as usize;
    (0..n)
        .map(|i| {
            let p = i as f32 * ratio;
            let k = p as usize;
            let f = p - k as f32;
            let a = x.get(k).copied().unwrap_or(0.0);
            let b = x.get(k + 1).copied().unwrap_or(0.0);
            a + (b - a) * f
        })
        .collect()
}

pub fn render_take(s: Sfx, ratio: f32) -> Vec<f32> {
    let mut x = repitch(&render(s), ratio);
    normalize(&mut x, s.tier().loudness_db());
    x
}

#[derive(Clone, Copy)]
enum Wave {
    Sine,
    Square,
    Triangle,
    Noise,
}

struct Voice {
    wave: Wave,
    f0: f32,
    f1: f32,
    start: f32,
    len: f32,
    attack: f32,
    gain: f32,
}

fn voice(wave: Wave, f0: f32, f1: f32, start: f32, len: f32, gain: f32) -> Voice {
    Voice {
        wave,
        f0,
        f1,
        start,
        len,
        attack: 0.004,
        gain,
    }
}

fn notes(wave: Wave, freqs: &[f32], step: f32, len: f32, gain: f32) -> Vec<Voice> {
    freqs
        .iter()
        .enumerate()
        .map(|(i, f)| voice(wave, *f, *f, i as f32 * step, len, gain))
        .collect()
}

fn recipe(s: Sfx) -> Vec<Voice> {
    use Wave::*;
    match s {
        Sfx::Tap => vec![voice(Sine, 880.0, 660.0, 0.0, 0.06, 0.8)],
        Sfx::Click => vec![voice(Square, 1500.0, 1200.0, 0.0, 0.025, 0.5)],
        Sfx::Tick => vec![voice(Sine, 1800.0, 1800.0, 0.0, 0.03, 0.6)],
        Sfx::Pop => vec![
            voice(Sine, 300.0, 1100.0, 0.0, 0.09, 0.9),
            voice(Noise, 0.0, 0.0, 0.0, 0.03, 0.25),
        ],
        Sfx::Coin => vec![
            voice(Square, 988.0, 988.0, 0.0, 0.07, 0.35),
            voice(Square, 1319.0, 1319.0, 0.06, 0.22, 0.35),
        ],
        Sfx::Score => vec![voice(Triangle, 660.0, 990.0, 0.0, 0.12, 0.8)],
        Sfx::Jump => vec![voice(Square, 280.0, 720.0, 0.0, 0.16, 0.3)],
        Sfx::Flap => vec![
            voice(Noise, 0.0, 0.0, 0.0, 0.08, 0.5),
            voice(Sine, 420.0, 600.0, 0.0, 0.07, 0.5),
        ],
        Sfx::Swish => vec![voice(Noise, 0.0, 0.0, 0.0, 0.14, 0.45)],
        Sfx::Slice => vec![
            voice(Noise, 0.0, 0.0, 0.0, 0.1, 0.5),
            voice(Sine, 1200.0, 300.0, 0.0, 0.12, 0.6),
        ],
        Sfx::Bounce => vec![voice(Sine, 180.0, 420.0, 0.0, 0.12, 0.9)],
        Sfx::Hit => vec![
            voice(Square, 220.0, 90.0, 0.0, 0.18, 0.4),
            voice(Noise, 0.0, 0.0, 0.0, 0.08, 0.5),
        ],
        Sfx::Boom => vec![
            voice(Noise, 0.0, 0.0, 0.0, 0.5, 0.9),
            voice(Sine, 120.0, 40.0, 0.0, 0.45, 0.9),
        ],
        Sfx::Wrong => vec![
            voice(Square, 220.0, 196.0, 0.0, 0.14, 0.3),
            voice(Square, 185.0, 165.0, 0.13, 0.2, 0.3),
        ],
        Sfx::Right => notes(Triangle, &[784.0, 1047.0], 0.08, 0.16, 0.7),
        Sfx::Win => notes(
            Triangle,
            &[523.0, 659.0, 784.0, 1047.0, 1319.0],
            0.1,
            0.25,
            0.6,
        ),
        Sfx::LevelUp => notes(Square, &[440.0, 554.0, 659.0, 880.0], 0.07, 0.14, 0.25),
        Sfx::Whoosh => vec![Voice {
            attack: 0.08,
            ..voice(Noise, 0.0, 0.0, 0.0, 0.3, 0.5)
        }],
        Sfx::Splash => vec![
            voice(Noise, 0.0, 0.0, 0.0, 0.3, 0.6),
            voice(Sine, 600.0, 200.0, 0.0, 0.2, 0.4),
        ],
        Sfx::Crunch => vec![
            voice(Noise, 0.0, 0.0, 0.0, 0.12, 0.7),
            voice(Square, 160.0, 80.0, 0.0, 0.1, 0.25),
        ],
    }
}

struct Rng(u32);

impl Rng {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

pub fn render(s: Sfx) -> Vec<f32> {
    let voices = recipe(s);
    let end = voices.iter().map(|v| v.start + v.len).fold(0.0, f32::max);
    let n = (end * RATE as f32).ceil() as usize + 1;
    let mut out = vec![0.0f32; n];
    let mut rng = Rng(0x9e37_79b9 ^ s as u32);
    for v in &voices {
        let mut phase = 0.0f32;
        let mut lp = 0.0f32;
        let first = (v.start * RATE as f32) as usize;
        let count = (v.len * RATE as f32) as usize;
        for k in 0..count {
            let t = k as f32 / RATE as f32;
            let p = t / v.len;
            let f = v.f0 * (v.f1 / v.f0.max(1.0)).max(0.0001).powf(p);
            phase = (phase + f / RATE as f32).fract();
            let raw = match v.wave {
                Wave::Sine => (phase * TAU).sin(),
                Wave::Square => {
                    if phase < 0.5 {
                        1.0
                    } else {
                        -1.0
                    }
                }
                Wave::Triangle => 4.0 * (phase - 0.5).abs() - 1.0,
                Wave::Noise => {
                    lp += (rng.next() - lp) * 0.5;
                    lp
                }
            };
            let attack = (t / v.attack).min(1.0);
            let release = (1.0 - p).powf(1.6);
            out[first + k] += raw * attack * release * v.gain;
        }
    }
    let peak = out.iter().fold(0.0f32, |m, x| m.max(x.abs()));
    if peak > 1.0 {
        for x in &mut out {
            *x /= peak;
        }
    }
    out
}

pub fn wav(samples: &[f32]) -> Vec<u8> {
    let data_len = samples.len() as u32 * 2;
    let mut b = Vec::with_capacity(44 + data_len as usize);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data_len).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&RATE.to_le_bytes());
    b.extend_from_slice(&(RATE * 2).to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        b.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32000.0) as i16).to_le_bytes());
    }
    b
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mood {
    Menu,
    Arcade,
    Learn,
}

impl Mood {
    pub const ALL: [Mood; 3] = [Mood::Menu, Mood::Arcade, Mood::Learn];

    fn bpm(self) -> f32 {
        match self {
            Mood::Menu => 108.0,
            Mood::Arcade => 120.0,
            Mood::Learn => 92.0,
        }
    }

    fn chords(self) -> [[f32; 3]; 4] {
        match self {
            Mood::Menu => [
                [60.0, 64.0, 67.0],
                [55.0, 59.0, 62.0],
                [57.0, 60.0, 64.0],
                [53.0, 57.0, 60.0],
            ],
            Mood::Arcade => [
                [57.0, 60.0, 64.0],
                [53.0, 57.0, 60.0],
                [60.0, 64.0, 67.0],
                [55.0, 59.0, 62.0],
            ],
            Mood::Learn => [
                [53.0, 57.0, 60.0],
                [58.0, 62.0, 65.0],
                [60.0, 64.0, 67.0],
                [53.0, 57.0, 60.0],
            ],
        }
    }

    fn arp(self) -> &'static [usize] {
        match self {
            Mood::Menu => &[0, 1, 2, 1, 0, 2, 1, 2],
            Mood::Arcade => &[0, 2, 1, 2, 0, 2, 1, 2],
            Mood::Learn => &[0, 1, 2, 1],
        }
    }
}

pub const BARS: usize = 4;
pub const MUSIC_DB: f32 = -25.0;

fn midi(n: f32) -> f32 {
    440.0 * semitones(n - 69.0)
}

pub fn loop_len(m: Mood) -> usize {
    let beat = 60.0 / m.bpm();
    (beat * 4.0 * BARS as f32 * RATE as f32).round() as usize
}

fn pluck(out: &mut [f32], at: usize, freq: f32, len: f32, gain: f32, wave: Wave) {
    let n = (len * RATE as f32) as usize;
    let mut phase = 0.0f32;
    for k in 0..n {
        let i = at + k;
        if i >= out.len() {
            break;
        }
        let t = k as f32 / RATE as f32;
        phase = (phase + freq / RATE as f32).fract();
        let raw = match wave {
            Wave::Triangle => 4.0 * (phase - 0.5).abs() - 1.0,
            _ => (phase * TAU).sin() + 0.25 * (phase * TAU * 2.0).sin(),
        };
        let env = (t / 0.008).min(1.0) * (1.0 - k as f32 / n as f32).powf(2.2);
        out[i] += raw * env * gain;
    }
}

pub fn render_music(m: Mood) -> Vec<f32> {
    let len = loop_len(m);
    let mut out = vec![0.0f32; len];
    let beat = 60.0 / m.bpm();
    let arp = m.arp();
    let step = beat * 4.0 / arp.len() as f32;
    let mut rng = Rng(0x2545_f491 ^ m as u32);
    for (bar, chord) in m.chords().iter().enumerate() {
        let bar_at = bar as f32 * beat * 4.0;
        for half in 0..2 {
            let at = ((bar_at + half as f32 * beat * 2.0) * RATE as f32) as usize;
            pluck(
                &mut out,
                at,
                midi(chord[0] - 24.0),
                beat * 1.8,
                0.55,
                Wave::Sine,
            );
        }
        for (i, idx) in arp.iter().enumerate() {
            let at = ((bar_at + i as f32 * step) * RATE as f32) as usize;
            let note = chord[*idx] + if m == Mood::Learn { 0.0 } else { 12.0 };
            pluck(&mut out, at, midi(note), step * 1.6, 0.22, Wave::Triangle);
        }
        if m == Mood::Arcade {
            for b in 0..8 {
                let at = ((bar_at + b as f32 * beat / 2.0) * RATE as f32) as usize;
                let n = (0.03 * RATE as f32) as usize;
                let mut lp = 0.0;
                for k in 0..n {
                    if at + k >= len {
                        break;
                    }
                    lp += (rng.next() - lp) * 0.7;
                    let hp = rng.next() - lp;
                    let g = if b % 2 == 1 { 0.12 } else { 0.06 };
                    out[at + k] += hp * g * (1.0 - k as f32 / n as f32).powi(3);
                }
            }
        }
    }
    let fade = (0.01 * RATE as f32) as usize;
    for k in 0..fade {
        let g = k as f32 / fade as f32;
        out[k] *= g;
        out[len - 1 - k] *= g;
    }
    normalize(&mut out, MUSIC_DB);
    out
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

type Sound = macroquad::audio::Sound;

struct Bank {
    takes: Vec<(Sfx, Vec<Sound>)>,
    ladders: Vec<(Sfx, Vec<Sound>)>,
    music: Vec<(Mood, Sound)>,
    limiter: Limiter,
    mix: Mix,
    playing: Option<Mood>,
    last_take: u32,
}

thread_local! {
    static BANK: RefCell<Option<Bank>> = const { RefCell::new(None) };
    static PENDING: RefCell<Vec<Cue>> = const { RefCell::new(Vec::new()) };
    static WANT: std::cell::Cell<Option<Mood>> = const { std::cell::Cell::new(None) };
    static SFX_ON: std::cell::Cell<bool> = const { std::cell::Cell::new(true) };
    static MUSIC_ON: std::cell::Cell<bool> = const { std::cell::Cell::new(true) };
}

async fn sound(x: &[f32]) -> Option<Sound> {
    macroquad::audio::load_sound_from_bytes(&wav(x)).await.ok()
}

pub async fn load() {
    let mut takes = vec![];
    let mut ladders = vec![];
    for s in Sfx::ALL {
        let mut v = vec![];
        for r in VARIANTS {
            v.extend(sound(&render_take(s, r)).await);
        }
        takes.push((s, v));
        if s.ladder() {
            let mut l = vec![];
            for n in LADDER {
                l.extend(sound(&render_take(s, semitones(n))).await);
            }
            ladders.push((s, l));
        }
    }
    BANK.with(|b| {
        *b.borrow_mut() = Some(Bank {
            takes,
            ladders,
            music: vec![],
            limiter: Limiter::default(),
            mix: Mix::default(),
            playing: None,
            last_take: 0,
        })
    });
}

pub async fn load_music() {
    for m in Mood::ALL {
        let Some(snd) = sound(&render_music(m)).await else {
            continue;
        };
        BANK.with(|b| {
            if let Some(bank) = b.borrow_mut().as_mut() {
                bank.music.push((m, snd));
                if bank.playing == Some(m) {
                    bank.playing = None;
                }
            }
        });
        macroquad::window::next_frame().await;
    }
}

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

fn sfx_gain() -> f32 {
    db_to_gain(SFX_BUS_DB)
}

pub fn flush(now: f64, dt: f32) {
    let cues = take_cues();
    let sfx_on = SFX_ON.with(|s| s.get());
    let want = if MUSIC_ON.with(|m| m.get()) {
        wanted_music()
    } else {
        None
    };
    BANK.with(|b| {
        let mut b = b.borrow_mut();
        let Some(bank) = b.as_mut() else { return };
        for c in cues {
            if !sfx_on || !bank.limiter.allow(c.sfx, now) {
                continue;
            }
            let snd = match c.pitch {
                Pitch::Step(n) => bank
                    .ladders
                    .iter()
                    .find(|(k, _)| *k == c.sfx)
                    .and_then(|(_, l)| l.get(n)),
                Pitch::Vary => None,
            }
            .or_else(|| {
                bank.takes
                    .iter()
                    .find(|(k, _)| *k == c.sfx)
                    .and_then(|(_, v)| {
                        bank.last_take = bank
                            .last_take
                            .wrapping_mul(1_103_515_245)
                            .wrapping_add(12_345);
                        v.get((bank.last_take >> 16) as usize % v.len().max(1))
                    })
            });
            if let Some(snd) = snd {
                macroquad::audio::play_sound(
                    snd,
                    macroquad::audio::PlaySoundParams {
                        looped: false,
                        volume: sfx_gain(),
                    },
                );
                if c.sfx.tier().ducks_music() {
                    bank.mix.ducked();
                }
            }
        }
        bank.mix.step(dt);
        if bank.playing != want {
            if let Some(old) = bank
                .playing
                .and_then(|m| bank.music.iter().find(|(k, _)| *k == m))
            {
                macroquad::audio::stop_sound(&old.1);
            }
            bank.playing = want;
            bank.mix.fade_in = 0.0;
            if let Some(new) = want.and_then(|m| bank.music.iter().find(|(k, _)| *k == m)) {
                macroquad::audio::play_sound(
                    &new.1,
                    macroquad::audio::PlaySoundParams {
                        looped: true,
                        volume: 0.0,
                    },
                );
            }
        }
        if let Some(cur) = bank
            .playing
            .and_then(|m| bank.music.iter().find(|(k, _)| *k == m))
        {
            macroquad::audio::set_sound_volume(&cur.1, bank.mix.music_gain());
        }
    });
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

    #[test]
    fn every_sound_renders_audible_and_short() {
        for s in Sfx::ALL {
            let x = render(s);
            let secs = x.len() as f32 / RATE as f32;
            assert!(secs > 0.02 && secs < 0.8, "{s:?} is {secs}s");
            let peak = x.iter().fold(0.0f32, |m, v| m.max(v.abs()));
            assert!(peak > 0.15 && peak <= 1.0, "{s:?} peak {peak}");
            assert!(x.iter().all(|v| v.is_finite()));
        }
    }

    #[test]
    fn sounds_fade_out_without_a_click() {
        for s in Sfx::ALL {
            let x = render(s);
            let tail = &x[x.len().saturating_sub(40)..];
            let loud = tail.iter().fold(0.0f32, |m, v| m.max(v.abs()));
            assert!(loud < 0.05, "{s:?} ends at {loud}");
        }
    }

    #[test]
    fn wav_header_is_valid_pcm() {
        let b = wav(&render(Sfx::Pop));
        assert_eq!(&b[0..4], b"RIFF");
        assert_eq!(&b[8..16], b"WAVEfmt ");
        assert_eq!(
            u32::from_le_bytes(b[4..8].try_into().unwrap()) as usize,
            b.len() - 8
        );
        assert_eq!(u32::from_le_bytes(b[24..28].try_into().unwrap()), RATE);
        assert_eq!(
            u32::from_le_bytes(b[40..44].try_into().unwrap()) as usize,
            b.len() - 44
        );
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
    fn rendering_every_take_and_loop_is_quick() {
        let t = std::time::Instant::now();
        for s in Sfx::ALL {
            for r in VARIANTS {
                render_take(s, r);
            }
            if s.ladder() {
                for n in LADDER {
                    render_take(s, semitones(n));
                }
            }
        }
        let sfx = t.elapsed();
        for m in Mood::ALL {
            render_music(m);
        }
        let all = t.elapsed();
        let limit = if cfg!(debug_assertions) { 3.0 } else { 0.25 };
        assert!(sfx.as_secs_f32() < limit, "effects took {sfx:?}");
        assert!(all.as_secs_f32() < limit * 2.0, "music took {all:?}");
        eprintln!("effects {sfx:?}, with music {all:?}");
    }

    #[test]
    #[ignore]
    fn dump_wavs() {
        let dir = std::env::var("SFX_DIR").unwrap_or("/tmp/sfx".into());
        std::fs::create_dir_all(&dir).unwrap();
        for s in Sfx::ALL {
            std::fs::write(format!("{dir}/{s:?}.wav"), wav(&render_take(s, 1.0))).unwrap();
        }
        for m in Mood::ALL {
            let x = render_music(m);
            let twice: Vec<f32> = x.iter().chain(&x).copied().collect();
            std::fs::write(format!("{dir}/music-{m:?}.wav"), wav(&twice)).unwrap();
        }
    }

    #[test]
    fn tiers_land_at_their_loudness_with_headroom() {
        for s in Sfx::ALL {
            for r in VARIANTS {
                let x = render_take(s, r);
                let l = loudness_db(&x);
                let want = s.tier().loudness_db();
                assert!((l - want).abs() < 2.5, "{s:?}@{r} at {l} dB, want {want}");
                assert!(
                    peak_db(&x) <= PEAK_DB + 0.01,
                    "{s:?} peaks at {}",
                    peak_db(&x)
                );
            }
        }
    }

    #[test]
    fn ui_ticks_sit_under_big_moments() {
        let l = |s| loudness_db(&render_take(s, 1.0));
        assert!(l(Sfx::Tick) + 6.0 < l(Sfx::Win));
        assert!(l(Sfx::Tap) < l(Sfx::Pop));
        assert!(l(Sfx::Pop) < l(Sfx::Boom));
    }

    #[test]
    fn ladder_steps_rise_in_pitch() {
        let base = render(Sfx::Pop).len() as f32;
        let mut prev = f32::MAX;
        for n in LADDER {
            let len = repitch(&render(Sfx::Pop), semitones(n)).len() as f32;
            assert!(len < prev, "step {n} not higher");
            prev = len;
        }
        assert!((base / prev - 2.0).abs() < 0.02, "top step is an octave up");
    }

    #[test]
    fn music_loops_are_whole_bars_and_seamless() {
        for m in Mood::ALL {
            let x = render_music(m);
            assert_eq!(x.len(), loop_len(m));
            let beat = (60.0 / m.bpm() * RATE as f32) as usize;
            assert_eq!(x.len() / beat, 16, "{m:?} is not four bars");
            assert!(
                x[0].abs() < 0.01 && x[x.len() - 1].abs() < 0.01,
                "{m:?} clicks at the seam"
            );
            assert!(x.iter().all(|v| v.is_finite()));
        }
    }

    #[test]
    fn music_sits_well_under_effects() {
        for m in Mood::ALL {
            let music = loudness_db(&render_music(m)) + MUSIC_BUS_DB;
            assert!(
                music + 6.0 <= Tier::Small.loudness_db(),
                "{m:?} music at {music} dB crowds the effects"
            );
        }
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
    fn play_queues_until_flushed() {
        take_pending();
        play(Sfx::Tap);
        play(Sfx::Win);
        play(Sfx::Tap);
        assert_eq!(take_pending(), vec![Sfx::Tap, Sfx::Win]);
        assert!(take_pending().is_empty());
    }
}
