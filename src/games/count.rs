use crate::engine::*;
use crate::fx::*;
use crate::learn::*;
use macroquad::prelude::*;

pub const RANGES: [(i32, i32); 5] = [(1, 5), (3, 8), (5, 10), (8, 15), (11, 20)];
pub const MAX_COUNT: i32 = 20;
const TICK_BOUNCE: f32 = 0.45;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Thing {
    Apple,
    Star,
    Fish,
    Duck,
}

impl Thing {
    pub const ALL: [Thing; 4] = [Thing::Apple, Thing::Star, Thing::Fish, Thing::Duck];

    pub fn plural(self) -> &'static str {
        match self {
            Thing::Apple => "apples",
            Thing::Star => "stars",
            Thing::Fish => "fish",
            Thing::Duck => "ducks",
        }
    }
}

pub fn level_name(level: usize) -> String {
    format!("Count to {}", RANGES[level].1)
}

pub fn choice_count(level: usize) -> usize {
    if level < 2 {
        3
    } else {
        4
    }
}

#[derive(Clone, Debug)]
pub struct Question {
    pub thing: Thing,
    pub count: i32,
    pub options: Vec<i32>,
    pub seed: u32,
}

impl Question {
    pub fn generate(level: usize) -> Self {
        let (low, high) = RANGES[level];
        Self::with_count(level, rand::gen_range(low, high + 1))
    }

    pub fn with_count(level: usize, count: i32) -> Self {
        let pool: Vec<i32> = (1..=4)
            .flat_map(|d| [count - d, count + d])
            .filter(|v| (1..=MAX_COUNT).contains(v))
            .collect();
        Self {
            thing: Thing::ALL[rand::gen_range(0, Thing::ALL.len())],
            count,
            options: choices(count, pool, choice_count(level)),
            seed: rand::gen_range(0u32, u32::MAX),
        }
    }

    pub fn fresh(level: usize, previous: Option<&Question>) -> Self {
        let mut q = Self::generate(level);
        for _ in 0..8 {
            if previous.is_none_or(|p| p.count != q.count) {
                break;
            }
            q = Self::generate(level);
        }
        q
    }

    pub fn slot_of_answer(&self) -> usize {
        self.options
            .iter()
            .position(|v| *v == self.count)
            .unwrap_or(0)
    }
}

fn mix(seed: u32, k: u32) -> u32 {
    let mut x = seed ^ k.wrapping_mul(0x9E37_79B1);
    x ^= x >> 15;
    x = x.wrapping_mul(0x85EB_CA6B);
    x ^= x >> 13;
    x = x.wrapping_mul(0xC2B2_AE35);
    x ^ (x >> 16)
}

fn unit(x: u32) -> f32 {
    (x >> 8) as f32 / 16_777_216.0
}

pub fn scatter(count: usize, seed: u32, area: Rect) -> (Vec<Vec2>, f32) {
    let cols = ((count as f32 * area.w / area.h).sqrt().ceil() as usize).clamp(1, count.max(1));
    let rows = count.div_ceil(cols).max(1);
    let cols = count.div_ceil(rows).max(1);
    let (cw, ch) = (area.w / cols as f32, area.h / rows as f32);
    let radius = (cw.min(ch) * 0.5 * 0.8).min(70.0);
    let mut cells: Vec<usize> = (0..cols * rows).collect();
    cells.sort_by_key(|c| mix(seed, *c as u32));
    cells.truncate(count);
    let slack = vec2((cw / 2.0 - radius).max(0.0), (ch / 2.0 - radius).max(0.0));
    let points = cells
        .iter()
        .map(|c| {
            let center = vec2(
                area.x + (c % cols) as f32 * cw + cw / 2.0,
                area.y + (c / cols) as f32 * ch + ch / 2.0,
            );
            let jitter = vec2(
                unit(mix(seed, *c as u32 + 1000)) * 2.0 - 1.0,
                unit(mix(seed, *c as u32 + 2000)) * 2.0 - 1.0,
            );
            center + jitter * slack * 0.9
        })
        .collect();
    (points, radius)
}

pub fn play_area(view: &View) -> Rect {
    Rect::new(50.0, 192.0, view.w - 100.0, view.h - 192.0 - 210.0)
}

