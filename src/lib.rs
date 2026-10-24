pub mod art;
pub mod engine;
pub mod feel;
pub mod fx;
pub mod games;
pub mod learn;
pub mod menu;
pub mod outline;
pub mod safe;
pub mod sfx;
pub mod ui;

use engine::*;
use macroquad::miniquad;
use macroquad::prelude::*;

pub fn window_conf() -> Conf {
    Conf {
        window_title: "Kids Free Games".to_owned(),
        window_width: 1024,
        window_height: 768,
        high_dpi: true,
        sample_count: if cfg!(any(target_os = "android", target_os = "ios")) {
            1
        } else {
            4
        },
        window_resizable: true,
        platform: miniquad::conf::Platform {
            linux_backend: miniquad::conf::LinuxBackend::WaylandWithX11Fallback,
            apple_gfx_api: miniquad::conf::AppleGfxApi::Metal,
            ..Default::default()
        },
        ..Default::default()
    }
}

pub fn begin_frame() -> View {
    begin_frame_safe(safe::Insets::default())
}

pub fn begin_frame_safe(px: safe::Insets) -> View {
    let dpi = screen_dpi_scale();
    let px = safe::Insets {
        left: px.left / dpi,
        top: px.top / dpi,
        right: px.right / dpi,
        bottom: px.bottom / dpi,
    };
    let view = View::fit_safe(screen_width(), screen_height(), px);
    set_text_scale(view.scale * dpi);
    set_view_camera(&view, Vec2::ZERO);
    view
}

pub async fn run() {
    sfx::load().await;
    macroquad::experimental::coroutines::start_coroutine(sfx::load_music());
    let mut tracker = InputTracker::default();
    let mut app = menu::App::new();
    let mut safe_area = safe::SafeArea::default();
    loop {
        let (w, h) = macroquad::miniquad::window::screen_size();
        let view = begin_frame_safe(safe_area.get((w, h)));
        let input = tracker.poll(&view);
        let dt = get_frame_time().min(1.0 / 20.0);
        app.update(&input, &view, dt);
        app.draw(&view);
        sfx::flush(get_time(), get_frame_time().min(0.1));
        next_frame().await;
    }
}
