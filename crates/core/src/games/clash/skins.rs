use crate::geom::Color;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pattern {
    Solid,
    Stripes,
    Dots,
    Rainbow,
    Fade,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Need {
    Free,
    Level(u32),
    Eat(u32),
    Floor(u32),
    Games(u32),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub best_level: u32,
    pub eaten: u32,
    pub floor: u32,
    pub games: u32,
    pub wins: u32,
    pub rating: i64,
}

impl Need {
    pub fn met(self, s: &Stats) -> bool {
        match self {
            Need::Free => true,
            Need::Level(n) => s.best_level >= n,
            Need::Eat(n) => s.eaten >= n,
            Need::Floor(n) => s.floor >= n,
            Need::Games(n) => s.games >= n,
        }
    }

    pub fn amount(self) -> u32 {
        match self {
            Need::Free => 0,
            Need::Level(n) | Need::Eat(n) | Need::Floor(n) | Need::Games(n) => n,
        }
    }

    pub fn progress(self, s: &Stats) -> f32 {
        let have = match self {
            Need::Free => return 1.0,
            Need::Level(_) => s.best_level,
            Need::Eat(_) => s.eaten,
            Need::Floor(_) => s.floor,
            Need::Games(_) => s.games,
        };
        (have as f32 / self.amount() as f32).min(1.0)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Skin {
    pub name: &'static str,
    pub a: Color,
    pub b: Color,
    pub pattern: Pattern,
    pub need: Need,
}

const fn skin(
    name: &'static str,
    a: (u8, u8, u8),
    b: (u8, u8, u8),
    pattern: Pattern,
    need: Need,
) -> Skin {
    Skin {
        name,
        a: Color::from_rgba(a.0, a.1, a.2, 255),
        b: Color::from_rgba(b.0, b.1, b.2, 255),
        pattern,
        need,
    }
}

pub const SKINS: [Skin; 20] = [
    skin(
        "Lime",
        (120, 215, 60),
        (160, 235, 90),
        Pattern::Fade,
        Need::Free,
    ),
    skin(
        "Sunny",
        (255, 205, 40),
        (255, 235, 110),
        Pattern::Fade,
        Need::Free,
    ),
    skin(
        "Berry",
        (235, 70, 130),
        (255, 130, 175),
        Pattern::Fade,
        Need::Free,
    ),
    skin(
        "Sky",
        (70, 170, 245),
        (130, 210, 255),
        Pattern::Fade,
        Need::Free,
    ),
    skin(
        "Candy",
        (255, 105, 160),
        (255, 245, 250),
        Pattern::Stripes,
        Need::Level(5),
    ),
    skin(
        "Bee",
        (255, 205, 30),
        (60, 45, 50),
        Pattern::Stripes,
        Need::Level(8),
    ),
    skin(
        "Mint",
        (110, 225, 185),
        (60, 175, 150),
        Pattern::Dots,
        Need::Eat(3),
    ),
    skin(
        "Ladybug",
        (225, 50, 50),
        (50, 40, 50),
        Pattern::Dots,
        Need::Eat(10),
    ),
    skin(
        "Zebra",
        (250, 250, 250),
        (50, 45, 60),
        Pattern::Stripes,
        Need::Floor(3),
    ),
    skin(
        "Rainbow",
        (255, 80, 80),
        (150, 90, 255),
        Pattern::Rainbow,
        Need::Floor(8),
    ),
    skin(
        "Ocean",
        (40, 120, 220),
        (60, 220, 220),
        Pattern::Fade,
        Need::Level(12),
    ),
    skin(
        "Lava",
        (255, 90, 30),
        (255, 210, 60),
        Pattern::Fade,
        Need::Floor(12),
    ),
    skin(
        "Grape",
        (150, 80, 220),
        (205, 150, 255),
        Pattern::Dots,
        Need::Games(5),
    ),
    skin(
        "Panda",
        (250, 250, 250),
        (45, 45, 55),
        Pattern::Dots,
        Need::Games(12),
    ),
    skin(
        "Gold",
        (255, 190, 20),
        (255, 245, 150),
        Pattern::Fade,
        Need::Level(20),
    ),
    skin(
        "Frost",
        (180, 230, 255),
        (255, 255, 255),
        Pattern::Stripes,
        Need::Eat(25),
    ),
    skin(
        "Tiger",
        (255, 150, 30),
        (80, 45, 30),
        Pattern::Stripes,
        Need::Floor(15),
    ),
    skin(
        "Galaxy",
        (60, 50, 150),
        (200, 120, 255),
        Pattern::Dots,
        Need::Floor(20),
    ),
    skin(
        "Bubblegum",
        (255, 150, 215),
        (140, 220, 255),
        Pattern::Stripes,
        Need::Eat(50),
    ),
    skin(
        "King",
        (255, 215, 60),
        (200, 40, 60),
        Pattern::Stripes,
        Need::Floor(30),
    ),
];

pub fn unlocked(i: usize, s: &Stats) -> bool {
    SKINS.get(i).is_some_and(|k| k.need.met(s))
}

pub fn unlocked_count(s: &Stats) -> usize {
    SKINS.iter().filter(|k| k.need.met(s)).count()
}

pub fn newly_unlocked(before: &Stats, after: &Stats) -> Vec<usize> {
    (0..SKINS.len())
        .filter(|i| !unlocked(*i, before) && unlocked(*i, after))
        .collect()
}

fn hue(t: f32) -> Color {
    let h = t.rem_euclid(1.0) * 6.0;
    let x = 1.0 - (h % 2.0 - 1.0).abs();
    let (r, g, b) = match h as u32 {
        0 => (1.0, x, 0.0),
        1 => (x, 1.0, 0.0),
        2 => (0.0, 1.0, x),
        3 => (0.0, x, 1.0),
        4 => (x, 0.0, 1.0),
        _ => (1.0, 0.0, x),
    };
    Color::new(0.35 + r * 0.65, 0.35 + g * 0.65, 0.35 + b * 0.65, 1.0)
}

impl Skin {
    pub fn segment(&self, i: usize, n: usize) -> Color {
        match self.pattern {
            Pattern::Solid => self.a,
            Pattern::Stripes => {
                if (i / 2).is_multiple_of(2) {
                    self.a
                } else {
                    self.b
                }
            }
            Pattern::Dots => {
                if i % 3 == 2 {
                    self.b
                } else {
                    self.a
                }
            }
            Pattern::Rainbow => hue(i as f32 / 14.0),
            Pattern::Fade => self.a.lerp(self.b, 0.5 - 0.5 * (i as f32 * 0.35).cos()),
        }
        .darker(1.0 - 0.18 * (i as f32 / n.max(1) as f32))
    }

    pub fn head(&self) -> Color {
        match self.pattern {
            Pattern::Rainbow => hue(0.0),
            Pattern::Stripes | Pattern::Dots => self.a,
            _ => self.a.lerp(self.b, 0.15),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_skins_are_free_and_every_skin_is_reachable() {
        assert_eq!(SKINS.iter().filter(|k| k.need == Need::Free).count(), 4);
        let all = Stats {
            best_level: 99,
            eaten: 999,
            floor: 99,
            games: 99,
            wins: 99,
            rating: 2000,
        };
        assert_eq!(unlocked_count(&all), SKINS.len());
    }

    #[test]
    fn no_two_skins_share_a_look_or_a_name() {
        for (i, a) in SKINS.iter().enumerate() {
            for b in &SKINS[i + 1..] {
                assert_ne!(a.name, b.name);
                assert!(
                    a.a != b.a || a.b != b.b || a.pattern != b.pattern,
                    "{} and {} look the same",
                    a.name,
                    b.name
                );
            }
        }
    }

    #[test]
    fn playing_unlocks_skins_in_steps_not_all_at_once() {
        let none = Stats::default();
        let one = Stats {
            best_level: 5,
            games: 1,
            ..Default::default()
        };
        assert_eq!(newly_unlocked(&none, &one), vec![4]);
        assert!(unlocked_count(&one) < SKINS.len() / 2);
    }

    #[test]
    fn patterns_alternate_along_the_body() {
        let candy = &SKINS[4];
        assert_ne!(candy.segment(0, 20), candy.segment(2, 20));
        let rainbow = &SKINS[9];
        assert_ne!(rainbow.segment(0, 20), rainbow.segment(5, 20));
    }
}
