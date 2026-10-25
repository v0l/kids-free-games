#[derive(Clone, Debug)]
pub struct HttpRequest {
    pub method: &'static str,
    pub path: String,
    pub token: Option<String>,
    pub body: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: i32,
    pub body: String,
}

impl HttpResponse {
    pub fn network_error() -> Self {
        Self {
            status: 0,
            body: String::new(),
        }
    }
}

pub trait Transport {
    fn start(&mut self, req: HttpRequest) -> u32;
    fn poll(&mut self, id: u32) -> Option<HttpResponse>;
}

pub struct Offline;

impl Transport for Offline {
    fn start(&mut self, _req: HttpRequest) -> u32 {
        0
    }

    fn poll(&mut self, _id: u32) -> Option<HttpResponse> {
        Some(HttpResponse::network_error())
    }
}

pub const API_URL: &str = match option_env!("KG_API_URL") {
    Some(url) => url,
    None => "https://kids-games-api.wrk-cf.workers.dev",
};

pub fn default_transport() -> Box<dyn Transport> {
    if cfg!(test) {
        Box::new(Offline)
    } else {
        Box::new(platform::Platform::default())
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod platform {
    use super::*;
    use std::collections::HashMap;
    use std::sync::mpsc::{channel, Receiver, Sender};
    use std::time::Duration;

    pub struct Platform {
        next: u32,
        done: HashMap<u32, HttpResponse>,
        tx: Sender<(u32, HttpResponse)>,
        rx: Receiver<(u32, HttpResponse)>,
    }

    impl Default for Platform {
        fn default() -> Self {
            let (tx, rx) = channel();
            Self {
                next: 0,
                done: HashMap::new(),
                tx,
                rx,
            }
        }
    }

    fn call(req: HttpRequest) -> HttpResponse {
        let agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(15))
            .build();
        let mut r = agent.request(req.method, &format!("{API_URL}{}", req.path));
        if let Some(t) = &req.token {
            r = r.set("Authorization", &format!("Bearer {t}"));
        }
        let result = match &req.body {
            Some(b) => r.set("Content-Type", "application/json").send_string(b),
            None => r.call(),
        };
        match result {
            Ok(resp) | Err(ureq::Error::Status(_, resp)) => {
                let status = resp.status() as i32;
                match resp.into_string() {
                    Ok(body) => HttpResponse { status, body },
                    Err(_) => HttpResponse::network_error(),
                }
            }
            Err(_) => HttpResponse::network_error(),
        }
    }

    impl Transport for Platform {
        fn start(&mut self, req: HttpRequest) -> u32 {
            self.next += 1;
            let id = self.next;
            let tx = self.tx.clone();
            let spawned = std::thread::Builder::new()
                .name("kg-net".into())
                .spawn(move || {
                    let _ = tx.send((id, call(req)));
                });
            if spawned.is_err() {
                self.done.insert(id, HttpResponse::network_error());
            }
            id
        }

        fn poll(&mut self, id: u32) -> Option<HttpResponse> {
            while let Ok((i, r)) = self.rx.try_recv() {
                self.done.insert(i, r);
            }
            self.done.remove(&id)
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod platform {
    use super::*;

    extern "C" {
        fn kg_http_start(
            method: *const u8,
            method_len: u32,
            url: *const u8,
            url_len: u32,
            token: *const u8,
            token_len: u32,
            body: *const u8,
            body_len: u32,
        ) -> u32;
        fn kg_http_status(id: u32) -> i32;
        fn kg_http_len(id: u32) -> u32;
        fn kg_http_read(id: u32, ptr: *mut u8, len: u32);
        fn kg_http_free(id: u32);
    }

    #[derive(Default)]
    pub struct Platform;

    impl Transport for Platform {
        fn start(&mut self, req: HttpRequest) -> u32 {
            let url = format!("{API_URL}{}", req.path);
            let token = req.token.unwrap_or_default();
            let body = req.body.unwrap_or_default();
            unsafe {
                kg_http_start(
                    req.method.as_ptr(),
                    req.method.len() as u32,
                    url.as_ptr(),
                    url.len() as u32,
                    token.as_ptr(),
                    token.len() as u32,
                    body.as_ptr(),
                    body.len() as u32,
                )
            }
        }

        fn poll(&mut self, id: u32) -> Option<HttpResponse> {
            unsafe {
                let status = kg_http_status(id);
                if status == 0 {
                    return None;
                }
                let n = kg_http_len(id);
                let mut buf = vec![0u8; n as usize];
                if n > 0 {
                    kg_http_read(id, buf.as_mut_ptr(), n);
                }
                kg_http_free(id);
                if status < 0 {
                    return Some(HttpResponse::network_error());
                }
                Some(HttpResponse {
                    status,
                    body: String::from_utf8(buf).unwrap_or_default(),
                })
            }
        }
    }
}
