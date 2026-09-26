//! Linux clock sampling detects sleep and wall-clock changes independently of
//! the monotonic focus timer. It never changes system time or power state.
use std::time::Duration;

#[derive(Clone, Copy, Debug)]
pub struct ClockSample {
    pub monotonic: Duration,
    pub boot: Duration,
    pub wall: Duration,
}

impl ClockSample {
    pub fn read() -> Result<Self, String> {
        fn read_clock(clock: libc::clockid_t) -> Result<Duration, String> {
            let mut sample = libc::timespec {
                tv_sec: 0,
                tv_nsec: 0,
            };
            // SAFETY: sample is valid writable storage for clock_gettime.
            if unsafe { libc::clock_gettime(clock, &mut sample) } != 0
                || sample.tv_sec < 0
                || sample.tv_nsec < 0
            {
                return Err("Lifecycle clock is unavailable".into());
            }
            Ok(Duration::new(sample.tv_sec as u64, sample.tv_nsec as u32))
        }
        Ok(Self {
            monotonic: read_clock(libc::CLOCK_MONOTONIC)?,
            boot: read_clock(libc::CLOCK_BOOTTIME)?,
            wall: read_clock(libc::CLOCK_REALTIME)?,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Interruption {
    Suspend,
    ClockChanged,
}
impl Interruption {
    pub fn description(self) -> &'static str {
        match self {
            Self::Suspend => "the computer suspended",
            Self::ClockChanged => "the system clock changed",
        }
    }
}

pub struct LifecycleWatch(ClockSample);
impl LifecycleWatch {
    pub fn new(sample: ClockSample) -> Self {
        Self(sample)
    }
    pub fn observe(&mut self, current: ClockSample) -> Option<Interruption> {
        let previous = std::mem::replace(&mut self.0, current);
        let monotonic = current.monotonic.saturating_sub(previous.monotonic);
        let boot = current.boot.saturating_sub(previous.boot);
        // Tiny clock sampling/slewing differences must not create false failures.
        let tolerance = Duration::from_millis(250);
        if boot.saturating_sub(monotonic) > tolerance {
            return Some(Interruption::Suspend);
        }
        match current.wall.checked_sub(previous.wall) {
            None => Some(Interruption::ClockChanged),
            Some(wall) if wall.abs_diff(monotonic) > tolerance => Some(Interruption::ClockChanged),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample(monotonic: u64, boot: u64, wall: u64) -> ClockSample {
        ClockSample {
            monotonic: Duration::from_secs(monotonic),
            boot: Duration::from_secs(boot),
            wall: Duration::from_secs(wall),
        }
    }
    #[test]
    fn detects_sleep_forward_backward_changes_without_counting_ordinary_delays() {
        let initial = sample(100, 200, 1000);
        for (next, expected) in [
            (sample(101, 201, 1001), None),
            (sample(160, 260, 1060), None),
            (sample(101, 261, 1061), Some(Interruption::Suspend)),
            (sample(101, 201, 1061), Some(Interruption::ClockChanged)),
            (sample(101, 201, 900), Some(Interruption::ClockChanged)),
        ] {
            assert_eq!(LifecycleWatch::new(initial).observe(next), expected);
        }
    }
    #[test]
    fn reads_real_clocks_without_mutating_them() {
        let first = ClockSample::read().unwrap();
        assert_eq!(
            LifecycleWatch::new(first).observe(ClockSample::read().unwrap()),
            None
        );
    }
}
