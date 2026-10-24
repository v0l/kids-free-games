use crate::engine::draw_circle;
use crate::engine::*;
use crate::feel::{self, Impact};
use crate::fx::*;
use crate::learn::*;
use crate::sfx::{self, Sfx};
use macroquad::prelude::*;

mod art {
    crate::sprites!("mathpop");
}

pub const LEVEL_NAMES: [&str; 5] = [
    "Adding to 10",
    "Adding to 20",
    "Taking away",
    "Mixed up",
    "Times tables",
];
pub const BALLOONS: usize = 4;
const LEAVE_TIME: f32 = 1.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Add,
    Sub,
    Mul,
}

impl Op {
    pub fn symbol(self) -> &'static str {
        match self {
            Op::Add => "+",
            Op::Sub => "−",
            Op::Mul => "×",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Question {
    pub a: i32,
    pub b: i32,
    pub op: Op,
    pub answer: i32,
    pub options: Vec<i32>,
}

fn operands(level: usize) -> (i32, i32, Op) {
    match level {
        0 => {
            let a = rand::gen_range(1, 10);
            (a, rand::gen_range(1, 11 - a), Op::Add)
        }
        1 => {
            let a = rand::gen_range(2, 19);
            let low = (11 - a).max(1);
            (a, rand::gen_range(low, 21 - a), Op::Add)
        }
        2 => {
            let a = rand::gen_range(3, 21);
            (a, rand::gen_range(1, a), Op::Sub)
        }
        3 => operands(rand::gen_range(0, 3)),
        _ => (rand::gen_range(2, 11), rand::gen_range(1, 11), Op::Mul),
    }
}

fn solve(a: i32, b: i32, op: Op) -> i32 {
    match op {
        Op::Add => a + b,
        Op::Sub => a - b,
        Op::Mul => a * b,
    }
}

impl Question {
    pub fn generate(level: usize) -> Self {
        let (a, b, op) = operands(level);
        Self::from_parts(a, b, op)
    }

    pub fn from_parts(a: i32, b: i32, op: Op) -> Self {
        let answer = solve(a, b, op);
        let mut pool: Vec<i32> = (1..=4).flat_map(|d| [answer - d, answer + d]).collect();
        match op {
            Op::Add => pool.extend([a - b, answer + 10, answer - 10]),
            Op::Sub => pool.extend([a + b, answer + 10]),
            Op::Mul => pool.extend([(a - 1) * b, (a + 1) * b, a * (b - 1), a * (b + 1), a + b]),
        }
        pool.retain(|v| *v >= 0);
        Self {
            a,
            b,
            op,
            answer,
            options: choices(answer, pool, BALLOONS),
        }
    }

    pub fn fresh(level: usize, previous: Option<&Question>) -> Self {
        let mut q = Self::generate(level);
        for _ in 0..8 {
            let same = previous.is_some_and(|p| (p.a, p.b, p.op) == (q.a, q.b, q.op));
            if !same {
                break;
            }
            q = Self::generate(level);
        }
        q
    }

    pub fn text(&self, solved: bool) -> String {
        let end = if solved {
            self.answer.to_string()
        } else {
            "?".to_string()
        };
        format!("{} {} {} = {}", self.a, self.op.symbol(), self.b, end)
    }

    pub fn slot_of_answer(&self) -> usize {
        self.options
            .iter()
            .position(|v| *v == self.answer)
            .unwrap_or(0)
    }
}

const WIDEST_QUESTION_EM: f32 = 6.6;
const QUESTION_PAD_EM: f32 = 0.6;

pub fn question_text_size(view: &View) -> f32 {
    (88.0 * ui_scale(view)).min((view.w - 40.0) / (WIDEST_QUESTION_EM + QUESTION_PAD_EM))
}

pub fn question_rect(view: &View) -> Rect {
    let size = question_text_size(view);
    let w = (size * (WIDEST_QUESTION_EM + QUESTION_PAD_EM))
        .max(360.0)
        .min(view.w - 20.0);
    Rect::new((view.w - w) / 2.0, hud_bottom(view) + 24.0, w, size * 1.25)
}

fn grid(view: &View) -> (usize, Rect) {
    let top = question_rect(view).bottom() + 30.0;
    let cols = if tall(view) { 2 } else { BALLOONS };
    (cols, Rect::new(0.0, top, view.w, view.h - 40.0 - top))
}

pub fn balloon_radius(view: &View) -> f32 {
    let (cols, area) = grid(view);
    let rows = BALLOONS.div_ceil(cols) as f32;
    let cap = if tall(view) { f32::MAX } else { 86.0 };
    (area.w / cols as f32 * 0.36)
        .min(area.h / rows / 4.0)
        .min(cap)
}

pub fn balloon_rest(view: &View, slot: usize) -> Vec2 {
    let (cols, area) = grid(view);
    let rows = BALLOONS.div_ceil(cols);
    let r = balloon_radius(view);
    let (col, row) = (slot % cols, slot / cols);
    let cell = vec2(area.w / cols as f32, area.h / rows as f32);
    let amp = ((cell.y - 3.2 * r) / 2.0 * 0.9).clamp(0.0, 70.0);
    let stagger = [-1.0, 0.6, -0.5, 1.0][slot % 4];
    vec2(
        cell.x * (col as f32 + 0.5),
        area.y + cell.y * (row as f32 + 0.5) - r * 0.6 + stagger * amp,
    )
}

#[derive(Clone, Debug)]
struct Balloon {
    value: i32,
    hue: f32,
    wobble: f32,
    tried: bool,
    popped: bool,
}

pub struct MathPop {
    pub ladder: Ladder,
    pub question: Question,
    pub attempt: Attempt,
    balloons: Vec<Balloon>,
    leave: Option<f32>,
    asked: f32,
    streak_bump: Bump,
    question_bump: Bump,
    particles: Particles,
    popups: Popups,
    t: f32,
}

impl MathPop {
    pub fn new() -> Self {
        let ladder = Ladder::new(LEVEL_NAMES.len());
        let question = Question::fresh(0, None);
        let mut g = Self {
            ladder,
            attempt: Attempt::new(BALLOONS),
            balloons: vec![],
            question,
            leave: None,
            asked: 0.0,
            streak_bump: Bump::default(),
            question_bump: Bump::default(),
            particles: Particles::default(),
            popups: Popups::default(),
            t: 0.0,
        };
        g.load_balloons();
        g
    }

    fn load_balloons(&mut self) {
        let offset = rand::gen_range(0.0, 1.0);
        self.balloons = self
            .question
            .options
            .iter()
            .enumerate()
            .map(|(i, v)| Balloon {
                value: *v,
                hue: offset + i as f32 * 0.19,
                wobble: 0.0,
                tried: false,
                popped: false,
            })
            .collect();
        self.attempt = Attempt::new(self.question.options.len());
        self.leave = None;
        self.asked = 0.0;
        self.question_bump.kick();
    }

    fn next_question(&mut self) {
        self.question = Question::fresh(self.ladder.level, Some(&self.question));
        self.load_balloons();
    }

    pub fn pick(&mut self, slot: usize) -> Tap {
        let Some(b) = self.balloons.get(slot) else {
            return Tap::Ignored;
        };
        if b.popped {
            return Tap::Ignored;
        }
        let correct = b.value == self.question.answer;
        let result = self.attempt.choose(slot, correct, &mut self.ladder);
        match result {
            Tap::Right(_) => {
                self.balloons[slot].popped = true;
                self.leave = Some(0.0);
            }
            Tap::Wrong | Tap::Reveal => {
                self.balloons[slot].tried = true;
                self.balloons[slot].wobble = 1.0;
            }
            Tap::Again => self.balloons[slot].wobble = 1.0,
            Tap::Ignored => {}
        }
        result
    }

    fn center(&self, view: &View, slot: usize) -> Vec2 {
        let rest = balloon_rest(view, slot);
        let rise = ease_out_cubic((self.asked - slot as f32 * 0.1) / 0.9);
        let bob = (self.t * 1.6 + slot as f32 * 1.7).sin() * 8.0;
        let lift = self.leave.map_or(0.0, |l| l * l * (view.h + 400.0));
        let wob = (self.balloons[slot].wobble * 38.0).sin() * 14.0 * self.balloons[slot].wobble;
        vec2(
            rest.x + wob,
            rest.y + bob + (1.0 - rise) * (view.h - rest.y + 220.0) - lift,
        )
    }

    fn react(&mut self, view: &View, slot: usize, tap: Tap) {
        let c = self.center(view, slot);
        let color = hsv(self.balloons[slot].hue, 0.65, 1.0);
        let gold = Color::from_rgba(255, 220, 70, 255);
        match tap {
            Tap::Right(step) => {
                sfx::play(Sfx::Pop);
                sfx::play(match step {
                    Step::Up | Step::Cleared => Sfx::Win,
                    _ => Sfx::Right,
                });
                if step == Step::Same {
                    sfx::play_step(Sfx::Right, self.ladder.streak as usize * 2);
                }
                if matches!(step, Step::Up | Step::Cleared) {
                    feel::impact_tinted(Impact::Large, Color::from_rgba(255, 230, 120, 255));
                }
                self.particles.emit(
                    c,
                    &[color, WHITE, gold],
                    Burst {
                        count: 26,
                        shape: Shape::Circle,
                        size: (5.0, 11.0),
                        ..Default::default()
                    },
                );
                self.particles.sparkle(c, gold);
                self.streak_bump.kick();
                sfx::say(match step {
                    Step::Up => sfx::Say::LevelUp,
                    Step::Cleared => sfx::Say::Congrats,
                    _ if self.attempt.wrong == 0 => sfx::Say::Great,
                    _ => sfx::Say::Correct,
                });
                let first_try = self.attempt.wrong == 0;
                let word = if first_try {
                    PRAISE[rand::gen_range(0, PRAISE.len())]
                } else {
                    "That's it!"
                };
                self.popups.add(c, word, gold, 60.0);
                let mid = vec2(view.w / 2.0, view.h * 0.36);
                match step {
                    Step::Up => {
                        self.particles.confetti(view);
                        self.popups.add(mid, "Level up!", gold, 88.0);
                        self.popups.add(
                            mid + vec2(0.0, 80.0),
                            LEVEL_NAMES[self.ladder.level],
                            WHITE,
                            48.0,
                        );
                    }
                    Step::Cleared => {
                        self.particles.confetti(view);
                        self.popups.add(mid, "You did it!", gold, 88.0);
                    }
                    Step::Down => {
                        self.popups.add(
                            mid,
                            format!("Let's practise {}", LEVEL_NAMES[self.ladder.level]),
                            WHITE,
                            44.0,
                        );
                    }
                    Step::Same => {}
                }
            }
            Tap::Wrong => {
                sfx::play(Sfx::Wrong);
                self.popups
                    .add(c, "Try again", Color::from_rgba(255, 170, 90, 255), 48.0);
            }
            Tap::Reveal => {
                sfx::play(Sfx::Wrong);
                let text = format!("It's {}", self.question.answer);
                self.popups
                    .add(c, text, Color::from_rgba(255, 170, 90, 255), 48.0);
            }
            Tap::Again | Tap::Ignored => {}
        }
    }
}

impl Default for MathPop {
    fn default() -> Self {
        Self::new()
    }
}

impl Game for MathPop {
    fn update(&mut self, input: &Input, view: &View, dt: f32) {
        self.t += dt;
        self.asked += dt;
        self.streak_bump.update(dt);
        self.question_bump.update(dt);
        for b in &mut self.balloons {
            b.wobble = (b.wobble - dt * 1.6).max(0.0);
        }
        if let Some(l) = &mut self.leave {
            *l += dt / LEAVE_TIME;
            if *l >= 1.0 {
                self.next_question();
            }
        } else if input.pressed {
            let r = balloon_radius(view) * 1.1;
            let hit = (0..self.balloons.len()).find(|i| {
                !self.balloons[*i].popped
                    && (input.pos - self.center(view, *i)).length() <= r.max(50.0)
            });
            if let Some(slot) = hit {
                let tap = self.pick(slot);
                self.react(view, slot, tap);
            }
        }
        self.particles.update(dt);
        self.popups.update(dt);
    }

    fn draw(&self, view: &View) {
        self.draw_backdrop(view);
        hud(
            view,
            &self.ladder,
            LEVEL_NAMES[self.ladder.level],
            Color::from_rgba(235, 100, 170, 255),
            &self.streak_bump,
        );
        self.draw_question(view);
        let r = balloon_radius(view);
        for (i, b) in self.balloons.iter().enumerate() {
            if b.popped {
                continue;
            }
            let c = self.center(view, i);
            let hint = self.attempt.revealed() && b.value == self.question.answer;
            if hint {
                let pulse = (self.t * 6.0).sin();
                draw_circle(
                    c.x,
                    c.y,
                    r * (1.3 + 0.06 * pulse),
                    Color::new(1.0, 0.95, 0.4, 0.4 + 0.15 * pulse),
                );
            }
            draw_balloon(c, r, b, self.t + i as f32);
        }
        self.particles.draw();
        self.popups.draw();
    }
}

impl MathPop {
    fn draw_backdrop(&self, view: &View) {
        let f = view.full();
        let side = f.w.max(f.h);
        art::sprite(
            "bg",
            Rect::new(f.x + (f.w - side) / 2.0, f.y + f.h - side, side, side),
        );
        for i in 0..5usize {
            let span = view.w + 500.0;
            let x = (i as f32 * 0.23 * span + self.t * (7.0 + i as f32 * 3.0)) % span - 250.0;
            let y = hud_bottom(view) + 20.0 + (i as f32 * 0.37 % 1.0) * view.h * 0.35;
            crate::ui::cloud(
                i * 2,
                vec2(x, y),
                150.0 + (i % 3) as f32 * 50.0,
                with_alpha(WHITE, 0.9),
            );
        }
        self.draw_bunnies(view);
    }

    fn mood(&self) -> &'static str {
        if self.leave.is_some() {
            "up"
        } else if self.balloons.iter().any(|b| b.wobble > 0.3) {
            "sad"
        } else {
            "stand"
        }
    }

    fn draw_bunnies(&self, view: &View) {
        let h = 150.0 * ui_scale(view);
        let hop = match self.leave {
            Some(l) => (l * 14.0).sin().abs() * h * 0.2,
            None => 0.0,
        };
        let names = match self.mood() {
            "up" => ["bunny_up", "pink_up"],
            "sad" => ["bunny_sad", "pink_sad"],
            _ => ["bunny", "pink"],
        };
        let y = view.h - h * 0.5 - 10.0 - hop;
        for (i, name) in names.iter().enumerate() {
            let side = if i == 0 { 1.0 } else { -1.0 };
            let x = if i == 0 { 70.0 } else { view.w - 70.0 };
            art::sprite_at(
                name,
                vec2(x, y),
                h,
                crate::art::Draw {
                    flip_x: side < 0.0,
                    ..Default::default()
                },
            );
        }
    }

    fn draw_question(&self, view: &View) {
        let solved = self.attempt.solved;
        let text = self.question.text(solved);
        let size = question_text_size(view);
        let pop = ease_out_back(self.question_bump.t).max(0.2);
        let base = question_rect(view);
        let c = base.center();
        let r = Rect::new(
            c.x - base.w * pop / 2.0,
            c.y - base.h * pop / 2.0,
            base.w * pop,
            base.h * pop,
        );
        crate::ui::chip_box(r);
        let ink = if solved {
            Color::from_rgba(60, 170, 90, 255)
        } else {
            Color::from_rgba(70, 60, 130, 255)
        };
        text_center(&text, c.x, c.y, size * pop, ink);
    }
}

fn draw_balloon(c: Vec2, r: f32, b: &Balloon, t: f32) {
    let color = hsv(b.hue, 0.65, 1.0);
    let fade = if b.tried { 0.5 } else { 1.0 };
    let squash = 1.0 + 0.06 * (b.wobble * 30.0).sin() * b.wobble;
    let rx = r * 0.88 * squash;
    let ry = r / squash;
    let sway = (t * 1.3).sin() * 10.0;
    let string = with_alpha(Color::from_rgba(90, 80, 120, 255), fade);
    let mut prev = vec2(c.x, c.y + ry + 12.0);
    for k in 1..=8 {
        let f = k as f32 / 8.0;
        let p = vec2(
            c.x + sway * (f * 3.0).sin() * f,
            c.y + ry + 12.0 + f * r * 1.1,
        );
        draw_line(prev.x, prev.y, p.x, p.y, 3.0, string);
        prev = p;
    }
    draw_triangle(
        vec2(c.x, c.y + ry - 4.0),
        vec2(c.x - 11.0, c.y + ry + 14.0),
        vec2(c.x + 11.0, c.y + ry + 14.0),
        with_alpha(darker(color, 0.8), fade),
    );
    let rect = Rect::new(c.x - rx * 1.04, c.y - ry * 1.04, rx * 2.08, ry * 2.08);
    art::sprite_ex(
        "ball",
        Rect::new(rect.x, rect.y + 6.0, rect.w, rect.h),
        crate::art::Draw {
            tint: Color::new(0.0, 0.0, 0.0, 0.15 * fade),
            ..Default::default()
        },
    );
    art::sprite_ex(
        "ball",
        rect,
        crate::art::Draw {
            tint: with_alpha(color, fade),
            ..Default::default()
        },
    );
    text_outline(
        &b.value.to_string(),
        c.x,
        c.y,
        r * 0.95,
        with_alpha(WHITE, fade),
        with_alpha(darker(color, 0.5), fade),
    );
}

pub fn icon(r: Rect) {
    art::sprite("bg", r);
    crate::ui::cloud(
        2,
        vec2(r.x + r.w * 0.78, r.y + r.h * 0.16),
        r.w * 0.34,
        with_alpha(WHITE, 0.9),
    );
    art::sprite_at(
        "bunny_up",
        vec2(r.x + r.w * 0.8, r.y + r.h * 0.84),
        r.h * 0.34,
        crate::art::Draw {
            flip_x: true,
            ..Default::default()
        },
    );
    for (c, rad, hue, label) in [
        (
            vec2(r.x + r.w * 0.34, r.y + r.h * 0.42),
            r.w * 0.27,
            0.97,
            "5",
        ),
        (
            vec2(r.x + r.w * 0.72, r.y + r.h * 0.5),
            r.w * 0.2,
            0.13,
            "+",
        ),
    ] {
        draw_line(
            c.x,
            c.y + rad,
            c.x + rad * 0.2,
            r.y + r.h * 0.95,
            2.0,
            WHITE,
        );
        art::sprite_ex(
            "ball",
            Rect::new(c.x - rad * 0.9, c.y - rad, rad * 1.8, rad * 2.0),
            crate::art::Draw {
                tint: hsv(hue, 0.65, 1.0),
                ..Default::default()
            },
        );
        text_outline(
            label,
            c.x,
            c.y,
            rad * 1.0,
            WHITE,
            darker(hsv(hue, 0.65, 1.0), 0.5),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mathpop_sprites_all_exist() {
        let have = art::names();
        for n in [
            "bg",
            "ball",
            "bunny",
            "bunny_up",
            "bunny_sad",
            "pink",
            "pink_up",
            "pink_sad",
        ] {
            assert!(have.iter().any(|h| h == n), "missing {n}");
        }
    }

    #[test]
    fn right_answers_are_praised_aloud_and_wrong_ones_are_not() {
        let v = View::ipad_landscape();
        let mut g = game_with_first_try_answers();
        let wrong = wrong_slot(&g);
        let tap = g.pick(wrong);
        g.react(&v, wrong, tap);
        assert_eq!(crate::sfx::take_voice(), None);
        let slot = g.question.slot_of_answer();
        let tap = g.pick(slot);
        g.react(&v, slot, tap);
        assert_eq!(crate::sfx::take_voice(), Some(sfx::Say::Correct));
    }

    #[test]
    fn first_try_answer_says_great_and_level_up_says_so() {
        let v = View::ipad_landscape();
        let mut g = game_with_first_try_answers();
        let slot = g.question.slot_of_answer();
        let tap = g.pick(slot);
        g.react(&v, slot, tap);
        assert_eq!(crate::sfx::take_voice(), Some(sfx::Say::Great));
        let mut g = game_with_first_try_answers();
        g.ladder.streak = UP_STREAK - 1;
        let slot = g.question.slot_of_answer();
        let tap = g.pick(slot);
        g.react(&v, slot, tap);
        assert_eq!(crate::sfx::take_voice(), Some(sfx::Say::LevelUp));
    }

    #[test]
    fn bunnies_cheer_after_a_pop_and_wince_after_a_miss() {
        let mut g = game_with_first_try_answers();
        assert_eq!(g.mood(), "stand");
        let wrong = wrong_slot(&g);
        g.pick(wrong);
        assert_eq!(g.mood(), "sad");
        let slot = g.question.slot_of_answer();
        g.pick(slot);
        assert_eq!(g.mood(), "up");
    }

    fn assert_sound(q: &Question) {
        assert_eq!(q.answer, solve(q.a, q.b, q.op));
        assert_eq!(q.options.len(), BALLOONS);
        assert_eq!(q.options.iter().filter(|v| **v == q.answer).count(), 1);
        let mut sorted = q.options.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), BALLOONS, "{q:?}");
        assert!(q.options.iter().all(|v| *v >= 0), "{q:?}");
        assert!(q.answer >= 1);
    }

    #[test]
    fn every_level_makes_sound_questions() {
        rand::srand(7);
        for level in 0..LEVEL_NAMES.len() {
            for _ in 0..400 {
                assert_sound(&Question::generate(level));
            }
        }
    }

    #[test]
    fn edge_questions_still_find_four_distinct_choices() {
        for (a, b, op) in [
            (1, 1, Op::Add),
            (3, 2, Op::Sub),
            (3, 1, Op::Sub),
            (2, 1, Op::Mul),
            (10, 10, Op::Mul),
            (18, 2, Op::Add),
            (20, 19, Op::Sub),
        ] {
            for _ in 0..50 {
                assert_sound(&Question::from_parts(a, b, op));
            }
        }
    }

    #[test]
    fn adding_levels_stay_inside_their_limits() {
        rand::srand(11);
        for _ in 0..400 {
            let q = Question::generate(0);
            assert!(q.op == Op::Add && q.a >= 1 && q.b >= 1 && q.answer <= 10);
            let q = Question::generate(1);
            assert!(q.op == Op::Add && (11..=20).contains(&q.answer) && q.b >= 1);
        }
    }

    #[test]
    fn taking_away_never_goes_negative() {
        rand::srand(13);
        for _ in 0..400 {
            let q = Question::generate(2);
            assert!(q.op == Op::Sub && q.b < q.a && q.answer >= 1 && q.a <= 20);
        }
    }

    #[test]
    fn mixed_level_uses_every_adding_and_taking_away_op() {
        rand::srand(17);
        let ops: Vec<Op> = (0..300).map(|_| Question::generate(3).op).collect();
        assert!(ops.contains(&Op::Add) && ops.contains(&Op::Sub));
        assert!(!ops.contains(&Op::Mul));
    }

    #[test]
    fn times_tables_stay_up_to_ten() {
        rand::srand(19);
        for _ in 0..400 {
            let q = Question::generate(4);
            assert!(q.op == Op::Mul && (2..=10).contains(&q.a) && (1..=10).contains(&q.b));
        }
    }

    #[test]
    fn the_next_question_differs_from_the_last() {
        rand::srand(23);
        let mut q = Question::fresh(0, None);
        for _ in 0..200 {
            let next = Question::fresh(0, Some(&q));
            assert_ne!((q.a, q.b, q.op), (next.a, next.b, next.op));
            q = next;
        }
    }

    #[test]
    fn question_text_hides_then_shows_the_answer() {
        let q = Question::from_parts(7, 5, Op::Add);
        assert_eq!(q.text(false), "7 + 5 = ?");
        assert_eq!(q.text(true), "7 + 5 = 12");
        assert_eq!(q.options[q.slot_of_answer()], 12);
    }

    fn game_with_first_try_answers() -> MathPop {
        let mut g = MathPop::new();
        g.ladder = Ladder::new(LEVEL_NAMES.len());
        g
    }

    fn wrong_slot(g: &MathPop) -> usize {
        (0..BALLOONS)
            .find(|i| *i != g.question.slot_of_answer() && !g.balloons[*i].tried)
            .unwrap()
    }

    #[test]
    fn tapping_the_right_balloon_pops_it_and_starts_leaving() {
        let mut g = game_with_first_try_answers();
        let slot = g.question.slot_of_answer();
        assert_eq!(g.pick(slot), Tap::Right(Step::Same));
        assert!(g.balloons[slot].popped);
        assert!(g.leave.is_some());
        assert_eq!(g.ladder.streak, 1);
    }

    #[test]
    fn a_wrong_tap_wobbles_and_a_second_one_shows_the_answer() {
        let mut g = game_with_first_try_answers();
        let first = wrong_slot(&g);
        assert_eq!(g.pick(first), Tap::Wrong);
        assert_eq!(g.balloons[first].wobble, 1.0);
        assert!(!g.attempt.revealed());
        let second = wrong_slot(&g);
        assert_eq!(g.pick(second), Tap::Reveal);
        assert!(g.attempt.revealed());
        assert!(g.leave.is_none());
    }

    #[test]
    fn four_first_try_answers_level_up_and_keep_going() {
        let v = View::ipad_landscape();
        let mut g = game_with_first_try_answers();
        for _ in 0..UP_STREAK {
            let slot = g.question.slot_of_answer();
            let tap = g.pick(slot);
            g.react(&v, slot, tap);
            for _ in 0..90 {
                g.update(&Input::default(), &v, 1.0 / 60.0);
            }
        }
        assert_eq!(g.ladder.level, 1);
        assert!(!g.attempt.solved);
        assert!(g.question.answer >= 2);
    }

    #[test]
    fn level_up_rains_confetti() {
        let v = View::ipad_landscape();
        let mut g = game_with_first_try_answers();
        g.ladder.streak = UP_STREAK - 1;
        let slot = g.question.slot_of_answer();
        let tap = g.pick(slot);
        assert_eq!(tap, Tap::Right(Step::Up));
        g.react(&v, slot, tap);
        assert!(g.particles.list.iter().any(|p| p.shape == Shape::Confetti));
    }

    #[test]
    fn tapping_during_the_exit_is_ignored() {
        let v = View::ipad_landscape();
        let mut g = game_with_first_try_answers();
        let slot = g.question.slot_of_answer();
        g.pick(slot);
        g.asked = 5.0;
        let other = wrong_slot(&g);
        let at = g.center(&v, other);
        g.update(&Input::press_at(at), &v, 1.0 / 60.0);
        assert_eq!(g.attempt.wrong, 0);
    }

    #[test]
    fn a_touch_on_a_balloon_picks_it() {
        let v = View::ipad_portrait();
        let mut g = game_with_first_try_answers();
        g.asked = 5.0;
        let slot = g.question.slot_of_answer();
        let at = g.center(&v, slot);
        g.update(&Input::press_at(at), &v, 1.0 / 60.0);
        assert!(g.attempt.solved);
    }

    fn phone() -> View {
        View::fit(1080.0, 2400.0)
    }

    #[test]
    fn balloons_are_big_touch_targets_and_fit_on_screen() {
        for v in [
            View::ipad_landscape(),
            View::ipad_portrait(),
            View::fit(390.0, 844.0),
            View::fit(1920.0, 1080.0),
            phone(),
        ] {
            let r = balloon_radius(&v);
            assert!(r * 2.0 >= 90.0 * 1.5);
            let top = question_rect(&v).bottom();
            for slot in 0..BALLOONS {
                let c = balloon_rest(&v, slot);
                assert!(c.x - r >= 0.0 && c.x + r <= v.w, "{v:?}");
                assert!(c.y - r >= top && c.y + r * 2.3 <= v.h, "{v:?}");
            }
            let (cols, _) = grid(&v);
            for slot in 1..BALLOONS {
                if slot % cols != 0 {
                    let gap = balloon_rest(&v, slot).x - balloon_rest(&v, slot - 1).x;
                    assert!(gap > r * 1.8);
                }
            }
        }
    }

    #[test]
    fn on_a_tall_phone_the_balloons_use_most_of_the_height() {
        let v = phone();
        assert!(v.h > 2000.0);
        let r = balloon_radius(&v);
        assert!(r >= 140.0, "{r}");
        let ys: Vec<f32> = (0..BALLOONS).map(|i| balloon_rest(&v, i).y).collect();
        let top = ys.iter().cloned().fold(f32::MAX, f32::min) - r;
        let bottom = ys.iter().cloned().fold(f32::MIN, f32::max) + r * 2.3;
        assert!(bottom - top >= v.h * 0.5, "{top} {bottom} of {}", v.h);
        assert!(question_rect(&v).h >= 150.0);
        assert!(question_text_size(&v) >= 120.0);
    }
}
