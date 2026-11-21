use crate::net::Unfold;
use candle_core::{DType, Device, Result, Tensor, Var, D};
use candle_nn::VarMap;
use kg_core::brain::learn::Spec;

pub struct Layout {
    entries: Vec<(String, usize, Vec<usize>)>,
    pub total: usize,
}

impl Layout {
    pub fn of(vm: &VarMap) -> Self {
        let data = vm.data().lock().unwrap();
        let mut names: Vec<&String> = data.keys().collect();
        names.sort();
        let mut entries = vec![];
        let mut total = 0;
        for name in names {
            let shape = data[name].dims().to_vec();
            let len = shape.iter().product::<usize>();
            entries.push((name.clone(), total, shape));
            total += len;
        }
        Self { entries, total }
    }

    fn entry(&self, name: &str) -> &(String, usize, Vec<usize>) {
        self.entries
            .iter()
            .find(|e| e.0 == name)
            .expect("known parameter")
    }

    pub fn pack(&self, vm: &VarMap) -> Result<Vec<f32>> {
        let data = vm.data().lock().unwrap();
        let mut out = Vec::with_capacity(self.total);
        for (name, ..) in &self.entries {
            out.extend(data[name].flatten_all()?.to_vec1::<f32>()?);
        }
        Ok(out)
    }

    pub fn unpack(&self, row: &[f32], vm: &VarMap, dev: &Device) -> Result<()> {
        let data = vm.data().lock().unwrap();
        for (name, off, shape) in &self.entries {
            let len = shape.iter().product::<usize>();
            let t = Tensor::from_slice(&row[*off..off + len], shape.as_slice(), dev)?;
            data[name].set(&t)?;
        }
        Ok(())
    }
}

pub struct Weights {
    convs: Vec<Tensor>,
    a1: (Tensor, Tensor),
    a2: (Tensor, Tensor),
    c1: (Tensor, Tensor),
    c2: (Tensor, Tensor),
}

fn batched(l: &(Tensor, Tensor), x: &Tensor) -> Result<Tensor> {
    x.matmul(&l.0)?.broadcast_add(&l.1)
}

const CHANNELS_P: usize = 40;

pub struct PopNet {
    first_p: usize,
    width: usize,
    last_pix: usize,
    pub dtype: DType,
    spec: Spec,
    layout: Layout,
    unfolds: Vec<Unfold>,
}

impl PopNet {
    pub fn new(spec: Spec, layout: Layout, dtype: DType, dev: &Device) -> Result<Self> {
        let mut unfolds = vec![];
        let mut side = spec.side;
        for _ in 0..3 {
            unfolds.push(Unfold::new(side, 3, 2, 1, dev)?);
            side = (side + 2 - 3) / 2 + 1;
        }
        let last_pix = unfolds.last().map_or(0, |u| u.side_out * u.side_out);
        let width = layout.entry("conv2.bias").2[0];
        let first_p = (spec.channels + 1).div_ceil(8) * 8;
        Ok(Self {
            first_p,
            width,
            last_pix,
            dtype,
            spec,
            layout,
            unfolds,
        })
    }

    fn dense_layer(&self, flat: &Tensor, name: &str, spread: Option<(usize, usize)>) -> Result<(Tensor, Tensor)> {
        let dt = self.dtype;
        let p = flat.dim(0)?;
        let dev = flat.device();
        let (_, off, shape) = self.layout.entry(&format!("{name}.weight"));
        let (o, i) = (shape[0], shape[1]);
        let raw = flat.narrow(1, *off, o * i)?.to_dtype(dt)?.reshape((p, o, i))?;
        let w = match spread {
            None => raw.transpose(1, 2)?.contiguous()?,
            Some((width, pix)) => {
                let conv = raw
                    .narrow(2, 0, width * pix)?
                    .reshape((p, o, width, pix))?
                    .permute((0, 3, 2, 1))?;
                let pad = Tensor::zeros((p, pix, CHANNELS_P - width, o), dt, dev)?;
                let conv = Tensor::cat(&[&conv, &pad], 2)?.reshape((p, pix * CHANNELS_P, o))?;
                let rest = raw.narrow(2, width * pix, i - width * pix)?.transpose(1, 2)?;
                Tensor::cat(&[&conv, &rest], 1)?.contiguous()?
            }
        };
        let (_, off, _) = self.layout.entry(&format!("{name}.bias"));
        let b = flat.narrow(1, *off, o)?.reshape((p, 1, o))?.to_dtype(dt)?;
        Ok((w, b))
    }

