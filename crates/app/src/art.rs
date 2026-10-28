use glam::{vec2, Vec2};
use kg_core::art::{decode_page, page_bytes, parse, Frame, Index};
use std::cell::RefCell;
use std::collections::HashMap;

pub static EMBEDDED: &[(&str, &[u8])] = &[
    ("ui", include_bytes!("../../../assets/art/ui.kga")),
    ("icons", include_bytes!("../../../assets/art/icons.kga")),
    ("menu", include_bytes!("../../../assets/art/menu.kga")),
    (
        "profiles",
        include_bytes!("../../../assets/art/profiles.kga"),
    ),
];

pub const LATER: &[&str] = &[
    "flappy", "crossy", "snake", "clash", "merge", "fruit", "stack", "breakout", "jumper",
    "blocks", "mathpop", "count", "paper", "memory", "hotel", "helix", "runner", "spell",
    "trains",
];

pub fn all_sheets() -> impl Iterator<Item = &'static str> {
    EMBEDDED
        .iter()
        .map(|(n, _)| *n)
        .chain(LATER.iter().copied())
}

type Store = std::sync::Mutex<HashMap<&'static str, &'static [u8]>>;

fn store() -> &'static Store {
    static S: std::sync::OnceLock<Store> = std::sync::OnceLock::new();
    S.get_or_init(Default::default)
}

#[cfg(test)]
thread_local! {
    static HELD: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) };
}

#[cfg(test)]
pub fn hold_for_test(sheet: &'static str, on: bool) {
    HELD.with(|h| {
        let mut h = h.borrow_mut();
        h.retain(|s| *s != sheet);
        if on {
            h.push(sheet);
        }
    });
}

#[cfg(test)]
pub fn arrive_for_test(sheet: &'static str) {
    hold_for_test(sheet, false);
    sheet_bytes(sheet);
}

pub fn sheet_bytes(sheet: &str) -> Option<&'static [u8]> {
    if let Some((_, b)) = EMBEDDED.iter().find(|(n, _)| *n == sheet) {
        return Some(b);
    }
    #[cfg(test)]
    if HELD.with(|h| h.borrow().contains(&sheet)) {
        return None;
    }
    let name = LATER.iter().find(|n| **n == sheet)?;
    if let Some(b) = store().lock().ok()?.get(name) {
        return Some(*b);
    }
    #[cfg(all(test, not(target_arch = "wasm32")))]
    {
        let b = kg_core::assets::leaked(&kg_core::assets::art_file(sheet))?;
        store().lock().ok()?.insert(name, b);
        return Some(b);
    }
    #[allow(unreachable_code)]
    None
}

pub fn ready(sheet: &str) -> bool {
    sheet_bytes(sheet).is_some()
}

pub fn all_ready() -> bool {
    all_sheets().all(ready)
}

struct Fetch {
    sheet: &'static str,
    pending: kg_core::assets::Pending,
}

thread_local! {
    static FETCHING: RefCell<Vec<Fetch>> = const { RefCell::new(Vec::new()) };
}

pub fn begin_loading() {
    FETCHING.with(|f| {
        let mut f = f.borrow_mut();
        if f.is_empty() {
            for sheet in LATER {
                f.push(Fetch {
                    sheet,
                    pending: kg_core::assets::start(&kg_core::assets::art_file(sheet)),
                });
            }
        }
    });
}

pub fn pump() {
    let arrived: Vec<(&'static str, Option<Vec<u8>>)> = FETCHING.with(|f| {
        let mut f = f.borrow_mut();
        let mut out = vec![];
        f.retain_mut(|x| match x.pending.poll() {
            Some(bytes) => {
                out.push((x.sheet, bytes));
                false
            }
            None => true,
        });
        out
    });
    for (sheet, bytes) in arrived {
        match bytes {
            Some(b) => install(sheet, Box::leak(b.into_boxed_slice())),
            None => bevy::log::warn!("art sheet {sheet} failed to load"),
        }
    }
}

pub fn install(sheet: &'static str, bytes: &'static [u8]) {
    if parse(bytes).is_none() {
        bevy::log::warn!("art sheet {sheet} is not a valid pack");
        return;
    }
    if let Ok(mut s) = store().lock() {
        s.insert(sheet, bytes);
    }
    predecode_sheet(sheet, bytes);
}

struct Sheet {
    bytes: &'static [u8],
    index: Index,
    sizes: Vec<Option<(f32, f32)>>,
}

thread_local! {
    static LOADED: RefCell<HashMap<&'static str, Sheet>> = RefCell::new(HashMap::new());
}

fn with<R>(sheet: &'static str, f: impl FnOnce(&mut Sheet) -> R) -> Option<R> {
    LOADED.with(|l| {
        let mut l = l.borrow_mut();
        if !l.contains_key(sheet) {
            let bytes = sheet_bytes(sheet)?;
            let index = parse(bytes)?;
            let n = index.pages.len();
            l.insert(
                sheet,
                Sheet {
                    bytes,
                    index,
                    sizes: vec![None; n],
                },
            );
        }
        l.get_mut(sheet).map(f)
    })
}

pub fn frame(sheet: &'static str, name: &str) -> Option<Frame> {
    with(sheet, |s| s.index.frames.get(name).copied()).flatten()
}

pub fn size(sheet: &'static str, name: &str) -> Vec2 {
    frame(sheet, name).map_or(vec2(1.0, 1.0), |f| f.size())
}

