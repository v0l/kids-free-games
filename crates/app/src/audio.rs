use bevy::prelude::*;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use kg_core::mixer::{effects_bank, Bank, Loader, Mixer, Words};
use kg_core::sfx::Mood;
use std::sync::{Arc, Mutex};

#[cfg(target_arch = "wasm32")]
mod worklet {
    use kg_core::mixer::Cmd;
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen(module = "/src/web_audio.js")]
    extern "C" {
        fn kg_audio_supported() -> bool;
        fn kg_audio_open() -> f64;
        fn kg_audio_play(id: u32, rate: u32, data: &[f32], gain: f32, music: bool);
        fn kg_audio_play_once(rate: u32, data: &[f32], gain: f32);
        fn kg_audio_stop_music();
        fn kg_audio_music_gain(gain: f32);
    }

    pub fn open() -> Option<u32> {
        kg_audio_supported().then(|| kg_audio_open() as u32)
    }

    pub fn send(cmds: Vec<Cmd>) {
        for c in cmds {
            match c {
                Cmd::Start {
                    clip,
                    gain,
                    music,
                    keep: true,
                } => kg_audio_play(clip.id, clip.rate, &clip.data, gain, music),
                Cmd::Start { clip, gain, .. } => kg_audio_play_once(clip.rate, &clip.data, gain),
                Cmd::StopMusic => kg_audio_stop_music(),
                Cmd::MusicGain(g) => kg_audio_music_gain(g),
            }
        }
    }
}

enum Output {
    Device(cpal::Stream),
    #[cfg(target_arch = "wasm32")]
    Worklet,
    Missing,
}

pub struct Audio {
    mixer: Arc<Mutex<Mixer>>,
    output: Output,
    loader: Option<Loader>,
    resumed: bool,
    reported: bool,
}

pub const WEB_LEAD_SECS: f32 = 0.35;

pub fn web_buffer_frames(rate: u32) -> u32 {
    ((rate as f32 * WEB_LEAD_SECS) as u32).next_power_of_two()
}

fn buffer_size(rate: u32) -> cpal::BufferSize {
    if cfg!(target_arch = "wasm32") {
        cpal::BufferSize::Fixed(web_buffer_frames(rate))
    } else {
        cpal::BufferSize::Default
    }
}

fn open(mixer: Arc<Mutex<Mixer>>) -> Option<(cpal::Stream, u32)> {
    let host = cpal::default_host();
    let device = host.default_output_device()?;
    let config = device.default_output_config().ok()?;
    let rate = config.sample_rate();
    let channels = config.channels() as usize;
    let format = config.sample_format();
    let mut cfg: cpal::StreamConfig = config.into();
    cfg.buffer_size = buffer_size(cfg.sample_rate);
    let err = |e| warn!("audio stream error: {e}");
    let stream = match format {
        cpal::SampleFormat::F32 => device
            .build_output_stream(
                &cfg,
                move |out: &mut [f32], _| {
                    if let Ok(mut m) = mixer.lock() {
                        m.render(out, channels);
                    } else {
                        out.fill(0.0);
                    }
                },
                err,
                None,
            )
            .ok()?,
        cpal::SampleFormat::I16 => {
            let mut buf = vec![];
            device
                .build_output_stream(
                    &cfg,
                    move |out: &mut [i16], _| {
                        buf.resize(out.len(), 0.0);
                        if let Ok(mut m) = mixer.lock() {
                            m.render(&mut buf, channels);
                        }
                        for (o, s) in out.iter_mut().zip(&buf) {
                            *o = (s * 32767.0) as i16;
                        }
                    },
                    err,
                    None,
                )
                .ok()?
        }
        _ => return None,
    };
    Some((stream, rate))
}

pub fn start_now(web: bool, gesture: bool, started: bool) -> bool {
    !started && (gesture || !web)
}

