use kg_core::assets::{self, Pending};
use std::cell::RefCell;

#[derive(Default)]
struct Loading {
    sound: Option<Pending>,
    words: Option<Pending>,
    started: bool,
}

impl Loading {
    fn idle(&self) -> bool {
        self.sound.is_none() && self.words.is_none()
    }
}

thread_local! {
    static LOADING: RefCell<Loading> = RefCell::new(Loading::default());
}

#[cfg(test)]
thread_local! {
    static WORDS_HELD: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(test)]
pub fn hold_words_for_test(on: bool) {
    WORDS_HELD.with(|h| h.set(on));
}

pub fn begin() {
    crate::art::begin_loading();
    LOADING.with(|l| {
        let mut l = l.borrow_mut();
        if !l.started {
            l.started = true;
            l.sound = Some(assets::start(assets::SOUND));
            l.words = Some(assets::start(assets::WORDS));
        }
    });
}

fn leak(bytes: Vec<u8>) -> &'static [u8] {
    Box::leak(bytes.into_boxed_slice())
}

pub fn pump() {
    crate::art::pump();
    LOADING.with(|l| {
        let mut l = l.borrow_mut();
        if let Some(p) = &mut l.sound {
            if let Some(bytes) = p.poll() {
                match bytes.map(leak).filter(|b| kg_core::sfx::install_sound(b)) {
                    Some(_) => {}
                    None => bevy::log::warn!("sound pack failed to load"),
                }
                l.sound = None;
            }
        }
        if let Some(p) = &mut l.words {
            if let Some(bytes) = p.poll() {
                match bytes.map(leak).filter(|b| kg_core::sfx::install_words(b)) {
                    Some(_) => {}
                    None => bevy::log::warn!("word pack failed to load"),
                }
                l.words = None;
            }
        }
    });
}

pub fn words_ready() -> bool {
    #[cfg(test)]
    {
        if WORDS_HELD.with(|h| h.get()) {
            return false;
        }
        if !kg_core::sfx::words_ready() {
            if let Some(b) = assets::leaked(assets::WORDS) {
                kg_core::sfx::install_words(b);
            }
        }
    }
    kg_core::sfx::words_ready()
}

pub fn ready(key: &str) -> bool {
    crate::art::ready(key) && (key != kg_core::games::spell::KEY || words_ready())
}

pub fn settled() -> bool {
    LOADING.with(|l| l.borrow().idle()) && crate::art::all_ready()
}
