use crate::sfx::{
    self, db_to_gain, decode, find, ladder_of, pick_take, takes_of, voice_line, Cue, Limiter, Mix,
    Mood, Pitch, Sfx, SFX_BUS_DB, VOICE_GAP,
};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

pub const MAX_VOICES: usize = 24;
const MUSIC_RAMP_PER_SEC: f32 = 6.0;

static NEXT_CLIP: AtomicU32 = AtomicU32::new(1);

#[derive(Clone, Debug)]
pub struct Clip {
    pub id: u32,
    pub rate: u32,
    pub data: Arc<Vec<f32>>,
}

impl Clip {
    pub fn new(rate: u32, data: Vec<f32>) -> Clip {
        Clip {
            id: NEXT_CLIP.fetch_add(1, Ordering::Relaxed),
            rate,
            data: Arc::new(data),
        }
    }

    pub fn from_ogg(ogg: &[u8]) -> Option<Clip> {
        let (rate, pcm) = decode(ogg)?;
        Some(Clip::new(
            rate,
            pcm.iter().map(|s| *s as f32 / 32768.0).collect(),
        ))
    }

    pub fn secs(&self) -> f32 {
        self.data.len() as f32 / self.rate.max(1) as f32
    }
}

#[derive(Default)]
pub struct Bank {
    pub takes: HashMap<Sfx, Vec<Clip>>,
    pub ladders: HashMap<Sfx, Vec<Clip>>,
    pub voices: HashMap<String, Vec<Clip>>,
    pub music: HashMap<Mood, Clip>,
}

