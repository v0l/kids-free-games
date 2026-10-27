use bevy::prelude::*;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use kg_core::mixer::{effects_bank, Bank, Loader, Mixer};
use kg_core::sfx::Mood;
use std::sync::{Arc, Mutex};

pub struct Audio {
    mixer: Arc<Mutex<Mixer>>,
    stream: Option<cpal::Stream>,
    loader: Option<Loader>,
    resumed: bool,
    reported: bool,
}

fn open(mixer: Arc<Mutex<Mixer>>) -> Option<(cpal::Stream, u32)> {
    let host = cpal::default_host();
    let device = host.default_output_device()?;
    let config = device.default_output_config().ok()?;
    let rate = config.sample_rate();
    let channels = config.channels() as usize;
    let format = config.sample_format();
    let cfg: cpal::StreamConfig = config.into();
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
    let _ = stream.play();
    Some((stream, rate))
}

impl Audio {
    pub fn new() -> Self {
        let mixer = Arc::new(Mutex::new(Mixer::new(Bank::default(), 48000)));
        let opened = open(mixer.clone());
        let stream = opened.map(|(s, rate)| {
            if let Ok(mut m) = mixer.lock() {
                m.out_rate = rate.max(1);
            }
            s
        });
        if stream.is_none() {
            warn!("no audio output device");
        }
        Self {
            mixer,
            stream,
            loader: None,
            resumed: false,
            reported: false,
        }
    }

    pub fn resume(&mut self) {
        if !self.resumed {
            self.resumed = true;
            if let Some(s) = &self.stream {
                let _ = s.play();
            }
        }
    }
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
    if let Ok(mut m) = audio.mixer.lock() {
        if !audio.reported && now > 3.0 {
            audio.reported = true;
            info!(
                "audio: {} Hz, {} frames rendered, device {}",
                m.out_rate,
                m.frames_rendered,
                if audio.stream.is_some() {
                    "open"
                } else {
                    "missing"
                }
            );
        }
        m.control(now, dt);
        m.played.clear();
        if audio.stream.is_none() {
            let mut sink = vec![0.0; (m.out_rate as f32 * dt) as usize * 2];
            m.render(&mut sink, 2);
        }
    }
}
