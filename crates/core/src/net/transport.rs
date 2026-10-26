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

pub fn full_url(path: &str) -> String {
    if path.starts_with("https://") {
        path.to_string()
    } else {
        format!("{API_URL}{path}")
    }
}

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
        let mut r = agent.request(req.method, &full_url(&req.path));
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
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::rc::Rc;
    use wasm_bindgen::JsCast;

    #[derive(Default)]
    pub struct Platform {
        next: u32,
        done: Rc<RefCell<HashMap<u32, HttpResponse>>>,
    }

    async fn call(req: HttpRequest) -> HttpResponse {
        let fail = HttpResponse::network_error();
        let Some(window) = web_sys::window() else {
            return fail;
        };
        let init = web_sys::RequestInit::new();
        init.set_method(req.method);
        let Ok(headers) = web_sys::Headers::new() else {
            return fail;
        };
        if let Some(t) = &req.token {
            let _ = headers.set("Authorization", &format!("Bearer {t}"));
        }
        if let Some(b) = &req.body {
            let _ = headers.set("Content-Type", "application/json");
            init.set_body(&wasm_bindgen::JsValue::from_str(b));
        }
        init.set_headers(&headers);
        let Ok(request) = web_sys::Request::new_with_str_and_init(&full_url(&req.path), &init)
        else {
            return fail;
        };
        let Ok(resp) =
            wasm_bindgen_futures::JsFuture::from(window.fetch_with_request(&request)).await
        else {
            return fail;
        };
        let Ok(resp) = resp.dyn_into::<web_sys::Response>() else {
            return fail;
        };
        let status = resp.status() as i32;
        let Ok(text) = resp.text() else { return fail };
        match wasm_bindgen_futures::JsFuture::from(text).await {
            Ok(t) => HttpResponse {
                status,
                body: t.as_string().unwrap_or_default(),
            },
            Err(_) => fail,
        }
    }

    impl Transport for Platform {
        fn start(&mut self, req: HttpRequest) -> u32 {
            self.next += 1;
            let id = self.next;
            let done = self.done.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let r = call(req).await;
                done.borrow_mut().insert(id, r);
            });
            id
        }

        fn poll(&mut self, id: u32) -> Option<HttpResponse> {
            self.done.borrow_mut().remove(&id)
        }
    }
}
