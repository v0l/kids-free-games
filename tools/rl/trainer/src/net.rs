use candle_core::{DType, Device, Result, Tensor, D};
use candle_nn::{conv2d, linear, Conv2d, Conv2dConfig, Linear, VarBuilder, VarMap};
use kg_core::brain::conv::{Layer, Policy};
use kg_core::brain::learn::Spec;

struct Unfold {
    index: Tensor,
    side_out: usize,
    k: usize,
}

impl Unfold {
    fn new(side_in: usize, k: usize, stride: usize, pad: usize, dev: &Device) -> Result<Self> {
        let side_out = (side_in + 2 * pad - k) / stride + 1;
        let zero = (side_in * side_in) as u32;
        let mut idx = Vec::with_capacity(side_out * side_out * k * k);
        for oy in 0..side_out {
            for ox in 0..side_out {
                for ky in 0..k {
                    for kx in 0..k {
                        let y = (oy * stride + ky) as isize - pad as isize;
                        let x = (ox * stride + kx) as isize - pad as isize;
                        let inside = (0..side_in as isize).contains(&y)
                            && (0..side_in as isize).contains(&x);
                        idx.push(if inside {
                            (y as usize * side_in + x as usize) as u32
                        } else {
                            zero
                        });
                    }
                }
            }
        }
        let n = idx.len();
        Ok(Self {
            index: Tensor::from_vec(idx, n, dev)?,
            side_out,
            k,
        })
    }

    fn conv(&self, x: &Tensor, c: &Conv2d, dt: DType) -> Result<Tensor> {
        let (n, cin, _) = x.dims3()?;
        let k = self.k;
        let cout = c.weight().dim(0)?;
        let x = Tensor::cat(&[x, &Tensor::zeros((n, cin, 1), x.dtype(), x.device())?], 2)?;
        let p = self.side_out * self.side_out;
        let cols = x
            .index_select(&self.index, 2)?
            .reshape((n, cin, p, k * k))?
            .transpose(1, 2)?
            .reshape((n * p, cin * k * k))?;
        let wm = c.weight().to_dtype(dt)?.reshape((cout, cin * k * k))?;
        let y = cols
            .matmul(&wm.t()?)?
            .broadcast_add(&c.bias().unwrap().to_dtype(dt)?)?
            .relu()?;
        y.reshape((n, p, cout))?.transpose(1, 2)?.contiguous()
    }
}

fn dense(l: &Linear, x: &Tensor, dt: DType) -> Result<Tensor> {
    let w = l.weight().to_dtype(dt)?;
    let y = x.matmul(&w.t()?)?;
    match l.bias() {
        Some(b) => y.broadcast_add(&b.to_dtype(dt)?),
        None => Ok(y),
    }
}

pub struct Net {
    pub dtype: DType,
    pub spec: Spec,
    convs: Vec<(Conv2d, usize, usize, usize, usize)>,
    unfolds: Vec<Unfold>,
    actor: (Linear, Linear),
    critic: (Linear, Linear),
}

const WIDTH: usize = 32;
const HIDDEN: usize = 128;

fn cfg(stride: usize, padding: usize) -> Conv2dConfig {
    Conv2dConfig {
        stride,
        padding,
        ..Default::default()
    }
}

impl Net {
    pub fn new(spec: Spec, vb: VarBuilder) -> Result<Self> {
        let mut convs = vec![];
        let mut unfolds = vec![];
        let mut cin = spec.channels;
        let mut side = spec.side;
        for k in 0..3 {
            let c = conv2d(cin, WIDTH, 3, cfg(2, 1), vb.pp(format!("conv{k}")))?;
            convs.push((c, cin, WIDTH, 3, 2));
            unfolds.push(Unfold::new(side, 3, 2, 1, vb.device())?);
            cin = WIDTH;
            side = (side + 2 - 3) / 2 + 1;
        }
        let flat = WIDTH * side * side + spec.scalars;
        let actor = (
            linear(flat, HIDDEN, vb.pp("a1"))?,
            linear(HIDDEN, spec.actions, vb.pp("a2"))?,
        );
        let critic = (
            linear(flat, HIDDEN, vb.pp("c1"))?,
            linear(HIDDEN, 1, vb.pp("c2"))?,
        );
        Ok(Self {
            dtype: DType::F32,
            spec,
            convs,
            unfolds,
            actor,
            critic,
        })
    }

