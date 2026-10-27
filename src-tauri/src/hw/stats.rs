//! Live stats, sampled once a second only while a view is watching.
//!
//! The sampler thread exists only while at least one subscriber is active, so
//! Apex Tune costs nothing in the background while you play.

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};

const INTERVAL: Duration = Duration::from_secs(1);
/// Listing every process is the most expensive part, so it runs less often.
const PROCESS_EVERY: u32 = 3;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsTick {
    /// Milliseconds since the Unix epoch.
    pub at: i64,
    pub cpu_percent: f32,
    pub ram_used_bytes: u64,
    pub ram_total_bytes: u64,
    pub processes: usize,
    /// 3D engine load of the busiest GPU; `None` if Windows doesn't report it.
    pub gpu_percent: Option<f32>,
    /// From NVAPI on NVIDIA GPUs.
    pub gpu_temp_c: Option<i32>,
}

type Sink = Arc<dyn Fn(StatsTick) + Send + Sync>;

#[derive(Default)]
pub struct Sampler {
    subscribers: Arc<AtomicUsize>,
    /// Bumped for each new thread, so a thread still finishing its last tick
    /// after an unsubscribe/subscribe pair exits instead of running twice.
    generation: Arc<AtomicUsize>,
    start: Mutex<()>,
}

impl Sampler {
    /// Adds a subscriber, starting the sampler thread if it isn't running.
    pub fn subscribe(&self, sink: Sink) {
        let _guard = self.start.lock().unwrap_or_else(|p| p.into_inner());
        if self.subscribers.fetch_add(1, Ordering::SeqCst) > 0 {
            return;
        }
        let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let (subscribers, current) = (self.subscribers.clone(), self.generation.clone());
        std::thread::spawn(move || {
            run(
                &|| {
                    subscribers.load(Ordering::SeqCst) > 0
                        && current.load(Ordering::SeqCst) == generation
                },
                &sink,
            )
        });
    }

    /// Removes a subscriber; the thread stops after its current tick when none remain.
    pub fn unsubscribe(&self) {
        let _ = self
            .subscribers
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1));
    }

    pub fn active(&self) -> bool {
        self.subscribers.load(Ordering::SeqCst) > 0
    }
}

#[cfg(windows)]
fn temperature() -> Option<i32> {
    use crate::sys::Nvidia;
    crate::sys::win::nvapi::WinNvidia.gpu_temperature()
}

#[cfg(not(windows))]
fn temperature() -> Option<i32> {
    None
}

fn run(keep_going: &dyn Fn() -> bool, sink: &Sink) {
    let mut system = System::new();
    let mut gpu = GpuCounter::new();
    // CPU usage is a delta, so take a baseline first.
    system.refresh_cpu_usage();
    let mut processes = 0;
    let mut tick = 0u32;

    while keep_going() {
        let started = Instant::now();
        std::thread::sleep(INTERVAL);
        if !keep_going() {
            break;
        }
        system.refresh_cpu_usage();
        system.refresh_memory();
        if tick.is_multiple_of(PROCESS_EVERY) {
            system.refresh_processes_specifics(
                ProcessesToUpdate::All,
                true,
                ProcessRefreshKind::nothing(),
            );
            processes = system.processes().len();
        }
        tick = tick.wrapping_add(1);

        sink(StatsTick {
            at: chrono::Utc::now().timestamp_millis(),
            cpu_percent: system.global_cpu_usage(),
            ram_used_bytes: system.used_memory(),
            ram_total_bytes: system.total_memory(),
            processes,
            gpu_percent: gpu.as_mut().and_then(GpuCounter::sample),
            gpu_temp_c: temperature(),
        });
        tracing::trace!("stats tick took {:?}", started.elapsed());
    }
}

/// Sums 3D-engine utilisation per adapter (instance names contain
/// `luid_0x…_0x…`) and returns the busiest adapter, capped at 100.
pub fn busiest_adapter<'a>(samples: impl IntoIterator<Item = (&'a str, f64)>) -> Option<f32> {
    let mut per_adapter: HashMap<&str, f64> = HashMap::new();
    for (instance, value) in samples {
        let Some(start) = instance.find("luid_") else {
            continue;
        };
        let rest = &instance[start..];
        let end = rest.find("_phys").unwrap_or(rest.len());
        *per_adapter.entry(&rest[..end]).or_default() += value.max(0.0);
    }
    per_adapter
        .values()
        .copied()
        .reduce(f64::max)
        .map(|v| v.min(100.0) as f32)
}

#[cfg(windows)]
use pdh::GpuCounter;

#[cfg(not(windows))]
struct GpuCounter;

#[cfg(not(windows))]
impl GpuCounter {
    fn new() -> Option<Self> {
        None
    }
    fn sample(&mut self) -> Option<f32> {
        None
    }
}

#[cfg(windows)]
mod pdh {
    use windows::Win32::System::Performance::{
        PDH_FMT_COUNTERVALUE_ITEM_W, PDH_FMT_DOUBLE, PDH_HCOUNTER, PDH_HQUERY,
        PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData, PdhGetFormattedCounterArrayW,
        PdhOpenQueryW,
    };
    use windows::core::{HSTRING, PCWSTR};

    const PDH_MORE_DATA: u32 = 0x8000_07D2;
    const COUNTER: &str = r"\GPU Engine(*engtype_3D)\Utilization Percentage";

