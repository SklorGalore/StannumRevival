//! A small seeded random number generator (xoshiro256**), so the same seed
//! builds the same case on every platform and with any dependency versions.

#[derive(Clone, Debug)]
pub struct Rng {
    s: [u64; 4],
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        // SplitMix64 spreads the seed over the whole state.
        let mut x = seed;
        let mut next = || {
            x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = x;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^ (z >> 31)
        };
        Self {
            s: [next(), next(), next(), next()],
        }
    }

    /// An independent stream for one stage, so adding draws to one stage
    /// doesn't change what the others build.
    pub fn fork(&mut self, stage: u64) -> Self {
        Self::new(self.next_u64() ^ stage.wrapping_mul(0xD1B5_4A32_D192_ED03))
    }

    pub fn next_u64(&mut self) -> u64 {
        let result = self.s[1].wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        let t = self.s[1] << 17;
        self.s[2] ^= self.s[0];
        self.s[3] ^= self.s[1];
        self.s[1] ^= self.s[2];
        self.s[0] ^= self.s[3];
        self.s[2] ^= t;
        self.s[3] = self.s[3].rotate_left(45);
        result
    }

    /// Uniform in [0, 1).
    pub fn f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    pub fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.f64()
    }

    /// Uniform index in 0..n (n > 0).
    pub fn index(&mut self, n: usize) -> usize {
        ((self.f64() * n as f64) as usize).min(n - 1)
    }

    /// Standard normal (Box-Muller).
    pub fn normal(&mut self) -> f64 {
        let u1 = self.f64().max(f64::MIN_POSITIVE);
        let u2 = self.f64();
        (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
    }

    /// Log-normal with the given median and log-space spread.
    pub fn lognormal(&mut self, median: f64, sigma: f64) -> f64 {
        median * (sigma * self.normal()).exp()
    }

    /// Shuffle in place (Fisher-Yates).
    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            let j = self.index(i + 1);
            items.swap(i, j);
        }
    }

    /// `k` distinct indices drawn with probability proportional to weight,
    /// without replacement (Efraimidis-Spirakis). Zero weights come last.
    pub fn weighted_sample(&mut self, weights: &[f64], k: usize) -> Vec<usize> {
        let mut keys: Vec<(f64, usize)> = weights
            .iter()
            .enumerate()
            .map(|(i, &w)| {
                let u = self.f64().max(f64::MIN_POSITIVE);
                let key = if w > 0.0 {
                    u.ln() / w
                } else {
                    f64::NEG_INFINITY
                };
                (key, i)
            })
            .collect();
        keys.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
        keys.into_iter().take(k).map(|(_, i)| i).collect()
    }
}

/// Sampling from fixed weights in O(log n) per draw.
pub struct Weighted {
    cumulative: Vec<f64>,
}

impl Weighted {
    pub fn new(weights: &[f64]) -> Self {
        let mut total = 0.0;
        let cumulative = weights
            .iter()
            .map(|w| {
                total += w.max(0.0);
                total
            })
            .collect();
        Self { cumulative }
    }

    pub fn sample(&self, rng: &mut Rng) -> usize {
        let total = *self.cumulative.last().unwrap_or(&0.0);
        let x = rng.f64() * total;
        self.cumulative
            .partition_point(|&c| c <= x)
            .min(self.cumulative.len() - 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_stream() {
        let a: Vec<u64> = (0..5)
            .map({
                let mut r = Rng::new(7);
                move |_| r.next_u64()
            })
            .collect();
        let b: Vec<u64> = (0..5)
            .map({
                let mut r = Rng::new(7);
                move |_| r.next_u64()
            })
            .collect();
        assert_eq!(a, b);
        assert_ne!(a, {
            let mut r = Rng::new(8);
            (0..5).map(|_| r.next_u64()).collect::<Vec<_>>()
        });
    }

    #[test]
    fn uniform_and_normal_look_right() {
        let mut r = Rng::new(1);
        let n = 20_000;
        let mean = (0..n).map(|_| r.f64()).sum::<f64>() / n as f64;
        assert!((mean - 0.5).abs() < 0.02);
        let xs: Vec<f64> = (0..n).map(|_| r.normal()).collect();
        let m = xs.iter().sum::<f64>() / n as f64;
        let v = xs.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / n as f64;
        assert!(m.abs() < 0.05 && (v - 1.0).abs() < 0.05);
    }
}