    pub fn frozen(&self) -> Net {
        let conv = |c: &Conv2d| {
            Conv2d::new(
                c.weight().detach(),
                c.bias().map(|b| b.detach()),
                *c.config(),
            )
        };
        let lin = |l: &Linear| Linear::new(l.weight().detach(), l.bias().map(|b| b.detach()));
        Net {
            dtype: self.dtype,
            spec: Spec { ..self.spec },
            convs: self
                .convs
                .iter()
                .map(|(c, a, b, k, st)| (conv(c), *a, *b, *k, *st))
                .collect(),
            unfolds: self
                .unfolds
                .iter()
                .map(|u| Unfold {
                    index: u.index.clone(),
                    side_out: u.side_out,
                    k: u.k,
                })
                .collect(),
            actor: (lin(&self.actor.0), lin(&self.actor.1)),
            critic: (lin(&self.critic.0), lin(&self.critic.1)),
        }
    }

    fn trunk(&self, grid: &Tensor, scalars: &Tensor) -> Result<Tensor> {
        let dt = self.dtype;
        let n = grid.dim(0)?;
        let mut x =
            grid.to_dtype(dt)?
                .reshape((n, self.spec.channels, self.spec.side * self.spec.side))?;
        for ((c, ..), u) in self.convs.iter().zip(&self.unfolds) {
            x = u.conv(&x, c, dt)?;
        }
        Tensor::cat(&[&x.flatten_from(1)?, &scalars.to_dtype(dt)?], 1)
    }

    fn head(&self, h: &Tensor, (a, b): &(Linear, Linear)) -> Result<Tensor> {
        let dt = self.dtype;
        dense(b, &dense(a, h, dt)?.relu()?, dt)?.to_dtype(DType::F32)
    }

    pub fn act(&self, grid: &Tensor, scalars: &Tensor) -> Result<Tensor> {
        let h = self.trunk(grid, scalars)?;
        self.head(&h, &self.actor)
    }

    pub fn forward(&self, grid: &Tensor, scalars: &Tensor) -> Result<(Tensor, Tensor)> {
        let h = self.trunk(grid, scalars)?;
        let logits = self.head(&h, &self.actor)?;
        let value = self.head(&h, &self.critic)?.squeeze(D::Minus1)?;
        Ok((logits, value))
    }

    pub fn export(&self) -> Result<Policy> {
        let host =
            |t: &Tensor| -> Result<Vec<f32>> { t.flatten_all()?.to_dtype(DType::F32)?.to_vec1() };
        let mut trunk = vec![];
        for (c, cin, cout, k, stride) in &self.convs {
            trunk.push(Layer::Conv {
                cin: *cin,
                cout: *cout,
                k: *k,
                stride: *stride,
                pad: 1,
                w: host(c.weight())?,
                b: host(c.bias().unwrap())?,
            });
            trunk.push(Layer::Relu);
        }
        let dense = |l: &Linear| -> Result<Layer> {
            let (o, i) = l.weight().dims2()?;
            Ok(Layer::Dense {
                n_in: i,
                n_out: o,
                w: host(l.weight())?,
                b: host(l.bias().unwrap())?,
            })
        };
        Ok(Policy {
            grid: self.spec.side,
            channels: self.spec.channels,
            scalars: self.spec.scalars,
            actions: self.spec.actions,
            trunk,
            head: vec![dense(&self.actor.0)?, Layer::Relu, dense(&self.actor.1)?],
        })
    }
}

pub fn build(spec: Spec, dev: &Device) -> Result<(VarMap, Net)> {
    let vm = VarMap::new();
    let vb = VarBuilder::from_varmap(&vm, DType::F32, dev);
    let net = Net::new(spec, vb)?;
    for (name, var) in vm.data().lock().unwrap().iter() {
        if name == "a2.weight" {
            var.set(&(var.as_tensor() * 0.01)?)?;
        }
        if name == "a2.bias" {
            var.set(&var.as_tensor().zeros_like()?)?;
        }
    }
    Ok((vm, net))
}
