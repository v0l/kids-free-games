use std::cell::RefCell;
use std::collections::BTreeMap;

pub type Map = BTreeMap<String, String>;

pub fn escape(v: &str) -> String {
    v.replace('\\', "\\\\").replace('\n', "\\n")
}

pub fn unescape(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    let mut it = v.chars();
    while let Some(c) = it.next() {
        if c == '\\' {
            match it.next() {
                Some('n') => out.push('\n'),
                Some(o) => out.push(o),
                None => {}
            }
        } else {
            out.push(c);
        }
    }
    out
}

pub fn encode(map: &Map) -> String {
    map.iter()
        .map(|(k, v)| format!("{k}={}\n", escape(v)))
        .collect()
}

pub fn decode(text: &str) -> Map {
    text.lines()
        .filter_map(|l| {
            let (k, v) = l.split_once('=')?;
            let k = k.trim();
            if k.is_empty() || k.contains(char::is_whitespace) {
                return None;
            }
            Some((k.to_string(), unescape(v)))
        })
        .collect()
}

struct Store {
    map: Map,
    loaded: bool,
}

thread_local! {
    static STORE: RefCell<Store> = const {
        RefCell::new(Store {
            map: BTreeMap::new(),
            loaded: false,
        })
    };
}

thread_local! {
    static MEMORY: std::cell::Cell<bool> = const { std::cell::Cell::new(cfg!(test)) };
}

pub fn in_memory() -> bool {
    MEMORY.with(|m| m.get())
}

pub fn use_memory_only() {
    MEMORY.with(|m| m.set(true));
}

fn with<R>(f: impl FnOnce(&mut Map) -> R) -> R {
    STORE.with(|s| {
        let mut s = s.borrow_mut();
        if !s.loaded {
            s.loaded = true;
            if !in_memory() {
                s.map = decode(&backend::read());
            }
        }
        f(&mut s.map)
    })
}

fn persist(text: Option<String>) {
    if let Some(t) = text {
        if !in_memory() {
            backend::write(&t);
        }
    }
}

pub fn get_str(key: &str) -> Option<String> {
    with(|m| m.get(key).cloned())
}

pub fn set_str(key: &str, value: &str) {
    persist(with(|m| {
        if m.get(key).map(|s| s.as_str()) == Some(value) {
            return None;
        }
        m.insert(key.to_string(), value.to_string());
        Some(encode(m))
    }));
}

pub fn remove_prefix(prefix: &str) {
    persist(with(|m| {
        let before = m.len();
        m.retain(|k, _| !k.starts_with(prefix));
        (m.len() != before).then(|| encode(m))
    }));
}

pub fn keys_with_prefix(prefix: &str) -> Vec<String> {
    with(|m| {
        m.range(prefix.to_string()..)
            .take_while(|(k, _)| k.starts_with(prefix))
            .map(|(k, _)| k.clone())
            .collect()
    })
}

pub fn get(key: &str) -> i64 {
    get_str(key).and_then(|s| s.parse().ok()).unwrap_or(0)
}

pub fn set(key: &str, value: i64) {
    set_str(key, &value.to_string());
}

pub fn raise(key: &str, value: i64) -> bool {
    if value > get(key) {
        set(key, value);
        true
    } else {
        false
    }
}

pub fn player_key(key: &str) -> String {
    match crate::profile::current_id() {
        Some(id) => format!("p.{id}.{key}"),
        None => format!("p.guest.{key}"),
    }
}

pub fn player_get(key: &str) -> i64 {
    get(&player_key(key))
}

pub fn player_set(key: &str, value: i64) {
    set(&player_key(key), value);
}

pub fn player_raise(key: &str, value: i64) -> bool {
    raise(&player_key(key), value)
}

pub fn player_get_str(key: &str) -> Option<String> {
    get_str(&player_key(key))
}

pub fn player_set_str(key: &str, value: &str) {
    set_str(&player_key(key), value);
}

pub fn reset_for_test() {
    use_memory_only();
    with(|m| m.clear());
}

pub fn swap_for_test(other: Map) -> Map {
    with(|m| std::mem::replace(m, other))
}

#[cfg(target_arch = "wasm32")]
mod backend {
    const KEY: &str = "kids-free-games-save";

    fn storage() -> Option<web_sys::Storage> {
        web_sys::window()?.local_storage().ok()?
    }

