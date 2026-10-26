#[cfg(not(target_arch = "wasm32"))]
mod native {
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::{Mutex, OnceLock};

    pub fn dirs() -> Vec<PathBuf> {
        let mut out = vec![];
        if let Some(dir) = std::env::current_exe()
            .ok()
            .and_then(|e| e.parent().map(|p| p.to_path_buf()))
        {
            out.push(dir.join("assets"));
            out.push(dir);
        }
        out.push(PathBuf::from("assets"));
        out.push(PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets")));
        out
    }

    static READER: OnceLock<fn(&str) -> Option<Vec<u8>>> = OnceLock::new();

    pub fn set_reader(f: fn(&str) -> Option<Vec<u8>>) {
        let _ = READER.set(f);
    }

    pub fn read(rel: &str) -> Option<Vec<u8>> {
        if let Some(f) = READER.get() {
            return f(rel);
        }
        dirs().into_iter().find_map(|d| std::fs::read(d.join(rel)).ok())
    }

    pub fn leaked(rel: &str) -> Option<&'static [u8]> {
        static CACHE: OnceLock<Mutex<HashMap<String, &'static [u8]>>> = OnceLock::new();
        let mut cache = CACHE.get_or_init(Default::default).lock().ok()?;
        if let Some(b) = cache.get(rel) {
            return Some(*b);
        }
        let bytes: &'static [u8] = Box::leak(read(rel)?.into_boxed_slice());
        cache.insert(rel.to_string(), bytes);
        Some(bytes)
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub use native::{dirs, leaked, read, set_reader};

pub struct Pending(imp::Slot);

pub fn start(rel: &str) -> Pending {
    Pending(imp::start(rel))
}

impl Pending {
    pub fn poll(&mut self) -> Option<Option<Vec<u8>>> {
        imp::poll(&mut self.0)
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use std::sync::mpsc::{channel, Receiver, TryRecvError};

    pub type Slot = Receiver<Option<Vec<u8>>>;

    pub fn start(rel: &str) -> Slot {
        let (tx, rx) = channel();
        let rel = rel.to_string();
        let sent = tx.clone();
        let spawned = std::thread::Builder::new()
            .name("kg-assets".into())
            .spawn(move || {
                let _ = tx.send(super::native::read(&rel));
            });
        if spawned.is_err() {
            let _ = sent.send(None);
        }
        rx
    }

    pub fn poll(slot: &mut Slot) -> Option<Option<Vec<u8>>> {
        match slot.try_recv() {
            Ok(v) => Some(v),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(None),
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod imp {
    use std::cell::RefCell;
    use std::rc::Rc;
    use wasm_bindgen::JsCast;

    pub type Slot = Rc<RefCell<Option<Option<Vec<u8>>>>>;

    fn url(rel: &str) -> String {
        match option_env!("KG_ASSET_VERSION") {
            Some(v) if !v.is_empty() => format!("assets/{rel}?v={v}"),
            _ => format!("assets/{rel}"),
        }
    }

    async fn fetch(rel: String) -> Option<Vec<u8>> {
        let window = web_sys::window()?;
        let resp = wasm_bindgen_futures::JsFuture::from(window.fetch_with_str(&url(&rel)))
            .await
            .ok()?
            .dyn_into::<web_sys::Response>()
            .ok()?;
        if !resp.ok() {
            return None;
        }
        let buf = wasm_bindgen_futures::JsFuture::from(resp.array_buffer().ok()?)
            .await
            .ok()?;
        Some(js_sys::Uint8Array::new(&buf).to_vec())
    }

    pub fn start(rel: &str) -> Slot {
        let slot: Slot = Rc::new(RefCell::new(None));
        let out = slot.clone();
        let rel = rel.to_string();
        wasm_bindgen_futures::spawn_local(async move {
            let r = fetch(rel).await;
            *out.borrow_mut() = Some(r);
        });
        slot
    }

    pub fn poll(slot: &mut Slot) -> Option<Option<Vec<u8>>> {
        slot.borrow_mut().take()
    }
}

pub const SOUND: &str = "sound.kgs";
pub const WORDS: &str = "sound_words.kgs";

pub fn art_file(sheet: &str) -> String {
    format!("art/{sheet}.kga")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_repo_assets_are_found_from_a_test() {
        assert!(leaked(SOUND).is_some());
        assert!(leaked(WORDS).is_some());
        assert!(leaked(&art_file("ui")).is_some());
        assert!(leaked("art/nope.kga").is_none());
    }

    #[test]
    fn a_started_read_arrives_and_a_missing_one_reports_failure() {
        let mut ok = start(SOUND);
        let mut missing = start("art/nope.kga");
        let (mut a, mut b) = (None, None);
        for _ in 0..400 {
            a = a.or_else(|| ok.poll());
            b = b.or_else(|| missing.poll());
            if a.is_some() && b.is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(a.unwrap().unwrap().starts_with(b"KGS1"));
        assert_eq!(b, Some(None));
    }
}
