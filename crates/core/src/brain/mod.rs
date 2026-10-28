pub mod train;

use crate::rng as rand;

pub const MAGIC: &[u8; 4] = b"KGB1";

#[derive(Clone, Debug, PartialEq)]
pub struct Net {
    pub sizes: Vec<usize>,
    pub weights: Vec<f32>,
}

impl Net {
    pub fn new(sizes: &[usize]) -> Self {
        let n = Self::param_count(sizes);
        Self {
            sizes: sizes.to_vec(),
            weights: vec![0.0; n],
        }
    }

    pub fn random(sizes: &[usize], scale: f32) -> Self {
        let mut net = Self::new(sizes);
        let mut at = 0;
        for w in sizes.windows(2) {
            let fan = (w[0] as f32).sqrt().max(1.0);
            for _ in 0..w[0] * w[1] {
                net.weights[at] = gauss() * scale / fan;
                at += 1;
            }
            at += w[1];
        }
        net
    }

    pub fn param_count(sizes: &[usize]) -> usize {
        sizes.windows(2).map(|w| w[0] * w[1] + w[1]).sum()
    }

    pub fn inputs(&self) -> usize {
        self.sizes[0]
    }

    pub fn eval(&self, input: &[f32]) -> Vec<f32> {
        debug_assert_eq!(input.len(), self.inputs());
        let mut x = input.to_vec();
        let mut at = 0;
        let last = self.sizes.len() - 2;
        for (layer, w) in self.sizes.windows(2).enumerate() {
            let (n_in, n_out) = (w[0], w[1]);
            let mut y = vec![0.0; n_out];
            for (o, out) in y.iter_mut().enumerate() {
                let row = &self.weights[at + o * n_in..at + (o + 1) * n_in];
                let mut s = 0.0;
                for (a, b) in row.iter().zip(&x) {
                    s += a * b;
                }
                *out = s;
            }
            at += n_in * n_out;
            for (o, out) in y.iter_mut().enumerate() {
                *out += self.weights[at + o];
                if layer < last {
                    *out = out.tanh();
                }
            }
            at += n_out;
            x = y;
        }
        x
    }

    pub fn score(&self, features: &[f32]) -> f32 {
        self.eval(features)[0]
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = MAGIC.to_vec();
        out.extend((self.sizes.len() as u32).to_le_bytes());
        for s in &self.sizes {
            out.extend((*s as u32).to_le_bytes());
        }
        for w in &self.weights {
            out.extend(w.to_le_bytes());
        }
        out
    }

    pub fn from_bytes(b: &[u8]) -> Option<Self> {
        if b.get(..4)? != MAGIC {
            return None;
        }
        let word = |i: usize| -> Option<[u8; 4]> { b.get(i..i + 4)?.try_into().ok() };
        let layers = u32::from_le_bytes(word(4)?) as usize;
        let mut at = 8;
        let mut sizes = Vec::with_capacity(layers);
        for _ in 0..layers {
            sizes.push(u32::from_le_bytes(word(at)?) as usize);
            at += 4;
        }
        if sizes.len() < 2 || sizes.contains(&0) {
            return None;
        }
        let n = Self::param_count(&sizes);
        if b.len() != at + n * 4 {
            return None;
        }
        let weights = (0..n)
            .map(|k| f32::from_le_bytes(word(at + k * 4).unwrap()))
            .collect();
        Some(Self { sizes, weights })
    }
}

pub fn gauss() -> f32 {
    let u = rand::gen_range(1e-7f32, 1.0);
    let v = rand::gen_range(0.0f32, std::f32::consts::TAU);
    (-2.0 * u.ln()).sqrt() * v.cos()
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Effort(f32);

impl Effort {
    pub const BEST: Effort = Effort(1.0);

    pub fn new(e: f32) -> Self {
        Self(e.clamp(0.0, 1.0))
    }

    pub fn get(self) -> f32 {
        self.0
    }

    pub fn temperature(self) -> f32 {
        let lazy = 1.0 - self.0;
        2.5 * lazy * lazy
    }

    pub fn think_every(self, fastest: f32) -> f32 {
        fastest * (1.0 + 3.0 * (1.0 - self.0))
    }

    pub fn stubborn(self) -> f32 {
        0.45 * (1.0 - self.0)
    }
}

pub struct Brain {
    pub net: Net,
    pub effort: Effort,
    last: Option<usize>,
}

impl Brain {
    pub fn new(net: Net, effort: Effort) -> Self {
        Self {
            net,
            effort,
            last: None,
        }
    }

    pub fn load(bytes: &[u8], effort: Effort) -> Option<Self> {
        Net::from_bytes(bytes).map(|n| Self::new(n, effort))
    }

    pub fn scores(&self, options: &[Vec<f32>]) -> Vec<f32> {
        options.iter().map(|f| self.net.score(f)).collect()
    }

    pub fn choose(&mut self, options: &[Vec<f32>]) -> usize {
        if options.is_empty() {
            return 0;
        }
        let keep = self
            .last
            .filter(|l| *l < options.len())
            .filter(|_| rand::gen_range(0.0, 1.0) < self.effort.stubborn());
        let pick = keep.unwrap_or_else(|| pick_soft(&self.scores(options), self.effort));
        self.last = Some(pick);
        pick
    }

    pub fn forget(&mut self) {
        self.last = None;
    }
}

pub fn pick_soft(scores: &[f32], effort: Effort) -> usize {
    let t = effort.temperature();
    if t < 1e-4 {
        return best(scores);
    }
    let top = scores.iter().copied().fold(f32::MIN, f32::max);
    let weights: Vec<f32> = scores.iter().map(|s| ((s - top) / t).exp()).collect();
    let total: f32 = weights.iter().sum();
    let mut r = rand::gen_range(0.0, total);
    for (k, w) in weights.iter().enumerate() {
        if r < *w {
            return k;
        }
        r -= w;
    }
    scores.len() - 1
}

pub fn best(scores: &[f32]) -> usize {
    scores
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1))
        .map_or(0, |(k, _)| k)
}

#[cfg(test)]
mod tests;