    /// The "GPU Engine" performance counter Task Manager uses. Vendor-neutral.
    pub struct GpuCounter {
        query: PDH_HQUERY,
        counter: PDH_HCOUNTER,
    }

    impl GpuCounter {
        pub fn new() -> Option<Self> {
            let mut query = PDH_HQUERY::default();
            // SAFETY: null data source means live data; `query` is a valid out-pointer.
            if unsafe { PdhOpenQueryW(PCWSTR::null(), 0, &mut query) } != 0 {
                return None;
            }
            let mut counter = PDH_HCOUNTER::default();
            // SAFETY: `query` is open; the path is a valid wide string.
            if unsafe { PdhAddEnglishCounterW(query, &HSTRING::from(COUNTER), 0, &mut counter) }
                != 0
            {
                // SAFETY: closing the query opened above.
                unsafe { PdhCloseQuery(query) };
                return None;
            }
            // SAFETY: first collection primes the rate counter.
            unsafe { PdhCollectQueryData(query) };
            Some(Self { query, counter })
        }

        pub fn sample(&mut self) -> Option<f32> {
            // SAFETY: the query is open for the lifetime of self.
            if unsafe { PdhCollectQueryData(self.query) } != 0 {
                return None;
            }
            let mut bytes = 0u32;
            let mut count = 0u32;
            // SAFETY: size query with no buffer.
            let status = unsafe {
                PdhGetFormattedCounterArrayW(
                    self.counter,
                    PDH_FMT_DOUBLE,
                    &mut bytes,
                    &mut count,
                    None,
                )
            };
            if status != PDH_MORE_DATA || bytes == 0 {
                // No GPU engine instances at all means nothing is using the GPU.
                return (status == 0).then_some(0.0);
            }
            let items = (bytes as usize).div_ceil(size_of::<PDH_FMT_COUNTERVALUE_ITEM_W>());
            let mut buf = vec![PDH_FMT_COUNTERVALUE_ITEM_W::default(); items];
            // SAFETY: the buffer holds at least `bytes` bytes, correctly aligned.
            let status = unsafe {
                PdhGetFormattedCounterArrayW(
                    self.counter,
                    PDH_FMT_DOUBLE,
                    &mut bytes,
                    &mut count,
                    Some(buf.as_mut_ptr()),
                )
            };
            if status != 0 {
                return None;
            }
            let samples: Vec<(String, f64)> = buf[..count as usize]
                .iter()
                .filter(|item| item.FmtValue.CStatus == 0)
                .map(|item| {
                    // SAFETY: PDH fills szName with a NUL-terminated string inside `buf`.
                    let name = unsafe { item.szName.to_string() }.unwrap_or_default();
                    // SAFETY: doubleValue is the active member for PDH_FMT_DOUBLE.
                    let value = unsafe { item.FmtValue.Anonymous.doubleValue };
                    (name, value)
                })
                .collect();
            super::busiest_adapter(samples.iter().map(|(n, v)| (n.as_str(), *v)))
        }
    }

    impl Drop for GpuCounter {
        fn drop(&mut self) {
            // SAFETY: the query was opened in `new` and is closed once.
            unsafe { PdhCloseQuery(self.query) };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU32;

    #[test]
    fn sums_per_adapter_and_takes_the_busiest() {
        let samples = [
            (
                "pid_100_luid_0x00000000_0x0000D1A2_phys_0_eng_0_engtype_3D",
                30.0,
            ),
            (
                "pid_200_luid_0x00000000_0x0000D1A2_phys_0_eng_0_engtype_3D",
                25.5,
            ),
            (
                "pid_300_luid_0x00000000_0x0000E000_phys_0_eng_0_engtype_3D",
                40.0,
            ),
            (
                "pid_400_luid_0x00000000_0x0000E000_phys_0_eng_0_engtype_3D",
                -1.0,
            ),
        ];
        assert_eq!(busiest_adapter(samples), Some(55.5));
        assert_eq!(
            busiest_adapter([("luid_a_phys_0", 80.0), ("luid_a_phys_0", 50.0)]),
            Some(100.0)
        );
        assert_eq!(busiest_adapter(std::iter::empty()), None);
    }

    #[test]
    fn sampler_runs_only_while_subscribed() {
        let sampler = Sampler::default();
        let ticks = Arc::new(AtomicU32::new(0));
        let counter = ticks.clone();
        let sink: Sink = Arc::new(move |t: StatsTick| {
            assert!(t.ram_total_bytes > 0);
            counter.fetch_add(1, Ordering::SeqCst);
        });
        sampler.subscribe(sink.clone());
        sampler.subscribe(sink);
        sampler.unsubscribe();
        assert!(sampler.active(), "one subscriber left");
        std::thread::sleep(Duration::from_millis(2300));
        sampler.unsubscribe();
        assert!(!sampler.active());
        std::thread::sleep(Duration::from_millis(1200));
        let after_stop = ticks.load(Ordering::SeqCst);
        assert!(after_stop >= 1);
        std::thread::sleep(Duration::from_millis(1200));
        assert_eq!(
            ticks.load(Ordering::SeqCst),
            after_stop,
            "no ticks after the last unsubscribe"
        );
        sampler.unsubscribe(); // extra unsubscribe is harmless
    }
}