    fn conv_layer(&self, flat: &Tensor, name: &str, cin_p: usize) -> Result<Tensor> {
        let dt = self.dtype;
        let p = flat.dim(0)?;
        let dev = flat.device();
        let (_, off, shape) = self.layout.entry(&format!("{name}.weight"));
        let (o, cin) = (shape[0], shape[1]);
        let raw = flat.narrow(1, *off, o * cin * 9)?.to_dtype(dt)?;
        let main = raw.reshape((p, o, cin, 9))?.permute((0, 3, 2, 1))?;
        let out_pad = Tensor::zeros((p, 9, cin, CHANNELS_P - o), dt, dev)?;
        let main = Tensor::cat(&[&main, &out_pad], 3)?;
        let (_, boff, _) = self.layout.entry(&format!("{name}.bias"));
        let bias = flat.narrow(1, *boff, o)?.to_dtype(dt)?.reshape((p, 1, 1, o))?;
        let one = Tensor::ones((p, 1, 1, 1), dt, dev)?;
        let tail = Tensor::zeros((p, 1, 1, CHANNELS_P - o - 1), dt, dev)?;
        let center = Tensor::cat(&[&bias, &one, &tail], 3)?;
        let before = Tensor::zeros((p, 4, 1, CHANNELS_P), dt, dev)?;
        let ones_row = Tensor::cat(&[&before, &center, &before], 1)?;
        let spare = Tensor::zeros((p, 9, cin_p - cin - 1, CHANNELS_P), dt, dev)?;
        Tensor::cat(&[&main, &ones_row, &spare], 2)?
            .reshape((p, 9 * cin_p, CHANNELS_P))?
            .contiguous()
    }

    pub fn weights(&self, flat: &Tensor) -> Result<Weights> {
        let mut convs = vec![];
        let mut cin_p = self.first_p;
        for k in 0..3 {
            convs.push(self.conv_layer(flat, &format!("conv{k}"), cin_p)?);
            cin_p = CHANNELS_P;
        }
        let spread = Some((self.width, self.last_pix));
        Ok(Weights {
            convs,
            a1: self.dense_layer(flat, "a1", spread)?,
            a2: self.dense_layer(flat, "a2", None)?,
            c1: self.dense_layer(flat, "c1", spread)?,
            c2: self.dense_layer(flat, "c2", None)?,
        })
    }

    pub fn prepare(&self, grid: &Tensor) -> Result<Tensor> {
        let dt = self.dtype;
        let rows = grid.dim(0)?;
        let dev = grid.device();
        let pix = self.spec.side * self.spec.side;
        let x = grid
            .to_dtype(dt)?
            .reshape((rows, self.spec.channels, pix))?
            .transpose(1, 2)?;
        let one = Tensor::ones((rows, pix, 1), dt, dev)?;
        let spare = Tensor::zeros((rows, pix, self.first_p - self.spec.channels - 1), dt, dev)?;
        let pad = Tensor::zeros((rows, 1, self.first_p), dt, dev)?;
        Tensor::cat(&[&x, &one, &spare], 2)?
            .pad_with_zeros(1, 0, 1)
            .or_else(|_| Tensor::cat(&[&Tensor::cat(&[&x, &one, &spare], 2)?, &pad], 1))?
            .contiguous()
    }

    pub fn forward(
        &self,
        w: &Weights,
        grid: &Tensor,
        scalars: &Tensor,
    ) -> Result<(Tensor, Tensor)> {
        let (p, n, g) = grid.dims3()?;
        let x0 = self.prepare(&grid.reshape((p * n, g))?)?;
        self.forward_prepared(w, &x0.reshape((p, n, x0.dim(1)?, x0.dim(2)?))?, scalars)
    }

    pub fn forward_prepared(
        &self,
        w: &Weights,
        x0: &Tensor,
        scalars: &Tensor,
    ) -> Result<(Tensor, Tensor)> {
        let dt = self.dtype;
        let (p, n, _, _) = x0.dims4()?;
        let dev = x0.device().clone();
        let mut x = x0.clone();
        let mut first = true;
        for (u, cw) in self.unfolds.iter().zip(&w.convs) {
            let cin = x.dim(3)?;
            let cout = cw.dim(2)?;
            let pix = u.side_out * u.side_out;
            let src = if first {
                x.clone()
            } else {
                let pad = Tensor::zeros((p, n, 1, cin), dt, &dev)?;
                Tensor::cat(&[&x, &pad], 2)?
            };
            first = false;
            let cols = src
                .index_select(&u.index, 2)?
                .reshape((p, n * pix, 9 * cin))?;
            x = cols
                .matmul(cw)?
                .relu()?
                .reshape((p, n, pix, cout))?
                .contiguous()?;
        }
        let h = Tensor::cat(&[&x.flatten_from(2)?, &scalars.to_dtype(dt)?], 2)?;
        let logits = batched(&w.a2, &batched(&w.a1, &h)?.relu()?)?.to_dtype(DType::F32)?;
        let value = batched(&w.c2, &batched(&w.c1, &h)?.relu()?)?
            .to_dtype(DType::F32)?
            .squeeze(D::Minus1)?;
        Ok((logits, value))
    }
}