pub fn button_rects(view: &View, n: usize) -> Vec<Rect> {
    let gap = 24.0;
    let w = ((view.w - 80.0 - gap * (n as f32 - 1.0)) / n as f32).min(240.0);
    let h = 124.0;
    let left = (view.w - w * n as f32 - gap * (n as f32 - 1.0)) / 2.0;
    (0..n)
        .map(|i| Rect::new(left + i as f32 * (w + gap), view.h - h - 40.0, w, h))
        .collect()
}

pub struct Count {
    pub ladder: Ladder,
    pub question: Question,
    pub attempt: Attempt,
    pub ticks: Vec<Option<u32>>,
    bounce: Vec<f32>,
    wobble: Vec<f32>,
    press: Vec<Bump>,
    celebrate: Option<f32>,
    streak_bump: Bump,
    prompt_bump: Bump,
    shake: Shake,
    particles: Particles,
    popups: Popups,
    t: f32,
}

impl Count {
    pub fn new() -> Self {
        let question = Question::fresh(0, None);
        let mut g = Self {
            ladder: Ladder::new(RANGES.len()),
            attempt: Attempt::new(0),
            ticks: vec![],
            bounce: vec![],
            wobble: vec![],
            press: vec![],
            celebrate: None,
            question,
            streak_bump: Bump::default(),
            prompt_bump: Bump::default(),
            shake: Shake::default(),
            particles: Particles::default(),
            popups: Popups::default(),
            t: 0.0,
        };
        g.load();
        g
    }

    fn load(&mut self) {
        let n = self.question.count as usize;
        let options = self.question.options.len();
        self.attempt = Attempt::new(options);
        self.ticks = vec![None; n];
        self.bounce = vec![TICK_BOUNCE; n];
        self.wobble = vec![0.0; options];
        self.press = (0..options).map(|_| Bump::default()).collect();
        self.celebrate = None;
        self.prompt_bump.kick();
        self.t = 0.0;
    }

    fn next_question(&mut self) {
        self.question = Question::fresh(self.ladder.level, Some(&self.question));
        self.load();
    }

    pub fn ticked(&self) -> u32 {
        self.ticks.iter().flatten().count() as u32
    }

    pub fn tick_object(&mut self, i: usize) -> bool {
        if self.celebrate.is_some() || self.ticks.get(i).is_none_or(|t| t.is_some()) {
            return false;
        }
        self.ticks[i] = Some(self.ticked() + 1);
        self.bounce[i] = 0.0;
        true
    }

    pub fn pick(&mut self, slot: usize) -> Tap {
        if self.celebrate.is_some() || slot >= self.question.options.len() {
            return Tap::Ignored;
        }
        let correct = self.question.options[slot] == self.question.count;
        let result = self.attempt.choose(slot, correct, &mut self.ladder);
        match result {
            Tap::Right(_) => self.celebrate = Some(0.0),
            Tap::Wrong | Tap::Reveal | Tap::Again => self.wobble[slot] = 1.0,
            Tap::Ignored => {}
        }
        if result != Tap::Ignored {
            self.press[slot].kick();
        }
        result
    }

    fn celebration_length(&self) -> f32 {
        self.question.count as f32 * 0.05 + 1.0
    }

