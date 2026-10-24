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

    pub fn volume(self) -> f32 {
        match self {
            Sfx::Tick | Sfx::Click => 0.35,
            Sfx::Swish | Sfx::Whoosh | Sfx::Flap => 0.45,
            Sfx::Boom => 0.8,
            _ => 0.6,
        }
    }
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

struct Bank {
    sounds: Vec<(Sfx, macroquad::audio::Sound)>,
    limiter: Limiter,
    muted: bool,
}

thread_local! {
    static BANK: RefCell<Option<Bank>> = const { RefCell::new(None) };
    static PENDING: RefCell<Vec<Sfx>> = const { RefCell::new(Vec::new()) };
}

pub async fn load() {
    let mut sounds = vec![];
    for s in Sfx::ALL {
        if let Ok(snd) = macroquad::audio::load_sound_from_bytes(&wav(&render(s))).await {
            sounds.push((s, snd));
        }
    }
    BANK.with(|b| {
        *b.borrow_mut() = Some(Bank {
            sounds,
            limiter: Limiter::default(),
            muted: false,
        })
    });
}

const MAX_PENDING: usize = 16;

pub fn play(s: Sfx) {
    PENDING.with(|p| {
        let mut p = p.borrow_mut();
        if p.len() < MAX_PENDING && !p.contains(&s) {
            p.push(s);
        }
    });
}

pub fn take_pending() -> Vec<Sfx> {
    PENDING.with(|p| std::mem::take(&mut *p.borrow_mut()))
}

pub fn flush(now: f64) {
    let pending = take_pending();
    BANK.with(|b| {
        let mut b = b.borrow_mut();
        let Some(bank) = b.as_mut() else { return };
        if bank.muted {
            return;
        }
        for s in pending {
            if !bank.limiter.allow(s, now) {
                continue;
            }
            if let Some((_, snd)) = bank.sounds.iter().find(|(k, _)| *k == s) {
                macroquad::audio::play_sound(
                    snd,
                    macroquad::audio::PlaySoundParams {
                        looped: false,
                        volume: s.volume(),
                    },
                );
            }
        }
    });
}

pub fn muted() -> bool {
    BANK.with(|b| b.borrow().as_ref().is_some_and(|b| b.muted))
}

pub fn toggle_mute() {
    BANK.with(|b| {
        if let Some(b) = b.borrow_mut().as_mut() {
            b.muted = !b.muted;
        }
    });
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
    #[ignore]
    fn dump_wavs() {
        let dir = std::env::var("SFX_DIR").unwrap_or("/tmp/sfx".into());
        std::fs::create_dir_all(&dir).unwrap();
        for s in Sfx::ALL {
            std::fs::write(format!("{dir}/{s:?}.wav"), wav(&render(s))).unwrap();
        }
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