pub fn names(sheet: &'static str) -> Vec<String> {
    with(sheet, |s| s.index.frames.keys().cloned().collect()).unwrap_or_default()
}

fn header_size(b: &[u8]) -> Option<(f32, f32)> {
    let r = image::ImageReader::new(std::io::Cursor::new(b))
        .with_guessed_format()
        .ok()?;
    let (w, h) = r.into_dimensions().ok()?;
    Some((w as f32, h as f32))
}

pub fn page_size(sheet: &'static str, page: usize) -> (f32, f32) {
    with(sheet, |s| {
        if let Some(sz) = s.sizes.get(page).copied().flatten() {
            return sz;
        }
        let sz = page_bytes(s.bytes, &s.index, page)
            .and_then(header_size)
            .unwrap_or((1.0, 1.0));
        if let Some(slot) = s.sizes.get_mut(page) {
            *slot = Some(sz);
        }
        sz
    })
    .unwrap_or((1.0, 1.0))
}

pub fn mip_chain(w: u32, h: u32, px: Vec<u8>) -> Vec<Vec<u8>> {
    let mut out = vec![px];
    if !(w.is_power_of_two() && h.is_power_of_two()) {
        return out;
    }
    let (mut w, mut h) = (w, h);
    while w > 1 || h > 1 {
        let (nw, nh) = ((w / 2).max(1), (h / 2).max(1));
        let src = out.last().unwrap();
        let mut dst = vec![0u8; (nw * nh * 4) as usize];
        for y in 0..nh {
            for x in 0..nw {
                for c in 0..4 {
                    let mut sum = 0u32;
                    for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                        let sx = (x * 2 + dx).min(w - 1);
                        let sy = (y * 2 + dy).min(h - 1);
                        sum += src[((sy * w + sx) * 4 + c) as usize] as u32;
                    }
                    dst[((y * nw + x) * 4 + c) as usize] = (sum / 4) as u8;
                }
            }
        }
        out.push(dst);
        w = nw;
        h = nh;
    }
    out
}

pub type Decoded = (u32, u32, Vec<Vec<u8>>);

type Decodes = std::sync::Mutex<HashMap<(&'static str, usize), Decoded>>;

static READY: std::sync::OnceLock<Decodes> = std::sync::OnceLock::new();

fn decoded() -> &'static Decodes {
    READY.get_or_init(Default::default)
}

fn decode_now(bytes: &'static [u8], sheet: &'static str, page: usize) -> Option<Decoded> {
    let index = parse(bytes)?;
    let (w, h, px) = decode_page(page_bytes(bytes, &index, page)?)?;
    let _ = sheet;
    Some((w, h, mip_chain(w, h, px)))
}

pub fn predecode() {
    for (sheet, bytes) in EMBEDDED {
        predecode_sheet(sheet, bytes);
    }
}

fn predecode_sheet(sheet: &'static str, bytes: &'static [u8]) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let pages = parse(bytes).map_or(0, |i| i.pages.len());
        for page in 0..pages {
            std::thread::spawn(move || {
                if let Some(d) = decode_now(bytes, sheet, page) {
                    if let Ok(mut r) = decoded().lock() {
                        r.insert((sheet, page), d);
                    }
                }
            });
        }
    }
    #[cfg(target_arch = "wasm32")]
    let _ = (sheet, bytes);
}

pub fn decode(sheet: &'static str, page: usize) -> Option<Decoded> {
    if let Some(d) = decoded()
        .lock()
        .ok()
        .and_then(|mut r| r.remove(&(sheet, page)))
    {
        return Some(d);
    }
    decode_now(sheet_bytes(sheet)?, sheet, page)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_embedded_sheet_parses_and_page_sizes_match_the_decoded_pixels() {
        for name in all_sheets() {
            assert!(!names(name).is_empty(), "{name} is empty");
            let (w, h) = page_size(name, 0);
            let (dw, dh, mips) = decode(name, 0).unwrap();
            assert_eq!((w as u32, h as u32), (dw, dh), "{name}");
            assert_eq!(mips[0].len(), (dw * dh * 4) as usize);
        }
    }

    #[test]
    fn pages_decoded_ahead_match_pages_decoded_on_demand() {
        predecode();
        let mut waited = 0;
        while decoded().lock().unwrap().get(&("ui", 0)).is_none() && waited < 200 {
            std::thread::sleep(std::time::Duration::from_millis(50));
            waited += 1;
        }
        let ahead = decode("ui", 0).unwrap();
        let now = decode("ui", 0).unwrap();
        assert_eq!((ahead.0, ahead.1), (now.0, now.1));
        assert_eq!(ahead.2[0], now.2[0]);
    }

    #[test]
    fn power_of_two_pages_get_a_full_mip_chain_down_to_one_pixel() {
        let px = vec![200u8; 8 * 4 * 4];
        let chain = mip_chain(8, 4, px);
        assert_eq!(chain.len(), 4);
        assert_eq!(chain[3].len(), 4);
        assert_eq!(chain[3][0], 200);
        assert_eq!(mip_chain(6, 4, vec![0; 96]).len(), 1);
    }

    #[test]
    fn every_ui_sprite_the_code_names_exists() {
        let have = names("ui");
        for n in crate::ui::SPRITES {
            assert!(have.iter().any(|h| h == n), "ui sheet lacks {n}");
        }
        for g in kg_core::games::CATALOG {
            assert!(
                frame("icons", &format!("icon_{}", g.key)).is_some(),
                "no icon for {}",
                g.key
            );
        }
    }
}