    fn react(&mut self, view: &View, slot: usize, tap: Tap) {
        let gold = Color::from_rgba(255, 220, 70, 255);
        let orange = Color::from_rgba(255, 170, 90, 255);
        let at = button_rects(view, self.question.options.len())[slot].center();
        match tap {
            Tap::Right(step) => {
                self.particles.sparkle(at, gold);
                self.particles.emit(
                    at,
                    &[gold, WHITE, hsv(0.55, 0.5, 1.0), hsv(0.9, 0.5, 1.0)],
                    Burst {
                        count: 30,
                        speed: (200.0, 520.0),
                        shape: Shape::Star,
                        size: (6.0, 12.0),
                        angle: -std::f32::consts::FRAC_PI_2,
                        spread: 2.4,
                        ..Default::default()
                    },
                );
                self.streak_bump.kick();
                let word = if self.attempt.wrong == 0 {
                    PRAISE[rand::gen_range(0, PRAISE.len())]
                } else {
                    "That's it!"
                };
                let mid = vec2(view.w / 2.0, view.h * 0.4);
                self.popups.add(mid, word, gold, 90.0);
                match step {
                    Step::Up => {
                        self.particles.confetti(view);
                        self.popups.add(
                            mid + vec2(0.0, 90.0),
                            format!("Level up: {}", level_name(self.ladder.level)),
                            WHITE,
                            50.0,
                        );
                    }
                    Step::Cleared => {
                        self.particles.confetti(view);
                        self.popups
                            .add(mid + vec2(0.0, 90.0), "You can count to 20!", WHITE, 50.0);
                    }
                    Step::Down => self.popups.add(
                        mid + vec2(0.0, 90.0),
                        format!("Let's practise: {}", level_name(self.ladder.level)),
                        WHITE,
                        44.0,
                    ),
                    Step::Same => {}
                }
            }
            Tap::Wrong => {
                self.shake.add(0.25);
                self.popups
                    .add(at - vec2(0.0, 90.0), "Try again", orange, 48.0);
            }
            Tap::Reveal => {
                self.shake.add(0.25);
                let text = format!("It's {}", self.question.count);
                self.popups.add(at - vec2(0.0, 90.0), text, orange, 48.0);
            }
            Tap::Again | Tap::Ignored => {}
        }
    }

    fn number_shown(&self, i: usize) -> Option<u32> {
        match self.celebrate {
            Some(t) if t > i as f32 * 0.05 => Some(i as u32 + 1),
            _ => self.ticks[i],
        }
    }
}

impl Default for Count {
    fn default() -> Self {
        Self::new()
    }
}

impl Game for Count {
    fn update(&mut self, input: &Input, view: &View, dt: f32) {
        self.t += dt;
        self.streak_bump.update(dt);
        self.prompt_bump.update(dt);
        for b in &mut self.press {
            b.update(dt);
        }
        for b in &mut self.bounce {
            *b = (*b + dt).min(TICK_BOUNCE);
        }
        for w in &mut self.wobble {
            *w = (*w - dt * 1.8).max(0.0);
        }
        let end = self.celebration_length();
        if let Some(t) = &mut self.celebrate {
            *t += dt;
            if *t >= end {
                self.next_question();
            }
        } else if input.pressed {
            let buttons = button_rects(view, self.question.options.len());
            if let Some(slot) = buttons.iter().position(|r| r.contains(input.pos)) {
                let tap = self.pick(slot);
                self.react(view, slot, tap);
            } else {
                let q = &self.question;
                let (points, r) = scatter(q.count as usize, q.seed, play_area(view));
                let hit = points
                    .iter()
                    .position(|p| p.distance(input.pos) <= r * 1.15);
                if let Some(i) = hit {
                    if self.tick_object(i) {
                        let col = hsv(i as f32 * 0.13, 0.5, 1.0);
                        self.particles.sparkle(points[i], col);
                    }
                }
            }
        }
        self.shake.update(dt);
        self.particles.update(dt);
        self.popups.update(dt);
    }

    fn draw(&self, view: &View) {
        sky(
            view,
            Color::from_rgba(255, 226, 150, 255),
            Color::from_rgba(185, 235, 170, 255),
        );
        drifting_clouds(view, self.t);
        hud(
            view,
            &self.ladder,
            &level_name(self.ladder.level),
            Color::from_rgba(250, 170, 60, 255),
            &self.streak_bump,
        );
        set_view_camera(view, self.shake.offset());
        self.draw_prompt(view);
        self.draw_things(view);
        self.draw_buttons(view);
        set_view_camera(view, Vec2::ZERO);
        self.particles.draw();
        self.popups.draw();
    }
}

impl Count {
    fn draw_prompt(&self, view: &View) {
        let s = format!("How many {}?", self.question.thing.plural());
        let pop = ease_out_back(self.prompt_bump.t).max(0.2);
        text_outline(
            &s,
            view.w / 2.0,
            132.0,
            62.0 * pop,
            WHITE,
            Color::from_rgba(40, 30, 60, 255),
        );
    }

