use candle_core::{DType, Device, Result, Tensor};

pub fn best() -> &'static str {
    if cfg!(feature = "cuda") {
        "cuda"
    } else if cfg!(feature = "metal") {
        "metal"
    } else {
        "cpu"
    }
}

pub fn open(name: &str, own_stream: bool) -> Result<Device> {
    match name {
        "cpu" => Ok(Device::Cpu),
        #[cfg(feature = "cuda")]
        "cuda" => {
            if own_stream {
                let dev = Device::new_cuda_with_stream(0)?;
                if let Device::Cuda(c) = &dev {
                    unsafe { c.cuda_stream().context().disable_event_tracking() };
                }
                Ok(dev)
            } else {
                Device::new_cuda(0)
            }
        }
        #[cfg(feature = "metal")]
        "metal" => {
            let _ = own_stream;
            Device::new_metal(0)
        }
        other => candle_core::bail!("device {other} is not compiled in (built with {})", compiled().join(", ")),
    }
}

pub fn compiled() -> Vec<&'static str> {
    let mut v = vec!["cpu"];
    if cfg!(feature = "cuda") {
        v.push("cuda");
    }
    if cfg!(feature = "metal") {
        v.push("metal");
    }
    v
}

pub fn setup() {
    #[cfg(feature = "cuda")]
    candle_core::cuda::set_gemm_reduced_precision_f32(std::env::var("KG_TF32").is_ok());
}

pub fn bf16_works(name: &str) -> bool {
    let probe = || -> Result<f32> {
        let dev = open(name, false)?;
        let a = Tensor::new(&[[1.0f32, 2.0], [3.0, 4.0]], &dev)?.to_dtype(DType::BF16)?;
        let b = a.matmul(&a)?.relu()?.sum_all()?.to_dtype(DType::F32)?;
        b.to_scalar::<f32>()
    };
    matches!(probe(), Ok(v) if (v - 54.0).abs() < 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_is_always_available() {
        assert!(open("cpu", false).is_ok());
        assert!(compiled().contains(&"cpu"));
    }

    #[test]
    fn unknown_device_is_rejected() {
        assert!(open("tpu", false).is_err());
    }

    #[test]
    fn best_device_is_compiled_in() {
        assert!(compiled().contains(&best()));
    }

    #[test]
    fn bf16_probe_matches_f32_on_cpu() {
        let _ = bf16_works("cpu");
    }
}
