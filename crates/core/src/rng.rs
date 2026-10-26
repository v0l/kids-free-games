use std::cell::Cell;

const MULTIPLIER: u64 = 6364136223846793005;
const INC: u64 = 1442695040888963407;

thread_local! {
    static STATE: Cell<u64> = const { Cell::new(0x853c_49e6_748f_ea9b) };
}

fn step(s: u64) -> u64 {
    s.wrapping_mul(MULTIPLIER).wrapping_add(INC)
}

pub fn srand(seed: u64) {
    STATE.with(|s| {
        s.set(step(step(0).wrapping_add(seed)));
    });
}

pub fn rand() -> u32 {
    STATE.with(|s| {
        let old = s.get();
        s.set(step(old));
        let xorshifted = (((old >> 18) ^ old) >> 27) as u32;
        xorshifted.rotate_right((old >> 59) as u32)
    })
}

pub trait RandomRange: Sized {
    fn gen_range(low: Self, high: Self) -> Self;
}

fn unit() -> f64 {
    rand() as f64 / (u32::MAX as f64 + 1.0)
}

macro_rules! float_range {
    ($($t:ty),*) => {$(
        impl RandomRange for $t {
            fn gen_range(low: Self, high: Self) -> Self {
                (low as f64 + (high as f64 - low as f64) * unit()) as $t
            }
        }
    )*};
}

macro_rules! int_range {
    ($($t:ty),*) => {$(
        impl RandomRange for $t {
            fn gen_range(low: Self, high: Self) -> Self {
                if high <= low {
                    return low;
                }
                let span = (high as i128 - low as i128) as f64;
                (low as i128 + (unit() * span) as i128) as $t
            }
        }
    )*};
}

float_range!(f32, f64);
int_range!(u8, u16, u32, u64, usize, i8, i16, i32, i64, isize);

pub fn gen_range<T: RandomRange>(low: T, high: T) -> T {
    T::gen_range(low, high)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_seed_gives_the_same_numbers_even_while_other_threads_draw() {
        let noise = std::thread::spawn(|| {
            for _ in 0..200_000 {
                gen_range(0u32, 100);
            }
        });
        srand(42);
        let a: Vec<u32> = (0..2000).map(|_| gen_range(0u32, 1000)).collect();
        srand(42);
        let b: Vec<u32> = (0..2000).map(|_| gen_range(0u32, 1000)).collect();
        noise.join().unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn ranges_stay_inside_their_bounds() {
        srand(7);
        for _ in 0..10_000 {
            let i = gen_range(-3i32, 4);
            assert!((-3..4).contains(&i));
            let f = gen_range(0.5f32, 1.5);
            assert!((0.5..1.5).contains(&f));
            let u = gen_range(0usize, 1);
            assert_eq!(u, 0);
        }
        assert_eq!(gen_range(5u32, 5), 5);
    }
}
