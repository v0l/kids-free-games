use kg_core::games::hotel::layout::{Build, Layout};
use kg_core::games::hotel::shell::plan;

fn main() {
    let number: u32 = std::env::args().nth(1).and_then(|a| a.parse().ok()).unwrap_or(3);
    let all = std::env::args().nth(2).as_deref() != Some("locked");
    let lay = Layout::new(number);
    let p = plan(&lay, &|b: Build| all || b == Build::Room(0));
    print!("{}", p.text);
}
