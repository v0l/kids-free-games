use crate::rng as rand;

pub const MAGIC: &[u8; 4] = b"KGB2";

#[derive(Clone, Debug, PartialEq)]
pub enum Layer {
    Conv {
        cin: usize,
        cout: usize,
        k: usize,
        stride: usize,
        pad: usize,
        w: Vec<f32>,
        b: Vec<f32>,
    },
    Dense {
        n_in: usize,
        n_out: usize,
        w: Vec<f32>,
        b: Vec<f32>,
    },
    Relu,
    Tanh,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Policy {
    pub grid: usize,
    pub channels: usize,
    pub scalars: usize,
    pub actions: usize,
    pub trunk: Vec<Layer>,
    pub head: Vec<Layer>,
}

#[derive(Clone, Debug, Default)]
pub struct Obs {
    pub grid: Vec<f32>,
    pub scalars: Vec<f32>,
}

fn conv(
    x: &[f32],
    side: usize,
    (cin, cout, k, stride, pad): (usize, usize, usize, usize, usize),
    w: &[f32],
    b: &[f32],
    cols: &mut Vec<f32>,
) -> (Vec<f32>, usize) {
    let out = (side + 2 * pad - k) / stride + 1;
    let positions = out * out;
    let taps = cin * k * k;
    cols.clear();
    cols.resize(taps * positions, 0.0);
    for c in 0..cin {
        let src = &x[c * side * side..(c + 1) * side * side];
        for ky in 0..k {
            for kx in 0..k {
                let row = &mut cols[((c * k + ky) * k + kx) * positions..][..positions];
                for oy in 0..out {
                    let iy = (oy * stride + ky) as isize - pad as isize;
                    if iy < 0 || iy >= side as isize {
                        continue;
                    }
                    let line = &src[iy as usize * side..][..side];
                    let dst = &mut row[oy * out..][..out];
                    for (ox, d) in dst.iter_mut().enumerate() {
                        let ix = (ox * stride + kx) as isize - pad as isize;
                        if ix >= 0 && ix < side as isize {
                            *d = line[ix as usize];
                        }
                    }
                }
            }
        }
    }
    let mut y = vec![0.0f32; cout * positions];
    for o in 0..cout {
        let acc = &mut y[o * positions..][..positions];
        acc.fill(b[o]);
        let wrow = &w[o * taps..][..taps];
        for (t, &wv) in wrow.iter().enumerate() {
            let col = &cols[t * positions..][..positions];
            for (a, &v) in acc.iter_mut().zip(col) {
                *a += wv * v;
            }
        }
    }
    (y, out)
}

fn dot(a: &[f32], b: &[f32]) -> f32 {
    let mut lanes = [0.0f32; 8];
    let (ca, cb) = (a.as_chunks::<8>(), b.as_chunks::<8>());
    let (ra, rb) = (ca.1, cb.1);
    for (x, y) in ca.0.iter().zip(cb.0) {
        for i in 0..8 {
            lanes[i] += x[i] * y[i];
        }
    }
    let mut s: f32 = lanes.iter().sum();
    for (x, y) in ra.iter().zip(rb) {
        s += x * y;
    }
    s
}

fn run(layers: &[Layer], mut x: Vec<f32>, mut side: usize) -> Vec<f32> {
    let mut cols = Vec::new();
    for l in layers {
        match l {
            Layer::Conv {
                cin,
                cout,
                k,
                stride,
                pad,
                w,
                b,
            } => {
                let (y, out) = conv(&x, side, (*cin, *cout, *k, *stride, *pad), w, b, &mut cols);
                x = y;
                side = out;
            }
            Layer::Dense { n_in, n_out, w, b } => {
                debug_assert_eq!(x.len(), *n_in);
                let mut y = b.clone();
                for (o, yo) in y.iter_mut().enumerate().take(*n_out) {
                    *yo += dot(&w[o * n_in..(o + 1) * n_in], &x);
                }
                x = y;
            }
            Layer::Relu => x.iter_mut().for_each(|v| *v = v.max(0.0)),
            Layer::Tanh => x.iter_mut().for_each(|v| *v = v.tanh()),
        }
    }
    x
}

impl Policy {
    pub fn logits(&self, obs: &Obs) -> Vec<f32> {
        let mut x = run(&self.trunk, obs.grid.clone(), self.grid);
        x.extend_from_slice(&obs.scalars);
        run(&self.head, x, 0)
    }

    pub fn choose(&self, obs: &Obs, effort: super::Effort) -> usize {
        let logits = self.logits(obs);
        let t = effort.temperature();
        if t < 1e-4 {
            return super::best(&logits);
        }
        sample(&logits, 1.0 + t)
    }