impl Bank {
    pub fn effects(pack: &[(&str, &[u8])]) -> Bank {
        let mut b = Bank::default();
        for s in Sfx::ALL {
            let takes: Vec<Clip> = takes_of(pack, s)
                .into_iter()
                .filter_map(Clip::from_ogg)
                .collect();
            b.takes.insert(s, takes);
            let ladder: Vec<Clip> = ladder_of(pack, s)
                .into_iter()
                .filter_map(Clip::from_ogg)
                .collect();
            if !ladder.is_empty() {
                b.ladders.insert(s, ladder);
            }
        }
        b
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Later {
    Music(Mood),
    Voice(String),
}

pub fn later_order(pack: &[(&str, &[u8])], first: Mood) -> Vec<(Later, usize)> {
    let index = |key: &str| pack.iter().position(|(k, _)| *k == key);
    let mut out = vec![];
    let mut moods = vec![first];
    moods.extend(Mood::ALL.iter().copied().filter(|m| *m != first));
    if let Some(i) = index(first.key()) {
        out.push((Later::Music(first), i));
    }
    for (i, (k, _)) in pack.iter().enumerate() {
        if k.starts_with("say_") {
            out.push((Later::Voice(voice_line(k).to_string()), i));
        }
    }
    for m in &moods[1..] {
        if let Some(i) = index(m.key()) {
            out.push((Later::Music(*m), i));
        }
    }
    out
}

#[derive(Clone, Debug)]
pub enum Cmd {
    Start {
        clip: Clip,
        gain: f32,
        music: bool,
        keep: bool,
    },
    StopMusic,
    MusicGain(f32),
}

#[derive(Default)]
pub struct Words {
    want: Vec<String>,
    clips: HashMap<String, Clip>,
    pub decoded: usize,
}

impl Words {
    pub fn set(&mut self, keys: Vec<String>) {
        self.clips.retain(|k, _| keys.contains(k));
        self.want = keys;
    }

    pub fn ready(&self, key: &str) -> bool {
        self.clips.contains_key(key)
    }

    pub fn missing(&self) -> Option<&str> {
        self.want
            .iter()
            .find(|k| !self.clips.contains_key(*k))
            .map(|k| k.as_str())
    }

    pub fn decode(key: &str) -> Option<Clip> {
        find(sfx::words(), key).and_then(Clip::from_ogg)
    }

    pub fn insert(&mut self, key: String, clip: Clip) {
        self.decoded += 1;
        if !self.want.contains(&key) {
            self.want.push(key.clone());
        }
        self.clips.insert(key, clip);
    }

    pub fn len(&self) -> usize {
        self.clips.len()
    }

    pub fn is_empty(&self) -> bool {
        self.clips.is_empty()
    }
}

struct Voice {
    clip: Clip,
    pos: f64,
    step: f64,
    gain: f32,
    music: bool,
}

pub struct Mixer {
    pub bank: Bank,
    pub out_rate: u32,
    limiter: Limiter,
    pub mix: Mix,
    playing: Option<Mood>,
    voices: Vec<Voice>,
    last_voice: HashMap<String, usize>,
    roll: u32,
    said_at: f64,
    music_now: f32,
    music_target: f32,
    music_live: bool,
    pub words: Words,
    waiting: Option<String>,
    pub remote: Option<Vec<Cmd>>,
    sent_gain: f32,
    pub played: Vec<String>,
    pub frames_rendered: u64,
}

impl Mixer {
    pub fn new(bank: Bank, out_rate: u32) -> Self {
        Self {
            bank,
            out_rate: out_rate.max(1),
            limiter: Limiter::default(),
            mix: Mix::default(),
            playing: None,
            voices: vec![],
            last_voice: HashMap::new(),
            roll: 0x2545_f491,
            said_at: -10.0,
            music_now: 0.0,
            music_target: 0.0,
            music_live: false,
            words: Words::default(),
            waiting: None,
            remote: None,
            sent_gain: -1.0,
            played: vec![],
            frames_rendered: 0,
        }
    }

    fn next_roll(&mut self) -> usize {
        self.roll = self.roll.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        (self.roll >> 16) as usize
    }

    pub fn remote() -> Self {
        let mut m = Self::new(Bank::default(), 48000);
        m.remote = Some(vec![]);
        m
    }

    pub fn take_cmds(&mut self) -> Vec<Cmd> {
        self.remote.as_mut().map(std::mem::take).unwrap_or_default()
    }

    pub fn run(&mut self, cmd: Cmd) {
        match cmd {
            Cmd::Start {
                clip,
                gain,
                music,
                keep,
            } => self.start(clip, gain, music, keep),
            Cmd::StopMusic => self.stop_music(),
            Cmd::MusicGain(g) => self.music_target = g,
        }
    }

    fn stop_music(&mut self) {
        self.music_live = false;
        match &mut self.remote {
            Some(out) => out.push(Cmd::StopMusic),
            None => self.voices.retain(|v| !v.music),
        }
    }

    fn start(&mut self, clip: Clip, gain: f32, music: bool, keep: bool) {
        if music {
            self.music_live = true;
        }
        if let Some(out) = &mut self.remote {
            out.push(Cmd::Start {
                clip,
                gain,
                music,
                keep,
            });
            return;
        }
        if music {
            self.voices.retain(|v| !v.music);
        } else if self.voices.iter().filter(|v| !v.music).count() >= MAX_VOICES {
            if let Some(i) = self.voices.iter().position(|v| !v.music) {
                self.voices.remove(i);
            }
        }
        let step = clip.rate as f64 / self.out_rate as f64;
        self.voices.push(Voice {
            clip,
            pos: 0.0,
            step,
            gain,
            music,
        });
    }

    pub fn control(&mut self, now: f64, dt: f32) {
        let cues = sfx::take_cues();
        let voice = sfx::take_voice();
        if let Some(w) = sfx::take_prefetch() {
            self.words
                .set(w.into_iter().map(|w| sfx::Say::Word(w).key()).collect());
        }
        let sfx_on = !sfx::muted();
        let want = if sfx::music_on() {
            sfx::wanted_music()
        } else {
            None
        };
        self.apply(&cues, voice, want, sfx_on, now, dt);
    }

    pub fn apply(
        &mut self,
        cues: &[Cue],
        voice: Option<sfx::Say>,
        want: Option<Mood>,
        sfx_on: bool,
        now: f64,
        dt: f32,
    ) {
        let gain = db_to_gain(SFX_BUS_DB);
        for c in cues {
            if !sfx_on || !self.limiter.allow(c.sfx, now) {
                continue;
            }
            let stepped = match c.pitch {
                Pitch::Step(n) => self
                    .bank
                    .ladders
                    .get(&c.sfx)
                    .and_then(|l| l.get(n))
                    .cloned(),
                Pitch::Vary => None,
            };
            let clip = stepped.or_else(|| {
                let n = self.bank.takes.get(&c.sfx).map_or(0, |t| t.len());
                let roll = self.next_roll();
                self.bank
                    .takes
                    .get(&c.sfx)
                    .and_then(|t| t.get(roll % n.max(1)))
                    .cloned()
            });
            if let Some(clip) = clip {
                self.start(clip, gain, false, true);
                self.played.push(c.sfx.key().to_string());
                if c.sfx.tier().ducks_music() {
                    self.mix.ducked();
                }
            }
        }
        if let Some(v) = voice.filter(|_| sfx_on && now - self.said_at > VOICE_GAP) {
            let key = v.key();
            if key.starts_with("word_") {
                self.waiting = Some(key);
                self.said_at = now;
            } else {
                let last = self.last_voice.get(&key).copied();
                let roll = self.next_roll();
                let pick = self.bank.voices.get(&key).map(|takes| {
                    let i = pick_take(takes.len(), last, roll);
                    (i, takes[i].clone())
                });
                if let Some((i, clip)) = pick {
                    self.waiting = None;
                    self.last_voice.insert(key.clone(), i);
                    self.start(clip, gain, false, true);
                    self.played.push(format!("{key}.{i}"));
                    self.said_at = now;
                    self.mix.ducked();
                }
            }
        }
        self.speak_waiting(sfx_on);
        self.mix.step(dt);
        let have = |m: Option<Mood>| m.is_some_and(|m| self.bank.music.contains_key(&m));
        let missing = want.is_some() && !self.music_live && have(want);
        if self.playing != want || missing {
            if self.music_live {
                self.stop_music();
            }
            self.playing = want;
            self.mix.fade_in = 0.0;
            if let Some(clip) = want.and_then(|m| self.bank.music.get(&m)).cloned() {
                self.start(clip, 1.0, true, true);
                self.music_now = 0.0;
            }
        }
        self.music_target = if self.playing.is_some() {
            self.mix.music_gain()
        } else {
            0.0
        };
        if let Some(out) = &mut self.remote {
            if (self.music_target - self.sent_gain).abs() > 1e-4 {
                self.sent_gain = self.music_target;
                out.push(Cmd::MusicGain(self.music_target));
            }
        }
    }

    fn speak_waiting(&mut self, sfx_on: bool) {
        let Some(key) = self.waiting.take() else {
            return;
        };
        if !sfx_on {
            return;
        }
        match self.words.clips.get(&key).cloned() {
            Some(clip) => {
                self.start(clip, db_to_gain(SFX_BUS_DB), false, false);
                self.played.push(format!("{key}.0"));
                self.mix.ducked();
            }
            None => {
                if !self.words.want.contains(&key) {
                    self.words.want.insert(0, key.clone());
                }
                self.waiting = Some(key);
            }
        }
    }

    pub fn next_word(&self) -> Option<String> {
        self.waiting
            .clone()
            .filter(|k| !self.words.ready(k))
            .or_else(|| self.words.missing().map(str::to_string))
    }

    pub fn word_decoded(&mut self, key: String, clip: Option<Clip>) {
        match clip {
            Some(c) => self.words.insert(key, c),
            None => {
                self.words.want.retain(|k| *k != key);
                if self.waiting.as_ref() == Some(&key) {
                    self.waiting = None;
                }
            }
        }
        let on = !sfx::muted();
        self.speak_waiting(on);
    }

    pub fn word_waiting(&self) -> bool {
        self.waiting.is_some()
    }

    pub fn playing_mood(&self) -> Option<Mood> {
        self.playing.filter(|_| self.music_live)
    }

    pub fn active_effects(&self) -> usize {
        self.voices.iter().filter(|v| !v.music).count()
    }

    pub fn music_gain(&self) -> f32 {
        self.music_now
    }

    pub fn render(&mut self, out: &mut [f32], channels: usize) {
        let channels = channels.max(1);
        out.fill(0.0);
        let frames = out.len() / channels;
        self.frames_rendered += frames as u64;
        let ramp = MUSIC_RAMP_PER_SEC / self.out_rate as f32;
        let target = self.music_target;
        let mut music = self.music_now;
        for v in &mut self.voices {
            let data = &v.clip.data;
            let len = data.len();
            if len == 0 {
                continue;
            }
            let mut m = music;
            for f in 0..frames {
                if v.pos >= len as f64 {
                    if v.music {
                        v.pos -= len as f64;
                    } else {
                        break;
                    }
                }
                let i = v.pos as usize;
                let t = (v.pos - i as f64) as f32;
                let a = data[i];
                let b = if i + 1 < len {
                    data[i + 1]
                } else if v.music {
                    data[0]
                } else {
                    0.0
                };
                let g = if v.music {
                    m += (target - m).clamp(-ramp, ramp);
                    m
                } else {
                    v.gain
                };
                let s = (a + (b - a) * t) * g;
                for c in 0..channels {
                    out[f * channels + c] += s;
                }
                v.pos += v.step;
            }
            if v.music {
                music = m;
            }
        }
        if !self.voices.iter().any(|v| v.music) {
            music = target;
        }
        self.music_now = music;
        self.voices
            .retain(|v| v.music || (v.pos as usize) < v.clip.data.len());
        for s in out.iter_mut() {
            *s = s.clamp(-1.0, 1.0);
        }
    }
}

pub struct Loader {
    pack: Vec<(&'static str, &'static [u8])>,
    order: Vec<(Later, usize)>,
    next: usize,
}

impl Loader {
    pub fn new(first: Mood) -> Self {
        let pack = sfx::packed().to_vec();
        let order = later_order(&pack, first);
        Self {
            pack,
            order,
            next: 0,
        }
    }

    pub fn done(&self) -> bool {
        self.next >= self.order.len()
    }

    pub fn step(&mut self) -> Option<(Later, Clip)> {
        let (what, i) = self.order.get(self.next)?.clone();
        self.next += 1;
        let clip = Clip::from_ogg(self.pack[i].1)?;
        Some((what, clip))
    }

    pub fn store(bank: &mut Bank, what: Later, clip: Clip) {
        match what {
            Later::Music(m) => {
                bank.music.insert(m, clip);
            }
            Later::Voice(line) => bank.voices.entry(line).or_default().push(clip),
        }
    }
}

pub fn effects_bank() -> Bank {
    Bank::effects(sfx::packed())
}

pub fn music_clip(m: Mood) -> Option<Clip> {
    Clip::from_ogg(find(sfx::packed(), m.key())?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sfx::{Say, DUCK_DB};

    fn tone(len: usize, v: f32) -> Clip {
        Clip::new(44100, vec![v; len])
    }

    fn bank() -> Bank {
        let mut b = Bank::default();
        for s in Sfx::ALL {
            b.takes.insert(s, vec![tone(441, 0.1), tone(441, 0.1)]);
        }
        b.voices.insert(
            "say_great".into(),
            (0..4).map(|_| tone(4410, 0.1)).collect(),
        );
        b.music.insert(Mood::Menu, tone(44100, 0.2));
        b.music.insert(Mood::Arcade, tone(44100, 0.3));
        b
    }

    fn cue(s: Sfx) -> Cue {
        Cue {
            sfx: s,
            pitch: Pitch::Vary,
        }
    }

    fn settle(m: &mut Mixer, want: Option<Mood>, secs: f32) {
        let mut buf = vec![0.0; 441 * 2];
        for _ in 0..(secs * 100.0) as usize {
            m.apply(&[], None, want, true, 0.0, 0.01);
            m.render(&mut buf, 2);
        }
    }

    #[test]
    fn music_fades_in_and_ducks_under_a_large_sound_then_recovers() {
        let mut m = Mixer::new(bank(), 44100);
        settle(&mut m, Some(Mood::Menu), 1.0);
        let full = m.music_gain();
        assert!(
            (full
                - Mix {
                    fade_in: 1.0,
                    ..Default::default()
                }
                .music_gain())
            .abs()
                < 1e-3
        );
        m.apply(&[cue(Sfx::Win)], None, Some(Mood::Menu), true, 1.0, 0.01);
        let mut buf = vec![0.0; 441 * 2];
        for _ in 0..20 {
            m.render(&mut buf, 2);
            m.apply(&[], None, Some(Mood::Menu), true, 1.0, 0.01);
        }
        let ducked = sfx::gain_to_db(m.music_gain() / full);
        assert!((ducked - DUCK_DB).abs() < 0.5, "ducked {ducked} dB");
        settle(&mut m, Some(Mood::Menu), 1.5);
        assert!((m.music_gain() - full).abs() < 1e-3);
    }

    #[test]
    fn small_sounds_do_not_duck_the_music() {
        let mut m = Mixer::new(bank(), 44100);
        settle(&mut m, Some(Mood::Menu), 1.0);
        let full = m.music_gain();
        m.apply(&[cue(Sfx::Flap)], None, Some(Mood::Menu), true, 1.0, 0.01);
        settle(&mut m, Some(Mood::Menu), 0.2);
        assert!((m.music_gain() - full).abs() < 1e-3);
    }

    #[test]
    fn a_voice_line_ducks_and_never_repeats_its_last_take() {
        let mut m = Mixer::new(bank(), 44100);
        let mut last = None;
        for i in 0..30 {
            m.played.clear();
            m.apply(&[], Some(Say::Great), None, true, i as f64 * 2.0, 0.01);
            let took = m.played.last().cloned().unwrap();
            assert_ne!(Some(&took), last.as_ref());
            last = Some(took);
        }
        assert!(m.mix.duck_db() < 0.0);
    }

    #[test]
    fn voice_lines_closer_than_the_gap_are_dropped() {
        let mut m = Mixer::new(bank(), 44100);
        m.apply(&[], Some(Say::Great), None, true, 1.0, 0.01);
        m.apply(&[], Some(Say::Great), None, true, 1.2, 0.01);
        assert_eq!(m.played.len(), 1);
    }

    #[test]
    fn the_same_effect_twice_in_one_moment_plays_once() {
        let mut m = Mixer::new(bank(), 44100);
        m.apply(&[cue(Sfx::Pop)], None, None, true, 1.0, 0.01);
        m.apply(&[cue(Sfx::Pop)], None, None, true, 1.01, 0.01);
        m.apply(&[cue(Sfx::Pop)], None, None, true, 1.1, 0.01);
        assert_eq!(m.played.len(), 2);
    }

    #[test]
    fn muted_effects_are_silent_and_music_off_stops_the_loop() {
        let mut m = Mixer::new(bank(), 44100);
        m.apply(&[cue(Sfx::Pop)], Some(Say::Great), None, false, 1.0, 0.01);
        assert_eq!(m.active_effects(), 0);
        settle(&mut m, Some(Mood::Menu), 0.5);
        assert_eq!(m.playing_mood(), Some(Mood::Menu));
        settle(&mut m, None, 0.5);
        assert_eq!(m.playing_mood(), None);
        let mut buf = vec![1.0; 64];
        m.render(&mut buf, 2);
        assert!(buf.iter().all(|s| *s == 0.0));
    }

    #[test]
    fn switching_mood_swaps_the_loop_and_fades_it_in() {
        let mut m = Mixer::new(bank(), 44100);
        settle(&mut m, Some(Mood::Menu), 1.0);
        m.apply(&[], None, Some(Mood::Arcade), true, 0.0, 0.01);
        assert_eq!(m.playing_mood(), Some(Mood::Arcade));
        assert!(m.music_gain() < 0.05);
        settle(&mut m, Some(Mood::Arcade), 1.0);
        assert!(m.music_gain() > 0.5);
    }

    #[test]
    fn music_that_arrives_late_starts_once_it_is_loaded() {
        let mut b = bank();
        let late = b.music.remove(&Mood::Arcade).unwrap();
        let mut m = Mixer::new(b, 44100);
        settle(&mut m, Some(Mood::Arcade), 0.2);
        assert_eq!(m.playing_mood(), None);
        m.bank.music.insert(Mood::Arcade, late);
        settle(&mut m, Some(Mood::Arcade), 0.2);
        assert_eq!(m.playing_mood(), Some(Mood::Arcade));
    }

    #[test]
    fn effects_end_and_are_dropped_while_music_loops() {
        let mut m = Mixer::new(bank(), 48000);
        settle(&mut m, Some(Mood::Menu), 1.0);
        m.apply(&[cue(Sfx::Pop)], None, Some(Mood::Menu), true, 1.0, 0.01);
        assert_eq!(m.active_effects(), 1);
        let mut buf = vec![0.0; 2048];
        m.render(&mut buf, 2);
        assert!(buf.iter().any(|s| *s > 0.15));
        assert_eq!(m.frames_rendered, 1024 + 441 * 100);
        m.render(&mut buf, 2);
        assert_eq!(m.active_effects(), 0);
        for _ in 0..200 {
            m.render(&mut buf, 2);
        }
        assert_eq!(m.playing_mood(), Some(Mood::Menu));
    }

    #[test]
    fn a_burst_of_effects_is_capped_and_the_mix_never_clips() {
        let mut m = Mixer::new(bank(), 44100);
        for (i, s) in Sfx::ALL.iter().cycle().take(80).enumerate() {
            m.apply(&[cue(*s)], None, None, true, i as f64, 0.0);
        }
        assert!(m.active_effects() <= MAX_VOICES);
        let mut buf = vec![0.0; 256];
        m.render(&mut buf, 1);
        assert!(buf.iter().all(|s| s.abs() <= 1.0));
    }

    fn decode_ahead(m: &mut Mixer) {
        while let Some(key) = m.next_word() {
            let clip = Words::decode(&key);
            m.word_decoded(key, clip);
        }
    }

    #[test]
    fn speaking_a_spell_word_never_decodes_on_the_spot() {
        use crate::games::spell::{self, Spell};
        let mut m = Mixer::new(bank(), 44100);
        crate::save::reset_for_test();
        sfx::take_voice();
        sfx::take_prefetch();
        let deck = spell::level_words(1, &[]);
        let mut g = Spell::with_level_deck(1, deck.clone());
        m.control(0.0, 0.01);
        assert_eq!(m.words.decoded, 0);
        decode_ahead(&mut m);
        assert_eq!(m.words.decoded, deck.len());
        assert!(deck.iter().all(|w| m.words.ready(&Say::Word(w).key())));
        assert_eq!(m.played, vec![format!("{}.0", Say::Word(deck[0]).key())]);
        for (i, w) in deck.iter().enumerate().skip(1) {
            m.played.clear();
            g.index = i;
            g.say_word();
            m.control(i as f64 * 2.0, 0.01);
            assert_eq!(m.played, vec![format!("{}.0", Say::Word(w).key())]);
            assert_eq!(m.words.decoded, deck.len(), "{w} was decoded when spoken");
        }
    }

    #[test]
    fn a_new_level_keeps_only_its_own_words_decoded() {
        let mut m = Mixer::new(bank(), 44100);
        sfx::prefetch_words(&["cat", "dog", "sun"]);
        m.control(0.0, 0.01);
        decode_ahead(&mut m);
        assert_eq!(m.words.len(), 3);
        sfx::prefetch_words(&["dog", "hat"]);
        m.control(1.0, 0.01);
        decode_ahead(&mut m);
        assert_eq!(m.words.len(), 2);
        assert!(!m.words.ready("word_cat"));
        assert!(m.words.ready("word_hat"));
        assert_eq!(m.words.decoded, 4);
    }

    #[test]
    fn a_word_asked_for_before_it_is_decoded_is_spoken_once_it_is() {
        let mut m = Mixer::new(bank(), 44100);
        sfx::take_prefetch();
        m.apply(&[], Some(Say::Word("dog")), None, true, 5.0, 0.01);
        assert!(m.played.is_empty());
        assert!(m.word_waiting());
        assert_eq!(m.next_word().as_deref(), Some("word_dog"));
        decode_ahead(&mut m);
        assert_eq!(m.played, vec!["word_dog.0".to_string()]);
        assert!(!m.word_waiting());
    }

    fn render_secs(m: &mut Mixer, secs: f32) -> Vec<f32> {
        let mut all = vec![];
        let mut buf = vec![0.0; 128 * 2];
        for _ in 0..(secs * m.out_rate as f32 / 128.0) as usize {
            m.render(&mut buf, 2);
            all.extend_from_slice(&buf);
        }
        all
    }

    #[test]
    fn a_remote_mixer_drives_the_render_side_to_the_same_output() {
        let mut local = Mixer::new(bank(), 44100);
        let mut remote = Mixer::remote();
        remote.out_rate = 44100;
        remote.bank = bank();
        let mut render = Mixer::new(Bank::default(), 44100);
        let script: Vec<(Vec<Cue>, Option<Say>)> = vec![
            (vec![], None),
            (vec![cue(Sfx::Pop)], None),
            (vec![], None),
            (vec![cue(Sfx::Win)], Some(Say::Great)),
            (vec![], None),
        ];
        let (mut a, mut b) = (vec![], vec![]);
        for (i, (cues, say)) in script.iter().cycle().take(60).enumerate() {
            let now = i as f64 * 0.1;
            local.apply(cues, *say, Some(Mood::Menu), true, now, 0.1);
            remote.apply(cues, *say, Some(Mood::Menu), true, now, 0.1);
            for c in remote.take_cmds() {
                render.run(c);
            }
            a.extend(render_secs(&mut local, 0.1));
            b.extend(render_secs(&mut render, 0.1));
        }
        assert_eq!(local.played, remote.played);
        assert_eq!(a.len(), b.len());
        assert!(a.iter().zip(&b).all(|(x, y)| (x - y).abs() < 1e-6));
    }

    #[test]
    fn the_render_side_keeps_playing_through_a_main_thread_stall() {
        let mut remote = Mixer::remote();
        remote.out_rate = 44100;
        remote.bank = bank();
        let mut render = Mixer::new(Bank::default(), 44100);
        for i in 0..100 {
            remote.apply(&[], None, Some(Mood::Menu), true, i as f64 * 0.01, 0.01);
            for c in remote.take_cmds() {
                render.run(c);
            }
            render_secs(&mut render, 0.01);
        }
        remote.apply(&[cue(Sfx::Pop)], None, Some(Mood::Menu), true, 1.0, 0.01);
        for c in remote.take_cmds() {
            render.run(c);
        }
        for stall in [0.05, 0.15, 0.6, 3.0] {
            let out = render_secs(&mut render, stall);
            assert!(
                out.chunks(128 * 2)
                    .all(|q| q.iter().any(|s| s.abs() > 0.01)),
                "silent block during a {stall} s stall"
            );
        }
    }

    #[test]
    fn a_remote_mixer_never_renders_or_decodes_itself() {
        let mut remote = Mixer::remote();
        remote.bank = bank();
        remote.apply(&[cue(Sfx::Pop)], None, Some(Mood::Menu), true, 0.0, 0.01);
        assert_eq!(remote.active_effects(), 0);
        let cmds = remote.take_cmds();
        assert!(matches!(
            cmds[0],
            Cmd::Start {
                music: false,
                keep: true,
                ..
            }
        ));
        assert!(cmds
            .iter()
            .any(|c| matches!(c, Cmd::Start { music: true, .. })));
        assert!(remote.take_cmds().is_empty());
    }

    #[test]
    fn the_real_pack_loads_effects_first_and_the_rest_in_order() {
        let order = later_order(sfx::packed(), Mood::Arcade);
        assert_eq!(order[0].0, Later::Music(Mood::Arcade));
        assert!(order.iter().any(|o| o.0 == Later::Voice("say_go".into())));
        assert_eq!(
            order
                .iter()
                .filter(|o| matches!(o.0, Later::Music(_)))
                .count(),
            3
        );
        let b = effects_bank();
        assert!(Sfx::ALL.iter().all(|s| !b.takes[s].is_empty()));
    }
}