pub struct Rollout {
    pub obs_g: Tensor,
    pub obs_s: Tensor,
    pub acts: Tensor,
    pub logp: Tensor,
    pub adv: Tensor,
    pub ret: Tensor,
}

pub struct Knobs {
    pub lr: f64,
    pub ent: f32,
    pub pg: f32,
}

pub struct Trainer {
    pub net: PopNet,
    pub dev: Device,
    flat: Var,
    m: Tensor,
    v: Tensor,
    steps: Vec<i32>,
    pop: usize,
}

impl Trainer {
    pub fn new(net: PopNet, rows: Vec<f32>, pop: usize, dev: Device) -> Result<Self> {
        let total = net.layout.total;
        let flat = Var::from_slice(&rows, (pop, total), &dev)?;
        Ok(Self {
            m: Tensor::zeros((pop, total), DType::F32, &dev)?,
            v: Tensor::zeros((pop, total), DType::F32, &dev)?,
            steps: vec![0; pop],
            net,
            dev,
            flat,
            pop,
        })
    }

    pub fn layout(&self) -> &Layout {
        &self.net.layout
    }

    pub fn acting(&self) -> Result<Weights> {
        self.net.weights(&self.flat.as_tensor().detach())
    }

    pub fn rows(&self) -> Result<Vec<f32>> {
        self.flat.as_tensor().flatten_all()?.to_vec1()
    }

    pub fn set_row(&mut self, dst: usize, from: &[f32]) -> Result<()> {
        let total = self.net.layout.total;
        let mut rows = self.rows()?;
        rows[dst * total..(dst + 1) * total].copy_from_slice(from);
        self.flat
            .set(&Tensor::from_vec(rows, (self.pop, total), &self.dev)?)?;
        let mut m: Vec<f32> = self.m.flatten_all()?.to_vec1()?;
        let mut v: Vec<f32> = self.v.flatten_all()?.to_vec1()?;
        m[dst * total..(dst + 1) * total].fill(0.0);
        v[dst * total..(dst + 1) * total].fill(0.0);
        self.m = Tensor::from_vec(m, (self.pop, total), &self.dev)?;
        self.v = Tensor::from_vec(v, (self.pop, total), &self.dev)?;
        self.steps[dst] = 0;
        Ok(())
    }

    fn adam(&mut self, grad: &Tensor, knobs: &[Knobs]) -> Result<()> {
        let (b1, b2) = (0.9f64, 0.999f64);
        for t in self.steps.iter_mut() {
            *t += 1;
        }
        let step: Vec<f32> = knobs
            .iter()
            .zip(&self.steps)
            .map(|(k, t)| (k.lr / (1.0 - b1.powi(*t))) as f32)
            .collect();
        let inv2: Vec<f32> = self
            .steps
            .iter()
            .map(|t| (1.0 / (1.0 - b2.powi(*t))) as f32)
            .collect();
        let step = Tensor::from_vec(step, (self.pop, 1), &self.dev)?;
        let inv2 = Tensor::from_vec(inv2, (self.pop, 1), &self.dev)?;
        self.m = ((&self.m * b1)? + (grad * (1.0 - b1))?)?;
        self.v = ((&self.v * b2)? + (grad.sqr()? * (1.0 - b2))?)?;
        let denom = (self.v.broadcast_mul(&inv2)?.sqrt()? + 1e-5)?;
        let update = self.m.broadcast_mul(&step)?.div(&denom)?;
        self.flat.set(&(self.flat.as_tensor() - update)?)
    }