    pub fn param_count(&self) -> usize {
        self.trunk
            .iter()
            .chain(&self.head)
            .map(|l| match l {
                Layer::Conv { w, b, .. } | Layer::Dense { w, b, .. } => w.len() + b.len(),
                _ => 0,
            })
            .sum()
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = MAGIC.to_vec();
        let u = |v: usize, out: &mut Vec<u8>| out.extend((v as u32).to_le_bytes());
        for v in [self.grid, self.channels, self.scalars, self.actions] {
            u(v, &mut out);
        }
        for part in [&self.trunk, &self.head] {
            u(part.len(), &mut out);
            for l in part.iter() {
                match l {
                    Layer::Conv {
                        cin,
                        cout,
                        k,
                        stride,
                        pad,
                        w,
                        b,
                    } => {
                        u(1, &mut out);
                        for v in [*cin, *cout, *k, *stride, *pad] {
                            u(v, &mut out);
                        }
                        floats(&mut out, w);
                        floats(&mut out, b);
                    }
                    Layer::Dense { n_in, n_out, w, b } => {
                        u(2, &mut out);
                        u(*n_in, &mut out);
                        u(*n_out, &mut out);
                        floats(&mut out, w);
                        floats(&mut out, b);
                    }
                    Layer::Relu => u(3, &mut out),
                    Layer::Tanh => u(4, &mut out),
                }
            }
        }
        out
    }

    pub fn from_bytes(b: &[u8]) -> Option<Self> {
        if b.get(..4)? != MAGIC {
            return None;
        }
        let mut r = Reader { b, at: 4 };
        let grid = r.u()?;
        let channels = r.u()?;
        let scalars = r.u()?;
        let actions = r.u()?;
        let trunk = r.layers()?;
        let head = r.layers()?;
        if r.at != b.len() {
            return None;
        }
        let p = Self {
            grid,
            channels,
            scalars,
            actions,
            trunk,
            head,
        };
        let probe = Obs {
            grid: vec![0.0; channels * grid * grid],
            scalars: vec![0.0; scalars],
        };
        (p.shape_ok() && p.logits(&probe).len() == actions).then_some(p)
    }

    fn shape_ok(&self) -> bool {
        let mut ch = self.channels;
        let mut side = self.grid;
        for l in &self.trunk {
            match l {
                Layer::Conv {
                    cin,
                    cout,
                    k,
                    stride,
                    pad,
                    ..
                } => {
                    if *cin != ch || *stride == 0 || side + 2 * pad < *k {
                        return false;
                    }
                    side = (side + 2 * pad - k) / stride + 1;
                    ch = *cout;
                }
                Layer::Dense { .. } => return false,
                _ => {}
            }
        }
        let mut n = ch * side * side + self.scalars;
        for l in &self.head {
            match l {
                Layer::Dense { n_in, n_out, .. } => {
                    if *n_in != n {
                        return false;
                    }
                    n = *n_out;
                }
                Layer::Conv { .. } => return false,
                _ => {}
            }
        }
        n == self.actions
    }
}

fn floats(out: &mut Vec<u8>, v: &[f32]) {
    for x in v {
        out.extend(x.to_le_bytes());
    }
}

struct Reader<'a> {
    b: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn u(&mut self) -> Option<usize> {
        let v = u32::from_le_bytes(self.b.get(self.at..self.at + 4)?.try_into().ok()?);
        self.at += 4;
        Some(v as usize)
    }

    fn fs(&mut self, n: usize) -> Option<Vec<f32>> {
        let end = self.at.checked_add(n.checked_mul(4)?)?;
        let raw = self.b.get(self.at..end)?;
        self.at = end;
        Some(
            raw.as_chunks::<4>()
                .0
                .iter()
                .map(|c| f32::from_le_bytes(*c))
                .collect(),
        )
    }

    fn layers(&mut self) -> Option<Vec<Layer>> {
        let n = self.u()?;
        let mut out = Vec::with_capacity(n.min(64));
        for _ in 0..n {
            out.push(match self.u()? {
                1 => {
                    let (cin, cout, k, stride, pad) =
                        (self.u()?, self.u()?, self.u()?, self.u()?, self.u()?);
                    let w = self.fs(cout * cin * k * k)?;
                    let b = self.fs(cout)?;
                    Layer::Conv {
                        cin,
                        cout,
                        k,
                        stride,
                        pad,
                        w,
                        b,
                    }
                }
                2 => {
                    let (n_in, n_out) = (self.u()?, self.u()?);
                    let w = self.fs(n_in * n_out)?;
                    let b = self.fs(n_out)?;
                    Layer::Dense { n_in, n_out, w, b }
                }
                3 => Layer::Relu,
                4 => Layer::Tanh,
                _ => return None,
            });
        }
        Some(out)
    }
}

pub fn sample(logits: &[f32], temperature: f32) -> usize {
    let top = logits.iter().copied().fold(f32::MIN, f32::max);
    let w: Vec<f32> = logits
        .iter()
        .map(|l| ((l - top) / temperature.max(1e-3)).exp())
        .collect();
    let total: f32 = w.iter().sum();
    let mut r = rand::gen_range(0.0, total);
    for (k, x) in w.iter().enumerate() {
        if r < *x {
            return k;
        }
        r -= x;
    }
    logits.len() - 1
}
