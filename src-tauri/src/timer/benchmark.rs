//! Measures how accurately `Sleep(1)` sleeps at each timer resolution, so
//! the recommendation fits this PC instead of assuming 0.5 ms is best.

use serde::Serialize;

/// What the benchmark needs from the OS; faked in tests.
pub trait TimerBackend {
    /// Finest supported resolution, in 100 ns units.
    fn finest(&self) -> Option<u32>;
    /// Requests a resolution for this process; returns what Windows granted.
    fn request(&mut self, units: u32) -> Option<u32>;
    fn release(&mut self, units: u32);
    /// Calls `Sleep(1)` and returns how long it actually took, in ms.
    fn sleep_one_ms(&mut self) -> f64;
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BenchRow {
    /// `None` is the baseline with no request from this process.
    pub requested: Option<u32>,
    pub granted: Option<u32>,
    pub mean_ms: f64,
    pub p99_ms: f64,
    pub max_ms: f64,
    pub samples: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BenchReport {
    pub rows: Vec<BenchRow>,
    pub recommended: Option<u32>,
}

/// Requests to try, coarsest first. 0.504 and 0.507 ms are included because
/// on some systems they oversleep less than an exact 0.5 ms.
pub fn candidates(finest: u32) -> Vec<Option<u32>> {
    let mut out = vec![None];
    for units in [10_000, 7_500, 5_070, 5_040, 5_000] {
        if units >= finest && !out.contains(&Some(units)) {
            out.push(Some(units));
        }
    }
    out
}

pub fn summarize(samples: &[f64]) -> (f64, f64, f64) {
    if samples.is_empty() {
        return (0.0, 0.0, 0.0);
    }
    let mut sorted = samples.to_vec();
    sorted.sort_by(f64::total_cmp);
    let mean = sorted.iter().sum::<f64>() / sorted.len() as f64;
    let p99_index = ((sorted.len() as f64 * 0.99).ceil() as usize)
        .saturating_sub(1)
        .min(sorted.len() - 1);
    (mean, sorted[p99_index], sorted[sorted.len() - 1])
}

/// The request whose 99th-percentile `Sleep(1)` is closest to 1 ms. Within
/// 0.02 ms counts as a tie, and ties go to the coarser value (less power).
pub fn recommend(rows: &[BenchRow]) -> Option<u32> {
    const TIE_MS: f64 = 0.02;
    let requested: Vec<&BenchRow> = rows.iter().filter(|r| r.requested.is_some()).collect();
    let best = requested
        .iter()
        .map(|r| (r.p99_ms - 1.0).abs())
        .reduce(f64::min)?;
    requested
        .iter()
        .filter(|r| (r.p99_ms - 1.0).abs() <= best + TIE_MS)
        .filter_map(|r| r.requested)
        .max()
}

pub fn run(
    backend: &mut dyn TimerBackend,
    samples: usize,
    mut progress: impl FnMut(usize, usize),
) -> BenchReport {
    let finest = backend.finest().unwrap_or(5_000);
    let plan = candidates(finest);
    let mut rows = Vec::new();
    for (i, requested) in plan.iter().enumerate() {
        progress(i, plan.len());
        let granted = requested.and_then(|u| backend.request(u));
        // The baseline sleeps ~15.6 ms each, so it gets fewer samples.
        let n = if requested.is_some() {
            samples
        } else {
            (samples / 8).max(10)
        };
        let measured: Vec<f64> = (0..n).map(|_| backend.sleep_one_ms()).collect();
        if let Some(units) = requested {
            backend.release(*units);
        }
        let (mean_ms, p99_ms, max_ms) = summarize(&measured);
        rows.push(BenchRow {
            requested: *requested,
            granted,
            mean_ms,
            p99_ms,
            max_ms,
            samples: n,
        });
    }
    progress(plan.len(), plan.len());
    BenchReport {
        recommended: recommend(&rows),
        rows,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sleeps for a fixed overshoot that depends on the requested resolution.
    struct Fake {
        current: Option<u32>,
        overshoot: fn(Option<u32>) -> f64,
    }

    impl TimerBackend for Fake {
        fn finest(&self) -> Option<u32> {
            Some(5_000)
        }
        fn request(&mut self, units: u32) -> Option<u32> {
            self.current = Some(units);
            Some(units)
        }
        fn release(&mut self, _: u32) {
            self.current = None;
        }
        fn sleep_one_ms(&mut self) -> f64 {
            1.0 + (self.overshoot)(self.current)
        }
    }

    #[test]
    fn candidates_respect_the_finest_supported() {
        assert_eq!(candidates(5_000).len(), 6);
        assert_eq!(candidates(10_000), vec![None, Some(10_000)]);
    }

    #[test]
    fn p99_and_max() {
        let samples: Vec<f64> = (1..=100).map(f64::from).collect();
        let (mean, p99, max) = summarize(&samples);
        assert_eq!((mean, p99, max), (50.5, 99.0, 100.0));
    }

    #[test]
    fn recommends_the_most_accurate_and_breaks_ties_coarser() {
        let mut fake = Fake {
            current: None,
            overshoot: |r| match r {
                None => 14.6,
                Some(10_000) => 0.95,
                Some(5_000) => 0.49,
                Some(5_040) => 0.48,
                Some(_) => 0.47,
            },
        };
        let mut steps = 0;
        let report = run(&mut fake, 40, |_, _| steps += 1);
        assert_eq!(steps, 7);
        assert_eq!(report.rows.len(), 6);
        assert!(report.rows[0].requested.is_none() && report.rows[0].samples == 10);
        // 0.507 and 0.750 tie within 0.02 ms of the best; the coarser one wins.
        assert_eq!(report.recommended, Some(7_500));
    }
}