    pub fn learn(
        &mut self,
        rolls: Vec<Rollout>,
        knobs: &[Knobs],
        minibatch: usize,
        order: &mut dyn FnMut(usize) -> Vec<u32>,
    ) -> Result<Vec<(f32, f32)>> {
        let ns = self.net.spec.scalars;
        let p = self.pop;
        let total = rolls[0].acts.dim(0)?;
        let dev = &self.dev.clone();
        let join = |f: &dyn Fn(&Rollout) -> &Tensor| -> Result<Tensor> {
            Tensor::cat(&rolls.iter().map(f).collect::<Vec<_>>(), 0)
        };
        let bg = self.net.prepare(&join(&|r| &r.obs_g)?)?;
        let full = minibatch >= total;
        let bs = join(&|r| &r.obs_s)?.to_dtype(self.net.dtype)?;
        let ba = join(&|r| &r.acts)?;
        let blp = join(&|r| &r.logp)?;
        let badv = join(&|r| &r.adv)?;
        let bret = join(&|r| &r.ret)?;
        drop(rolls);
        let pg_scale = Tensor::from_vec(knobs.iter().map(|k| k.pg).collect(), p, dev)?;
        let ent_coef = Tensor::from_vec(knobs.iter().map(|k| k.ent).collect(), p, dev)?;
        let mb = minibatch.min(total);
        let mut last: Option<(Tensor, Tensor)> = None;
        for _ in 0..3 {
            let orders: Vec<Vec<u32>> = (0..p).map(|k| order(k)).collect();
            for start in (0..total).step_by(mb) {
                let end = (start + mb).min(total);
                let len = end - start;
                let idx = if full {
                    Tensor::zeros(1, DType::U32, dev)?
                } else {
                    let mut v = Vec::with_capacity(p * len);
                    for (k, o) in orders.iter().enumerate() {
                        v.extend(o[start..end].iter().map(|i| i + (k * total) as u32));
                    }
                    Tensor::from_vec(v, p * len, dev)?
                };
                let pick = |t: &Tensor, shape: &[usize]| -> Result<Tensor> {
                    if full {
                        t.reshape(shape)
                    } else {
                        t.index_select(&idx, 0)?.reshape(shape)
                    }
                };
                let g = {
                    let x = pick(&bg, &[p * len, bg.dim(1)?, bg.dim(2)?])?;
                    x.reshape((p, len, x.dim(1)?, x.dim(2)?))?
                };
                let s = pick(&bs, &[p, len, ns])?;
                let weights = self.net.weights(self.flat.as_tensor())?;
                let (logits, v) = self.net.forward_prepared(&weights, &g, &s)?;
                let lsm = candle_nn::ops::log_softmax(&logits, D::Minus1)?;
                let a_ = pick(&ba, &[p, len, 1])?;
                let new_lp = lsm.gather(&a_, 2)?.squeeze(2)?;
                let old = pick(&blp, &[p, len])?;
                let ratio = (new_lp - old)?.exp()?;
                let ad = pick(&badv, &[p, len])?;
                let s1 = (&ratio * &ad)?;
                let s2 = (ratio.clamp(0.8f32, 1.2f32)? * &ad)?;
                let pg = s1.minimum(&s2)?.mean(1)?.neg()?;
                let ret = pick(&bret, &[p, len])?;
                let vl = (v - ret)?.sqr()?.mean(1)?;
                let ent = (lsm.exp()? * &lsm)?.sum(2)?.mean(1)?.neg()?;
                let loss = (((&pg * &pg_scale)? + (&vl * 0.5)?)? - (&ent * &ent_coef)?)?
                    .sum_all()?;
                let grads = loss.backward()?;
                let g = grads.get(self.flat.as_tensor()).expect("flat gradient").clone();
                self.adam(&g, knobs)?;
                last = Some((ent, vl));
            }
        }
        let (ent, vl) = last.expect("at least one minibatch");
        let ent: Vec<f32> = ent.to_vec1()?;
        let vl: Vec<f32> = vl.to_vec1()?;
        Ok(ent.into_iter().zip(vl).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net;

    fn spec() -> Spec {
        Spec {
            channels: 3,
            side: 8,
            scalars: 4,
            actions: 5,
        }
    }

    fn data(len: usize, salt: u32) -> Vec<f32> {
        (0..len)
            .map(|i| {
                (((i as u32).wrapping_mul(2654435761).wrapping_add(salt.wrapping_mul(40503)) >> 8) % 1000)
                    as f32
                    / 1000.0
                    - 0.3
            })
            .collect()
    }

    fn population(pop: usize) -> (Vec<(VarMap, net::Net)>, PopNet, Tensor) {
        let dev = Device::Cpu;
        let mut members = vec![];
        let mut rows = vec![];
        for k in 0..pop {
            let (vm, n) = net::build(spec(), &dev).unwrap();
            for (i, (_, v)) in vm.data().lock().unwrap().iter().enumerate() {
                let w = v.as_tensor();
                let noise = Tensor::from_vec(data(w.elem_count(), (k * 31 + i) as u32), w.shape(), &dev).unwrap();
                v.set(&(w + (noise * 0.2).unwrap()).unwrap()).unwrap();
            }
            members.push((vm, n));
        }
        let layout = Layout::of(&members[0].0);
        for (vm, _) in &members {
            rows.extend(layout.pack(vm).unwrap());
        }
        let flat = Tensor::from_vec(rows, (pop, layout.total), &dev).unwrap();
        (members, PopNet::new(spec(), layout, DType::F32, &dev).unwrap(), flat)
    }

    fn max_diff(a: Tensor, b: Tensor) -> f32 {
        (a - b).unwrap().abs().unwrap().max_all().unwrap().to_scalar::<f32>().unwrap()
    }

    #[test]
    fn batched_forward_matches_each_member() {
        let (members, pop, flat) = population(3);
        let n = 6;
        let dev = Device::Cpu;
        let g = Tensor::from_vec(data(3 * n * 192, 7), (3, n, 192), &dev).unwrap();
        let s = Tensor::from_vec(data(3 * n * 4, 9), (3, n, 4), &dev).unwrap();
        let w = pop.weights(&flat).unwrap();
        let (logits, value) = pop.forward(&w, &g, &s).unwrap();
        for (k, (_, net)) in members.iter().enumerate() {
            let (l, v) = net.forward(&g.get(k).unwrap(), &s.get(k).unwrap()).unwrap();
            let dl = max_diff(logits.get(k).unwrap(), l);
            let dv = max_diff(value.get(k).unwrap(), v);
            assert!(dl < 1e-4 && dv < 1e-4, "member {k}: logits {dl} value {dv}");
        }
    }

    #[test]
    fn pack_and_unpack_round_trip() {
        let (members, pop, flat) = population(2);
        let (vm, net) = net::build(spec(), &Device::Cpu).unwrap();
        let row: Vec<f32> = flat.get(1).unwrap().to_vec1().unwrap();
        pop.layout.unpack(&row, &vm, &Device::Cpu).unwrap();
        let g = Tensor::from_vec(data(4 * 192, 3), (4, 192), &Device::Cpu).unwrap();
        let s = Tensor::from_vec(data(16, 5), (4, 4), &Device::Cpu).unwrap();
        let d = max_diff(net.forward(&g, &s).unwrap().0, members[1].1.forward(&g, &s).unwrap().0);
        assert!(d < 1e-6, "{d}");
    }

    #[test]
    fn learning_moves_every_member_and_stays_finite() {
        let (_, pop, flat) = population(2);
        let dev = Device::Cpu;
        let before: Vec<f32> = flat.flatten_all().unwrap().to_vec1().unwrap();
        let mut tr = Trainer::new(pop, before.clone(), 2, dev.clone()).unwrap();
        let total = 32;
        let rolls: Vec<Rollout> = (0..2)
            .map(|k| Rollout {
                obs_g: Tensor::from_vec(data(total * 192, k), (total, 192), &dev).unwrap(),
                obs_s: Tensor::from_vec(data(total * 4, k + 1), (total, 4), &dev).unwrap(),
                acts: Tensor::from_vec((0..total as u32).map(|i| i % 5).collect::<Vec<_>>(), total, &dev).unwrap(),
                logp: Tensor::from_vec(vec![-1.6f32; total], total, &dev).unwrap(),
                adv: Tensor::from_vec(data(total, k + 2), total, &dev).unwrap(),
                ret: Tensor::from_vec(data(total, k + 3), total, &dev).unwrap(),
            })
            .collect();
        let knobs = vec![
            Knobs { lr: 1e-3, ent: 0.01, pg: 1.0 },
            Knobs { lr: 1e-3, ent: 0.01, pg: 1.0 },
        ];
        let mut order = |_: usize| (0..total as u32).collect::<Vec<_>>();
        let stats = tr.learn(rolls, &knobs, 16, &mut order).unwrap();
        assert!(stats.iter().all(|(e, v)| e.is_finite() && v.is_finite()));
        let after = tr.rows().unwrap();
        let per = after.len() / 2;
        for k in 0..2 {
            assert!(after[k * per..(k + 1) * per].iter().all(|x| x.is_finite()));
            assert!(after[k * per..(k + 1) * per] != before[k * per..(k + 1) * per]);
        }
    }
}