    pub fn read() -> String {
        storage()
            .and_then(|s| s.get_item(KEY).ok().flatten())
            .unwrap_or_default()
    }

    pub fn write(text: &str) {
        if let Some(s) = storage() {
            let _ = s.set_item(KEY, text);
        }
    }
}

thread_local! {
    static DIR: RefCell<Option<std::path::PathBuf>> = const { RefCell::new(None) };
}

pub fn set_dir(dir: std::path::PathBuf) {
    DIR.with(|d| *d.borrow_mut() = Some(dir));
    STORE.with(|s| {
        let mut s = s.borrow_mut();
        s.loaded = false;
        s.map.clear();
    });
}

pub fn dir() -> Option<std::path::PathBuf> {
    DIR.with(|d| d.borrow().clone()).or_else(default_dir)
}

#[cfg(target_arch = "wasm32")]
fn default_dir() -> Option<std::path::PathBuf> {
    None
}

#[cfg(not(target_arch = "wasm32"))]
fn default_dir() -> Option<std::path::PathBuf> {
    backend::default_dir()
}

#[cfg(not(target_arch = "wasm32"))]
mod backend {
    use std::path::PathBuf;

    #[cfg(target_os = "ios")]
    pub fn default_dir() -> Option<PathBuf> {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support"))
    }

    #[cfg(target_os = "android")]
    pub fn default_dir() -> Option<PathBuf> {
        None
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    pub fn default_dir() -> Option<PathBuf> {
        let base = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("APPDATA").map(PathBuf::from))
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))?;
        Some(base.join("kids-free-games"))
    }

    fn path() -> Option<PathBuf> {
        super::dir().map(|d| d.join("save.txt"))
    }

    pub fn read() -> String {
        path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .unwrap_or_default()
    }

    pub fn write(text: &str) {
        let Some(p) = path() else { return };
        if let Some(d) = p.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let tmp = p.with_extension("tmp");
        if std::fs::write(&tmp, text).is_ok() {
            let _ = std::fs::rename(tmp, p);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_strings_numbers_and_newlines() {
        let mut m = Map::new();
        m.insert("p.x.name".into(), "Mia\\Lou\nX".into());
        m.insert("p.x.spell.level".into(), "4".into());
        assert_eq!(decode(&encode(&m)), m);
        let d = decode("a=1\nbad line\n=3\nb = 7\nd e=1\n");
        assert_eq!(d.len(), 2);
        assert_eq!(d["b"].trim(), "7");
    }

    #[test]
    fn raise_only_goes_up() {
        reset_for_test();
        assert!(raise("t.best", 5));
        assert!(!raise("t.best", 3));
        assert_eq!(get("t.best"), 5);
        set("t.best", 1);
        assert_eq!(get("t.best"), 1);
        assert_eq!(get("missing"), 0);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn the_save_file_is_written_atomically_and_read_back() {
        let dir = std::env::temp_dir().join(format!("kg-save-{}", std::process::id()));
        set_dir(dir.clone());
        let mut m = Map::new();
        m.insert("p.x.name".into(), "Mia\nLou".into());
        backend::write(&encode(&m));
        assert!(dir.join("save.txt").exists());
        assert!(!dir.join("save.tmp").exists());
        assert_eq!(decode(&backend::read()), m);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn pointing_at_the_platform_dir_after_a_first_read_loads_that_save() {
        let dir = std::env::temp_dir().join(format!("kg-late-dir-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("save.txt"), "profiles=x\nmarker=42\n").unwrap();
        let got = std::thread::spawn(move || {
            MEMORY.with(|m| m.set(false));
            DIR.with(|d| *d.borrow_mut() = Some(std::env::temp_dir().join("kg-no-such-dir")));
            assert_eq!(get("marker"), 0);
            set_dir(dir.clone());
            let v = get("marker");
            let _ = std::fs::remove_dir_all(dir);
            v
        })
        .join()
        .unwrap();
        assert_eq!(got, 42);
    }

    #[test]
    fn prefixes_list_and_remove_only_their_keys() {
        reset_for_test();
        set("p.a.x", 1);
        set("p.a.y", 2);
        set("p.b.x", 3);
        assert_eq!(keys_with_prefix("p.a."), vec!["p.a.x", "p.a.y"]);
        remove_prefix("p.a.");
        assert_eq!(get("p.a.x"), 0);
        assert_eq!(get("p.b.x"), 3);
    }
}
