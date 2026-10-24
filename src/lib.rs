pub mod engine;
pub mod games;
pub mod menu;

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
    let view = View::fit(screen_width(), screen_height());
    set_text_scale(view.scale * screen_dpi_scale());
    set_camera(&Camera2D::from_display_rect(Rect::new(
        0.0, view.h, view.w, -view.h,
    )));
    view
}

pub async fn run() {
    let mut tracker = InputTracker::default();
    let mut app = menu::App::new();
    loop {
        let view = begin_frame();
        let input = tracker.poll(&view);
        let dt = get_frame_time().min(1.0 / 20.0);
        app.update(&input, &view, dt);
        app.draw(&view);
        next_frame().await;
    }
}
