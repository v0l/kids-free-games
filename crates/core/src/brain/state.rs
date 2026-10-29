use super::conv::Obs;
use super::learn::Spec;
use glam::{vec2, Vec2};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mix {
    Max,
    Add,
    Set,
}

#[derive(Clone, Copy, Debug)]
pub struct Layer {
    pub name: &'static str,
    pub mix: Mix,
    pub near: bool,
    pub on_map: bool,
}

impl Layer {
    pub const fn max(name: &'static str) -> Self {
        Self {
            name,
            mix: Mix::Max,
            near: true,
            on_map: false,
        }
    }

    pub const fn add(name: &'static str) -> Self {
        Self {
            name,
            mix: Mix::Add,
            near: true,
            on_map: false,
        }
    }

    pub const fn set(name: &'static str) -> Self {
        Self {
            name,
            mix: Mix::Set,
            near: true,
            on_map: false,
        }
    }

    pub const fn mapped(self) -> Self {
        Self {
            on_map: true,
            ..self
        }
    }

    pub const fn map_only(self) -> Self {
        Self {
            near: false,
            on_map: true,
            ..self
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Mark {
    pub layer: usize,
    pub pos: Vec2,
    pub near: Option<f32>,
    pub map: Option<f32>,
}

impl Mark {
    pub fn near(layer: usize, pos: Vec2, v: f32) -> Self {
        Self {
            layer,
            pos,
            near: Some(v),
            map: None,
        }
    }

    pub fn map(layer: usize, pos: Vec2, v: f32) -> Self {
        Self {
            layer,
            pos,
            near: None,
            map: Some(v),
        }
    }

    pub fn both(layer: usize, pos: Vec2, near: f32, map: f32) -> Self {
        Self {
            layer,
            pos,
            near: Some(near),
            map: Some(map),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Lens {
    pub side: usize,
    pub world: Vec2,
}

pub struct Viewer {
    pub pos: Vec2,
    pub angle: f32,
    pub span: f32,
}

pub trait BrainState {
    fn layers() -> &'static [Layer];
    fn scalar_names() -> &'static [&'static str];
    fn lens(&self) -> Lens;
    fn viewer(&self, agent: usize) -> Viewer;
    fn marks(&self, agent: usize, out: &mut Vec<Mark>);
    fn scalars(&self, agent: usize, out: &mut Vec<f32>);
}

fn near_layers(layers: &[Layer]) -> impl Iterator<Item = (usize, &Layer)> {
    layers.iter().enumerate().filter(|(_, l)| l.near)
}

fn map_layers(layers: &[Layer]) -> impl Iterator<Item = (usize, &Layer)> {
    layers.iter().enumerate().filter(|(_, l)| l.on_map)
}

pub fn channels(layers: &[Layer]) -> usize {
    2 + near_layers(layers).count() + map_layers(layers).count()
}

pub fn spec<S: BrainState>(side: usize, actions: usize) -> Spec {
    Spec {
        channels: channels(S::layers()),
        side,
        scalars: S::scalar_names().len(),
        actions,
    }
}

pub fn channel_names<S: BrainState>() -> Vec<String> {
    let mut v = vec!["wall".to_string()];
    v.extend(near_layers(S::layers()).map(|(_, l)| l.name.to_string()));
    v.push("map wall".to_string());
    v.extend(map_layers(S::layers()).map(|(_, l)| format!("map {}", l.name)));
    v
}

pub fn near_channel(layers: &[Layer], layer: usize) -> Option<usize> {
    near_layers(layers)
        .position(|(k, _)| k == layer)
        .map(|p| 1 + p)
}

pub fn map_channel(layers: &[Layer], layer: usize) -> Option<usize> {
    let base = 2 + near_layers(layers).count();
    map_layers(layers)
        .position(|(k, _)| k == layer)
        .map(|p| base + p)
}

struct Frame {
    head: Vec2,
    fwd: Vec2,
    right: Vec2,
    cell: f32,
    side: usize,
}

impl Frame {
    fn place(&self, p: Vec2) -> Option<usize> {
        let half = self.side as f32 / 2.0;
        let rel = p - self.head;
        let y = half - rel.dot(self.fwd) / self.cell;
        let x = rel.dot(self.right) / self.cell + half;
        let n = self.side as f32;
        ((0.0..n).contains(&y) && (0.0..n).contains(&x))
            .then(|| y as usize * self.side + x as usize)
    }

    fn point(&self, y: usize, x: usize) -> Vec2 {
        let half = self.side as f32 / 2.0;
        let ahead = (half - y as f32 - 0.5) * self.cell;
        let side = (x as f32 + 0.5 - half) * self.cell;
        self.head + self.fwd * ahead + self.right * side
    }
}

fn mix(cell: &mut f32, how: Mix, v: f32) {
    match how {
        Mix::Max => *cell = cell.max(v),
        Mix::Add => *cell = (*cell + v).min(1.0),
        Mix::Set => *cell = v,
    }
}

pub fn look<S: BrainState>(s: &S, agent: usize) -> Obs {
    let layers = S::layers();
    let lens = s.lens();
    let area = lens.side * lens.side;
    let mut grid = vec![0.0f32; channels(layers) * area];
    let v = s.viewer(agent);
    let fwd = vec2(v.angle.cos(), v.angle.sin());
    let near = Frame {
        head: v.pos,
        fwd,
        right: vec2(-fwd.y, fwd.x),
        cell: v.span / lens.side as f32,
        side: lens.side,
    };
    let far = Frame {
        cell: lens.world.max_element() * 2.0 / lens.side as f32,
        ..near
    };
    let outside = |q: Vec2| q.x < 0.0 || q.y < 0.0 || q.x > lens.world.x || q.y > lens.world.y;
    let map_wall = (1 + near_layers(layers).count()) * area;
    for y in 0..lens.side {
        for x in 0..lens.side {
            let k = y * lens.side + x;
            if outside(near.point(y, x)) {
                grid[k] = 1.0;
            }
            if outside(far.point(y, x)) {
                grid[map_wall + k] = 1.0;
            }
        }
    }
    let close: Vec<Option<usize>> = (0..layers.len()).map(|l| near_channel(layers, l)).collect();
    let map: Vec<Option<usize>> = (0..layers.len()).map(|l| map_channel(layers, l)).collect();
    let mut marks = Vec::new();
    s.marks(agent, &mut marks);
    for m in &marks {
        let how = layers[m.layer].mix;
        let seen = (m.pos - v.pos).length() <= v.span * 0.5;
        if let (true, Some(c), Some(val), Some(k)) =
            (seen, close[m.layer], m.near, near.place(m.pos))
        {
            mix(&mut grid[c * area + k], how, val);
        }
        if let (Some(c), Some(val), Some(k)) = (map[m.layer], m.map, far.place(m.pos)) {
            mix(&mut grid[c * area + k], how, val);
        }
    }
    let mut scalars = Vec::with_capacity(S::scalar_names().len());
    s.scalars(agent, &mut scalars);
    debug_assert_eq!(scalars.len(), S::scalar_names().len());
    Obs { grid, scalars }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Toy {
        coin: Vec2,
    }

    static LAYERS: [Layer; 3] = [
        Layer::add("coins").mapped(),
        Layer::set("height"),
        Layer::max("rivals").map_only(),
    ];

    impl BrainState for Toy {
        fn layers() -> &'static [Layer] {
            &LAYERS
        }
        fn scalar_names() -> &'static [&'static str] {
            &["one"]
        }
        fn lens(&self) -> Lens {
            Lens {
                side: 8,
                world: vec2(1000.0, 1000.0),
            }
        }
        fn viewer(&self, _: usize) -> Viewer {
            Viewer {
                pos: vec2(500.0, 500.0),
                angle: 0.0,
                span: 80.0,
            }
        }
        fn marks(&self, _: usize, out: &mut Vec<Mark>) {
            out.push(Mark::both(0, self.coin, 0.6, 0.3));
            out.push(Mark::both(0, self.coin, 0.6, 0.3));
            out.push(Mark::near(1, self.coin, -0.5));
            out.push(Mark::map(2, self.coin, 1.0));
        }
        fn scalars(&self, _: usize, out: &mut Vec<f32>) {
            out.push(1.0);
        }
    }

    fn channel(o: &Obs, c: usize) -> &[f32] {
        &o.grid[c * 64..(c + 1) * 64]
    }

    #[test]
    fn a_thing_ahead_shows_above_the_centre_in_both_views() {
        let o = look(
            &Toy {
                coin: vec2(530.0, 500.0),
            },
            0,
        );
        assert_eq!(o.grid.len(), channels(&LAYERS) * 64);
        assert_eq!(
            channel_names::<Toy>(),
            [
                "wall",
                "coins",
                "height",
                "map wall",
                "map coins",
                "map rivals"
            ]
        );
        let coins = channel(&o, near_channel(&LAYERS, 0).unwrap());
        let at = coins.iter().position(|v| *v > 0.0).unwrap();
        assert_eq!((at / 8, at % 8), (1, 4));
        assert_eq!(coins[at], 1.0);
        assert_eq!(channel(&o, near_channel(&LAYERS, 1).unwrap())[at], -0.5);
        let map = channel(&o, map_channel(&LAYERS, 0).unwrap());
        assert!((map.iter().sum::<f32>() - 0.6).abs() < 1e-6);
        assert_eq!(o.scalars, [1.0]);
    }

    #[test]
    fn a_map_only_layer_has_no_near_channel() {
        let o = look(
            &Toy {
                coin: vec2(510.0, 500.0),
            },
            0,
        );
        assert_eq!(near_channel(&LAYERS, 2), None);
        assert_eq!(map_channel(&LAYERS, 1), None);
        let rivals = channel(&o, map_channel(&LAYERS, 2).unwrap());
        assert_eq!(rivals.iter().sum::<f32>(), 1.0);
    }

    #[test]
    fn near_marks_stop_at_the_edge_of_the_circle_any_screen_shows() {
        let corner = vec2(500.0 + 37.0, 500.0 + 37.0);
        let o = look(&Toy { coin: corner }, 0);
        assert!(channel(&o, near_channel(&LAYERS, 0).unwrap())
            .iter()
            .all(|v| *v == 0.0));
        let map = channel(&o, map_channel(&LAYERS, 0).unwrap());
        assert!(map.iter().sum::<f32>() > 0.0);
    }

    #[test]
    fn walls_appear_on_the_minimap_but_not_up_close_in_the_middle() {
        let o = look(
            &Toy {
                coin: vec2(0.0, 0.0),
            },
            0,
        );
        assert_eq!(channel(&o, 0).iter().sum::<f32>(), 0.0);
        assert!(channel(&o, 3).iter().sum::<f32>() > 10.0);
        assert!(channel(&o, 1).iter().all(|v| *v == 0.0));
    }
}
