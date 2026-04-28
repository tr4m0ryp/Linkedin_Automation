//! D5.A: lognormal inter-send delay sampling.
//!
//! Real human spacing between LinkedIn actions is heavy-tailed -- mostly short
//! pauses, occasional long ones. A lognormal distribution with parameters
//! `(mu, sigma)` matches that shape closely. We pick `mu = ln(median)` so the
//! configured `delay_lognormal_median_secs` is the 50th percentile of the
//! sampled distribution; sigma controls spread (larger sigma -> heavier tail).

use crate::config::HumanizerConfig;
use rand_distr::{Distribution, LogNormal};
use std::time::Duration;

/// Minimum sampled delay in seconds. Anything shorter would not look human.
const MIN_DELAY_SECS: f64 = 60.0;
/// Maximum sampled delay in seconds (4 hours). Caps long-tail draws so a
/// single unlucky sample cannot stall the runner for the rest of the day.
const MAX_DELAY_SECS: f64 = 14_400.0;

/// Lognormal-distributed inter-send delay sampler.
///
/// Sampled durations are clamped to `[MIN_DELAY_SECS, MAX_DELAY_SECS]` so that
/// extreme tail values do not produce nonsensical sleeps.
pub struct LogNormalDelay {
    dist: LogNormal<f64>,
}

impl LogNormalDelay {
    /// Build a delay sampler from a `HumanizerConfig`.
    ///
    /// `mu` is derived as `ln(median)` so that `e^mu == median`. The config
    /// validator in `src/config.rs` enforces `delay_lognormal_median_secs > 0`
    /// and `delay_lognormal_sigma > 0`, so the underlying `LogNormal::new`
    /// call cannot fail at runtime; the `expect` here would only fire on a
    /// programming bug that bypassed the validator.
    pub fn from_config(cfg: &HumanizerConfig) -> Self {
        let mu = cfg.delay_lognormal_median_secs.ln();
        let sigma = cfg.delay_lognormal_sigma;
        let dist = LogNormal::new(mu, sigma).expect("valid lognormal params");
        Self { dist }
    }

    /// Draw a single delay duration, clamped to the configured bounds.
    pub fn sample(&self) -> Duration {
        let mut rng = rand::thread_rng();
        let secs = self
            .dist
            .sample(&mut rng)
            .clamp(MIN_DELAY_SECS, MAX_DELAY_SECS);
        Duration::from_secs(secs as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_cfg() -> HumanizerConfig {
        HumanizerConfig::default()
    }

    #[test]
    fn sample_is_within_clamp_bounds() {
        let delay = LogNormalDelay::from_config(&test_cfg());
        for _ in 0..1024 {
            let d = delay.sample();
            assert!(d >= Duration::from_secs(MIN_DELAY_SECS as u64));
            assert!(d <= Duration::from_secs(MAX_DELAY_SECS as u64));
        }
    }

    #[test]
    fn sample_with_tight_sigma_concentrates_near_median() {
        let cfg = HumanizerConfig {
            delay_lognormal_median_secs: 720.0,
            delay_lognormal_sigma: 0.05,
            ..HumanizerConfig::default()
        };
        let delay = LogNormalDelay::from_config(&cfg);
        let mut sum: u128 = 0;
        let n: u128 = 256;
        for _ in 0..n {
            sum += delay.sample().as_secs() as u128;
        }
        let mean = (sum / n) as f64;
        // With sigma=0.05 the mean is within roughly 10% of the median.
        assert!((mean - 720.0).abs() < 100.0, "mean={} expected ~720", mean);
    }
}