impl Audio {
    pub fn new() -> Self {
        #[cfg(target_arch = "wasm32")]
        if let Some(rate) = worklet::open() {
            let mut m = Mixer::remote();
            m.out_rate = rate.max(1);
            return Self {
                mixer: Arc::new(Mutex::new(m)),
                output: Output::Worklet,
                loader: None,
                resumed: false,
                reported: false,
            };
        }
        let mixer = Arc::new(Mutex::new(Mixer::new(Bank::default(), 48000)));
        let output = match open(mixer.clone()) {
            Some((s, rate)) => {
                if let Ok(mut m) = mixer.lock() {
                    m.out_rate = rate.max(1);
                }
                Output::Device(s)
            }
            None => {
                warn!("no audio output device");
                Output::Missing
            }
        };
        let mut a = Self {
            mixer,
            output,
            loader: None,
            resumed: false,
            reported: false,
        };
        a.start(false);
        a
    }

    fn start(&mut self, gesture: bool) {
        if start_now(cfg!(target_arch = "wasm32"), gesture, self.resumed) {
            self.resumed = true;
            if let Output::Device(s) = &self.output {
                let _ = s.play();
            }
        }
    }

    fn backend(&self) -> &'static str {
        match self.output {
            Output::Device(_) => "device",
            #[cfg(target_arch = "wasm32")]
            Output::Worklet => "worklet",
            Output::Missing => "missing",
        }
    }

    pub fn resume(&mut self) {
        self.start(true);
    }
}

pub fn decode_next_word(mixer: &Mutex<Mixer>) -> bool {
    let Some(key) = mixer.lock().ok().and_then(|m| m.next_word()) else {
        return false;
    };
    let clip = Words::decode(&key);
    if let Ok(mut m) = mixer.lock() {
        m.word_decoded(key, clip);
    }
    true
}

impl Default for Audio {
    fn default() -> Self {
        Self::new()
    }
}

pub fn tick(audio: &mut Audio, now: f64, dt: f32, touched: bool) {
    if touched {
        audio.resume();
    }
    if audio.loader.is_none() && kg_core::sfx::sound_ready() {
        if let Ok(mut m) = audio.mixer.lock() {
            m.bank = effects_bank();
        }
        audio.loader = Some(Loader::new(
            kg_core::sfx::wanted_music().unwrap_or(Mood::Menu),
        ));
    }
    if let Some(loader) = audio.loader.as_mut().filter(|l| !l.done()) {
        if let Some((what, clip)) = loader.step() {
            if let Ok(mut m) = audio.mixer.lock() {
                Loader::store(&mut m.bank, what, clip);
            }
        }
    }
    let backend = audio.backend();
    if let Ok(mut m) = audio.mixer.lock() {
        if !audio.reported && now > 3.0 {
            audio.reported = true;
            info!(
                "audio: {} Hz, {} frames rendered, output {backend}",
                m.out_rate, m.frames_rendered,
            );
        }
        m.control(now, dt);
        m.played.clear();
        if let Output::Missing = audio.output {
            let mut sink = vec![0.0; (m.out_rate as f32 * dt) as usize * 2];
            m.render(&mut sink, 2);
        }
    }
    decode_next_word(&audio.mixer);
    #[cfg(target_arch = "wasm32")]
    if let (Output::Worklet, Ok(mut m)) = (&audio.output, audio.mixer.lock()) {
        worklet::send(m.take_cmds());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stream_is_started_exactly_once() {
        for web in [false, true] {
            let mut started = false;
            let mut plays = 0;
            for gesture in [false, false, true, true, false, true] {
                if start_now(web, gesture, started) {
                    started = true;
                    plays += 1;
                }
            }
            assert_eq!(plays, 1, "web {web}");
        }
        assert!(!start_now(true, false, false));
        assert!(start_now(false, false, false));
    }

    #[test]
    fn web_audio_rides_out_a_long_frame() {
        for rate in [44_100, 48_000] {
            let frames = web_buffer_frames(rate);
            let secs = frames as f32 / rate as f32;
            assert!(secs >= 0.3, "{rate} Hz buffers last only {secs} s");
            assert!(secs <= 0.8, "{rate} Hz buffers add {secs} s of lag");
        }
    }
}