    fn draw_things(&self, view: &View) {
        let area = play_area(view);
        round_rect(
            Rect::new(area.x - 20.0, area.y - 14.0, area.w + 40.0, area.h + 28.0),
            36.0,
            with_alpha(WHITE, 0.4),
        );
        let q = &self.question;
        let (points, r) = scatter(q.count as usize, q.seed, area);
        for (i, p) in points.iter().enumerate() {
            let k = (self.bounce[i] / TICK_BOUNCE).clamp(0.0, 1.0);
            let hop = (k * std::f32::consts::PI).sin() * r * 0.6;
            let party = self.celebrate.map_or(0.0, |t| {
                ((t * 9.0 - i as f32 * 0.7).sin().max(0.0)) * r * 0.25
            });
            let squash = 1.0 + 0.25 * (1.0 - ease_out_cubic(k)) * (k < 1.0) as i32 as f32;
            let c = *p - vec2(0.0, hop + party);
            soft_shadow(*p + vec2(0.0, r * 0.85), r * 0.7, r * 0.16, 0.6);
            let intro = ease_out_back((self.t * 2.0 - i as f32 * 0.04).clamp(0.0, 1.0));
            draw_thing(q.thing, c, r * squash * intro, self.t + i as f32);
            if let Some(n) = self.number_shown(i) {
                let b = r * 0.55;
                draw_circle(
                    c.x,
                    c.y + r * 0.1,
                    b + 3.0,
                    Color::from_rgba(70, 60, 130, 255),
                );
                draw_circle(c.x, c.y + r * 0.1, b, WHITE);
                text_center(
                    &n.to_string(),
                    c.x,
                    c.y + r * 0.1,
                    b * 1.25,
                    Color::from_rgba(70, 60, 130, 255),
                );
            }
        }
    }

    fn draw_buttons(&self, view: &View) {
        let rects = button_rects(view, self.question.options.len());
        for (i, (r, v)) in rects.iter().zip(&self.question.options).enumerate() {
            let tried = self.attempt.tried[i];
            let hint = self.attempt.revealed() && *v == self.question.count;
            let pop = self.press[i].scale(0.12);
            let wob = (self.wobble[i] * 36.0).sin() * 12.0 * self.wobble[i];
            let c = r.center() + vec2(wob, 0.0);
            let rr = Rect::new(
                c.x - r.w * pop / 2.0,
                c.y - r.h * pop / 2.0,
                r.w * pop,
                r.h * pop,
            );
            if hint {
                let pulse = (self.t * 6.0).sin();
                let g = 10.0 + 4.0 * pulse;
                round_rect(
                    Rect::new(rr.x - g, rr.y - g, rr.w + g * 2.0, rr.h + g * 2.0),
                    24.0,
                    Color::new(1.0, 0.95, 0.4, 0.7),
                );
            }
            let correct_now = self.attempt.solved && *v == self.question.count;
            let color = if correct_now {
                Color::from_rgba(70, 190, 100, 255)
            } else {
                hsv(0.02 + i as f32 * 0.17, 0.62, 0.95)
            };
            let label = v.to_string();
            button(rr, &label, color, !tried);
        }
    }
}

fn draw_thing(thing: Thing, c: Vec2, r: f32, t: f32) {
    match thing {
        Thing::Apple => draw_apple(c, r),
        Thing::Star => {
            let wiggle = (t * 2.0).sin() * 0.15;
            star(
                c + vec2(0.0, r * 0.07),
                r * 1.25,
                wiggle,
                Color::from_rgba(200, 140, 20, 255),
            );
            star(c, r * 1.2, wiggle, Color::from_rgba(255, 215, 60, 255));
            draw_circle(
                c.x - r * 0.18,
                c.y - r * 0.25,
                r * 0.16,
                Color::new(1.0, 1.0, 1.0, 0.5),
            );
        }
        Thing::Fish => draw_fish(c, r, t),
        Thing::Duck => draw_duck(c, r),
    }
}

