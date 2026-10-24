use kids_games::{run, window_conf};

#[macroquad::main(window_conf)]
async fn main() {
    run().await;
}
