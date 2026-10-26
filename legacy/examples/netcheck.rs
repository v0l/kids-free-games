use kids_games::{net, profile, save};

fn main() {
    let a = profile::create("Netcheck", 3).unwrap();
    profile::unlock("snake", 7);
    profile::award_stars("snake", 1, 3);
    net::begin_play("snake");
    profile::submit("snake", 123, profile::Better::Higher);
    let start = std::time::Instant::now();
    let mut ticks = 0;
    while start.elapsed().as_secs() < 25 {
        net::update(0.1, true);
        if ticks == 40 {
            net::end_play();
            net::request_board("snake");
        }
        ticks += 1;
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    println!("player {}", a.id);
    println!("online {}", net::is_online());
    println!("board {:?}", net::online_board("snake"));
    println!(
        "token {}",
        save::get_str(&format!("net.{}.token", a.id)).is_some()
    );
    println!("outbox {:?}", save::get_str("net.outbox"));
    println!("playtime {:?}", save::get_str("net.playtime"));
}