fn draw_apple(c: Vec2, r: f32) {
    let red = Color::from_rgba(235, 60, 60, 255);
    let dark = Color::from_rgba(170, 30, 45, 255);
    let body = vec2(c.x, c.y + r * 0.1);
    draw_circle(body.x - r * 0.28, body.y, r * 0.72, dark);
    draw_circle(body.x + r * 0.28, body.y, r * 0.72, dark);
    draw_circle(body.x, body.y + r * 0.08, r * 0.78, dark);
    draw_circle(body.x - r * 0.28, body.y - r * 0.04, r * 0.7, red);
    draw_circle(body.x + r * 0.28, body.y - r * 0.04, r * 0.7, red);
    draw_circle(body.x, body.y + r * 0.04, r * 0.76, red);
    draw_rectangle(
        c.x - r * 0.06,
        c.y - r * 0.85,
        r * 0.12,
        r * 0.38,
        Color::from_rgba(110, 70, 40, 255),
    );
    draw_ellipse(
        c.x + r * 0.3,
        c.y - r * 0.6,
        r * 0.3,
        r * 0.14,
        -0.5,
        Color::from_rgba(90, 190, 80, 255),
    );
    draw_ellipse(
        c.x - r * 0.35,
        c.y - r * 0.15,
        r * 0.14,
        r * 0.24,
        0.5,
        Color::new(1.0, 1.0, 1.0, 0.45),
    );
}

fn draw_fish(c: Vec2, r: f32, t: f32) {
    let body = Color::from_rgba(255, 140, 50, 255);
    let dark = Color::from_rgba(215, 95, 30, 255);
    let flick = (t * 6.0).sin() * r * 0.12;
    draw_triangle(
        vec2(c.x - r * 0.6, c.y),
        vec2(c.x - r * 1.15, c.y - r * 0.5 + flick),
        vec2(c.x - r * 1.15, c.y + r * 0.5 + flick),
        dark,
    );
    draw_triangle(
        vec2(c.x - r * 0.1, c.y - r * 0.45),
        vec2(c.x + r * 0.3, c.y - r * 0.85),
        vec2(c.x + r * 0.45, c.y - r * 0.4),
        dark,
    );
    draw_ellipse(c.x, c.y, r * 0.95, r * 0.62, 0.0, body);
    draw_ellipse(
        c.x + r * 0.05,
        c.y + r * 0.22,
        r * 0.7,
        r * 0.28,
        0.0,
        Color::from_rgba(255, 190, 110, 255),
    );
    draw_triangle(
        vec2(c.x - r * 0.05, c.y),
        vec2(c.x + r * 0.2, c.y + r * 0.35),
        vec2(c.x - r * 0.3, c.y + r * 0.3),
        dark,
    );
    draw_circle(c.x + r * 0.5, c.y - r * 0.12, r * 0.17, WHITE);
    draw_circle(
        c.x + r * 0.55,
        c.y - r * 0.12,
        r * 0.08,
        Color::from_rgba(30, 30, 50, 255),
    );
}

fn draw_duck(c: Vec2, r: f32) {
    let yellow = Color::from_rgba(255, 215, 60, 255);
    let shade = Color::from_rgba(235, 175, 30, 255);
    draw_ellipse(c.x - r * 0.05, c.y + r * 0.3, r * 0.9, r * 0.6, 0.0, shade);
    draw_ellipse(
        c.x - r * 0.05,
        c.y + r * 0.26,
        r * 0.88,
        r * 0.58,
        0.0,
        yellow,
    );
    draw_triangle(
        vec2(c.x - r * 0.8, c.y + r * 0.1),
        vec2(c.x - r * 1.1, c.y - r * 0.15),
        vec2(c.x - r * 0.6, c.y + r * 0.5),
        yellow,
    );
    draw_circle(c.x + r * 0.35, c.y - r * 0.4, r * 0.46, yellow);
    draw_triangle(
        vec2(c.x + r * 0.7, c.y - r * 0.5),
        vec2(c.x + r * 1.15, c.y - r * 0.32),
        vec2(c.x + r * 0.72, c.y - r * 0.2),
        Color::from_rgba(255, 130, 40, 255),
    );
    draw_circle(
        c.x + r * 0.45,
        c.y - r * 0.55,
        r * 0.09,
        Color::from_rgba(30, 30, 50, 255),
    );
    draw_ellipse(c.x - r * 0.15, c.y + r * 0.3, r * 0.4, r * 0.25, 0.3, shade);
    draw_circle(
        c.x + r * 0.6,
        c.y - r * 0.3,
        r * 0.1,
        Color::from_rgba(255, 150, 140, 160),
    );
}

