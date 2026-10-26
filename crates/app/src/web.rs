pub fn query(key: &str) -> Option<String> {
    #[cfg(target_arch = "wasm32")]
    {
        let search = web_sys::window()?.location().search().ok()?;
        let q = web_sys::UrlSearchParams::new_with_str(&search).ok()?;
        q.get(key)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let flag = format!("--{key}");
        let mut args = std::env::args().skip(1);
        while let Some(a) = args.next() {
            if a == flag {
                return Some(args.next().unwrap_or_default());
            }
            if let Some(v) = a.strip_prefix(&format!("{flag}=")) {
                return Some(v.to_string());
            }
        }
        None
    }
}

pub fn hash() -> Option<String> {
    #[cfg(target_arch = "wasm32")]
    {
        let h = web_sys::window()?.location().hash().ok()?;
        let h = h.trim_start_matches('#');
        (!h.is_empty()).then(|| h.to_string())
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        None
    }
}
