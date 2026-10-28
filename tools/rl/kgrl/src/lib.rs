use kg_core::brain::conv::Policy;
use kg_core::brain::Effort;
use kg_core::games::paper::rl::{self as paper, Arena};
use numpy::{PyArray1, PyArray2, PyArrayMethods, PyReadonlyArray1};
use pyo3::prelude::*;
use std::sync::Mutex;

fn workers() -> usize {
    std::env::var("KGRL_THREADS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map_or(8, |n| n.get())
                .min(32)
        })
}

#[pyclass]
struct PaperVec {
    arenas: Vec<Mutex<Arena>>,
}

#[pymethods]
impl PaperVec {
    #[new]
    fn new(envs: usize, seed: u64, skill: f32, secs: f32) -> Self {
        let arenas = (0..envs)
            .map(|i| {
                Mutex::new(Arena::new(
                    seed.wrapping_mul(1_000_003) + i as u64,
                    skill,
                    secs,
                ))
            })
            .collect();
        Self { arenas }
    }

    #[getter]
    fn agents(&self) -> usize {
        self.arenas.len() * paper::BOTS
    }

    #[staticmethod]
    fn spec() -> (usize, usize, usize, usize) {
        (paper::CHANNELS, paper::VIEW, paper::SCALARS, paper::ACTIONS)
    }

    fn observe<'py>(
        &self,
        py: Python<'py>,
    ) -> (Bound<'py, PyArray2<u8>>, Bound<'py, PyArray2<f32>>) {
        let g = paper::grid_len();
        let n = self.agents();
        let per = paper::BOTS;
        let mut grid = vec![0u8; n * g];
        let mut sc = vec![0.0f32; n * paper::SCALARS];
        py.detach(|| {
            let threads = workers();
            let chunk = self.arenas.len().div_ceil(threads).max(1);
            std::thread::scope(|s| {
                for ((group, gs), ss) in self
                    .arenas
                    .chunks(chunk)
                    .zip(grid.chunks_mut(chunk * per * g))
                    .zip(sc.chunks_mut(chunk * per * paper::SCALARS))
                {
                    s.spawn(move || {
                        for (j, a) in group.iter().enumerate() {
                            let a = a.lock().unwrap();
                            a.write_obs(
                                &mut gs[j * per * g..(j + 1) * per * g],
                                &mut ss[j * per * paper::SCALARS..(j + 1) * per * paper::SCALARS],
                            );
                        }
                    });
                }
            });
        });
        (
            PyArray1::from_vec(py, grid).reshape([n, g]).unwrap(),
            PyArray1::from_vec(py, sc)
                .reshape([n, paper::SCALARS])
                .unwrap(),
        )
    }

    fn step<'py>(
        &self,
        py: Python<'py>,
        actions: PyReadonlyArray1<'py, i64>,
    ) -> (Bound<'py, PyArray1<f32>>, Bound<'py, PyArray1<bool>>) {
        let acts: Vec<usize> = actions
            .as_slice()
            .unwrap()
            .iter()
            .map(|a| *a as usize)
            .collect();
        let n = self.agents();
        let threads = workers();
        let chunk = self.arenas.len().div_ceil(threads).max(1);
        let results: Vec<(usize, Vec<f32>, Vec<bool>)> = py.detach(|| {
            std::thread::scope(|s| {
                let handles: Vec<_> = self
                    .arenas
                    .chunks(chunk)
                    .enumerate()
                    .map(|(c, group)| {
                        let acts = &acts;
                        s.spawn(move || {
                            group
                                .iter()
                                .enumerate()
                                .map(|(j, a)| {
                                    let e = c * chunk + j;
                                    let mut a = a.lock().unwrap();
                                    let mine = &acts[e * paper::BOTS..(e + 1) * paper::BOTS];
                                    let st = a.step(mine);
                                    (e, st.reward, st.done)
                                })
                                .collect::<Vec<_>>()
                        })
                    })
                    .collect();
                handles
                    .into_iter()
                    .flat_map(|h| h.join().unwrap())
                    .collect()
            })
        });
        let mut rew = vec![0.0f32; n];
        let mut done = vec![false; n];
        for (e, r, d) in results {
            rew[e * paper::BOTS..(e + 1) * paper::BOTS].copy_from_slice(&r);
            done[e * paper::BOTS..(e + 1) * paper::BOTS].copy_from_slice(&d);
        }
        (PyArray1::from_vec(py, rew), PyArray1::from_vec(py, done))
    }

    fn circling(&self) -> f32 {
        let (mut home, mut circ) = (0.0, 0.0);
        for a in &self.arenas {
            let mut a = a.lock().unwrap();
            home += a.home_secs;
            circ += a.circling_secs;
            a.home_secs = 0.0;
            a.circling_secs = 0.0;
        }
        circ / f32::max(home, 1e-3)
    }

    fn finished(&self) -> (Vec<f32>, Vec<f32>) {
        let mut ret = vec![];
        let mut peaks = vec![];
        for a in &self.arenas {
            let mut a = a.lock().unwrap();
            ret.append(&mut a.finished);
            peaks.append(&mut a.peaks);
        }
        (ret, peaks)
    }
}

#[pyfunction]
fn paper_logits(kgb: &[u8], grid: Vec<f32>, scalars: Vec<f32>) -> PyResult<Vec<f32>> {
    let p = Policy::from_bytes(kgb)
        .ok_or_else(|| pyo3::exceptions::PyValueError::new_err("bad kgb"))?;
    Ok(p.logits(&kg_core::brain::conv::Obs { grid, scalars }))
}

#[pyfunction]
fn paper_eval(
    kgb: &[u8],
    seeds: u64,
    secs: f32,
    effort: f32,
    opponents: &str,
) -> PyResult<(f32, f32, f32, f32, f32)> {
    let p = Policy::from_bytes(kgb)
        .ok_or_else(|| pyo3::exceptions::PyValueError::new_err("bad kgb"))?;
    let r = paper::evaluate(&p, seeds, secs, Effort::new(effort), opponents == "policy");
    Ok((r.peak, r.deaths, r.kills, r.inside, r.circling))
}

#[pyfunction]
fn paper_baseline(seeds: u64, secs: f32) -> (f32, f32, f32, f32) {
    paper::baseline(seeds, secs)
}

#[pymodule]
fn kgrl(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PaperVec>()?;
    m.add_function(wrap_pyfunction!(paper_logits, m)?)?;
    m.add_function(wrap_pyfunction!(paper_eval, m)?)?;
    m.add_function(wrap_pyfunction!(paper_baseline, m)?)?;
    Ok(())
}