pub fn icon(r: Rect) {
    draw_rectangle(r.x, r.y, r.w, r.h, Color::from_rgba(255, 224, 140, 255));
    draw_apple(vec2(r.x + r.w * 0.26, r.y + r.h * 0.3), r.w * 0.2);
    draw_apple(vec2(r.x + r.w * 0.7, r.y + r.h * 0.27), r.w * 0.2);
    draw_apple(vec2(r.x + r.w * 0.48, r.y + r.h * 0.6), r.w * 0.2);
    text_outline(
        "3",
        r.x + r.w * 0.84,
        r.y + r.h * 0.78,
        r.h * 0.4,
        WHITE,
        Color::from_rgba(40, 30, 60, 255),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn views() -> [View; 4] {
        [
            View::ipad_landscape(),
            View::ipad_portrait(),
            View::fit(390.0, 844.0),
            View::fit(1920.0, 1080.0),
        ]
    }

    #[test]
    fn counts_stay_inside_each_level_range() {
        rand::srand(5);
        for (level, (low, high)) in RANGES.iter().enumerate() {
            for _ in 0..300 {
                let q = Question::generate(level);
                assert!((*low..=*high).contains(&q.count), "{q:?}");
            }
        }
        assert_eq!(RANGES[0].0, 1);
        assert_eq!(RANGES[RANGES.len() - 1].1, MAX_COUNT);
    }

    #[test]
    fn choices_have_one_right_answer_and_no_duplicates() {
        rand::srand(9);
        for (level, (low, high)) in RANGES.iter().enumerate() {
            for count in *low..=*high {
                for _ in 0..30 {
                    let q = Question::with_count(level, count);
                    assert_eq!(q.options.len(), choice_count(level));
                    assert_eq!(q.options.iter().filter(|v| **v == count).count(), 1);
                    let mut sorted = q.options.clone();
                    sorted.sort_unstable();
                    sorted.dedup();
                    assert_eq!(sorted.len(), q.options.len(), "{q:?}");
                    assert!(q.options.iter().all(|v| (1..=MAX_COUNT).contains(v)));
                    assert_eq!(q.options[q.slot_of_answer()], count);
                }
            }
        }
    }

    #[test]
    fn the_next_count_differs_from_the_last() {
        rand::srand(21);
        let mut q = Question::fresh(0, None);
        for _ in 0..200 {
            let next = Question::fresh(0, Some(&q));
            assert_ne!(q.count, next.count);
            q = next;
        }
    }

    #[test]
    fn scattered_things_never_overlap_and_stay_in_the_play_area() {
        for v in views() {
            let area = play_area(&v);
            for count in 1..=MAX_COUNT as usize {
                for seed in [1u32, 77, 4_000_000_000] {
                    let (points, r) = scatter(count, seed, area);
                    assert_eq!(points.len(), count);
                    for (i, p) in points.iter().enumerate() {
                        assert!(
                            p.x - r >= area.x - 0.01
                                && p.x + r <= area.x + area.w + 0.01
                                && p.y - r >= area.y - 0.01
                                && p.y + r <= area.y + area.h + 0.01,
                            "{count} in {v:?}"
                        );
                        for q in &points[i + 1..] {
                            assert!(p.distance(*q) >= 2.0 * r - 0.01, "{count} in {v:?}");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn scatter_is_stable_for_a_seed_and_varies_between_seeds() {
        let area = play_area(&View::ipad_landscape());
        assert_eq!(scatter(9, 3, area).0, scatter(9, 3, area).0);
        assert_ne!(scatter(9, 3, area).0, scatter(9, 4, area).0);
    }

    #[test]
    fn things_are_big_enough_to_tap_even_at_twenty() {
        for v in views() {
            let (_, r) = scatter(20, 1, play_area(&v));
            assert!(r >= 35.0, "{r} in {v:?}");
        }
    }

    #[test]
    fn answer_buttons_are_big_and_fit_on_screen() {
        for v in views() {
            for n in [3, 4] {
                let rects = button_rects(&v, n);
                for (i, r) in rects.iter().enumerate() {
                    assert!(r.w >= 90.0 && r.h >= 90.0, "{r:?}");
                    assert!(r.x >= 0.0 && r.x + r.w <= v.w && r.y + r.h <= v.h);
                    if i > 0 {
                        assert!(r.x >= rects[i - 1].x + rects[i - 1].w);
                    }
                }
                assert!(rects[0].y >= play_area(&v).y + play_area(&v).h);
            }
        }
    }

    #[test]
    fn ticking_numbers_things_in_the_order_tapped() {
        let mut g = Count::new();
        g.question = Question::with_count(0, 4);
        g.load();
        assert!(g.tick_object(2));
        assert!(g.tick_object(0));
        assert!(!g.tick_object(2));
        assert_eq!(g.ticks[2], Some(1));
        assert_eq!(g.ticks[0], Some(2));
        assert_eq!(g.ticked(), 2);
        assert!(!g.tick_object(999));
    }

    #[test]
    fn tapping_a_thing_in_the_play_area_ticks_it() {
        let v = View::ipad_landscape();
        let mut g = Count::new();
        let (points, _) = scatter(g.question.count as usize, g.question.seed, play_area(&v));
        g.update(&Input::press_at(points[0]), &v, 1.0 / 60.0);
        assert_eq!(g.ticks[0], Some(1));
        assert!(!g.particles.list.is_empty());
    }

    #[test]
    fn right_button_celebrates_then_moves_to_a_new_question() {
        let v = View::ipad_landscape();
        let mut g = Count::new();
        let before = g.question.clone();
        let slot = before.slot_of_answer();
        let at = button_rects(&v, before.options.len())[slot].center();
        g.update(&Input::press_at(at), &v, 1.0 / 60.0);
        assert!(g.celebrate.is_some());
        assert_eq!(g.ladder.streak, 1);
        assert!(!g.popups.list.is_empty());
        assert!(!g.tick_object(0));
        for _ in 0..240 {
            g.update(&Input::default(), &v, 1.0 / 60.0);
        }
        assert!(g.celebrate.is_none());
        assert_ne!(g.question.count, before.count);
        assert!(g.ticks.iter().all(|t| t.is_none()));
    }

    #[test]
    fn wrong_buttons_shake_gently_then_show_the_answer() {
        let v = View::ipad_landscape();
        let mut g = Count::new();
        let right = g.question.slot_of_answer();
        let wrongs: Vec<usize> = (0..g.question.options.len())
            .filter(|i| *i != right)
            .collect();
        let rects = button_rects(&v, g.question.options.len());
        g.update(&Input::press_at(rects[wrongs[0]].center()), &v, 1.0 / 60.0);
        assert!(g.shake.active());
        assert!(!g.attempt.revealed());
        g.update(&Input::press_at(rects[wrongs[1]].center()), &v, 1.0 / 60.0);
        assert!(g.attempt.revealed());
        assert!(g.celebrate.is_none());
        assert_eq!(g.ladder.level, 0);
    }

    #[test]
    fn counting_celebration_numbers_every_thing() {
        let mut g = Count::new();
        let slot = g.question.slot_of_answer();
        g.pick(slot);
        g.celebrate = Some(5.0);
        for i in 0..g.ticks.len() {
            assert_eq!(g.number_shown(i), Some(i as u32 + 1));
        }
    }

    #[test]
    fn four_right_answers_in_a_row_level_up() {
        let v = View::ipad_landscape();
        let mut g = Count::new();
        for k in 0..UP_STREAK {
            let slot = g.question.slot_of_answer();
            let tap = g.pick(slot);
            g.react(&v, slot, tap);
            if k + 1 == UP_STREAK {
                assert!(g.particles.list.iter().any(|p| p.shape == Shape::Confetti));
            }
            for _ in 0..240 {
                g.update(&Input::default(), &v, 1.0 / 60.0);
            }
        }
        assert_eq!(g.ladder.level, 1);
        assert!((RANGES[1].0..=RANGES[1].1).contains(&g.question.count));
    }
}
