//! Minimal 5-field cron for per-target backup schedules (fork §4 parity:
//! robfig/cron standard syntax). Fields: minute hour day-of-month month
//! day-of-week. Supports `*`, `*/n`, `a-b`, `a-b/n` and comma lists; day-of-
//! week accepts 0-7 (both 0 and 7 = Sunday).
//!
//! Matching happens in the process timezone (`TZ` environment variable,
//! decoded by the images' tzdata; containers default to UTC), so an operator
//! writing `0 2 * * *` means 02:00 at the deployment's wall clock, not UTC.
//!
//! The scheduler ticks every minute and asks "was this target due in the last
//! window?", so a missed minute (process asleep) does not replay: standard
//! cron semantics, matching the fork's "skip, do not catch up" rule.

use serde::{Deserialize, Serialize};
use time::UtcOffset;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Parsed 5-field cron spec (minute hour dom month dow).
pub struct CronSpec {
    minutes: Vec<u32>,
    hours: Vec<u32>,
    days: Vec<u32>,
    months: Vec<u32>,
    weekdays: Vec<u32>,
    /// Day-of-month and day-of-week both restricted: standard cron ORs them.
    dom_restricted: bool,
    dow_restricted: bool,
}

impl CronSpec {
    /// Parse a 5-field cron expression (minute hour day-of-month month day-of-week).
    /// Day-of-week accepts 0-7 (7 folds to Sunday).
    ///
    /// # Examples
    ///
    /// ```
    /// use pergame_rs::target::cron::CronSpec;
    /// use time::UtcOffset;
    ///
    /// let spec = CronSpec::parse("30 3 * * *").unwrap();
    /// // 2026-11-10T03:30:00Z matches; one minute later does not.
    /// assert!(spec.matches_in(1794281400, UtcOffset::UTC));
    /// assert!(!spec.matches_in(1794281400 + 60, UtcOffset::UTC));
    /// assert!(CronSpec::parse("60 * * * *").is_err());
    /// ```
    pub fn parse(expr: &str) -> Result<Self, String> {
        let fields: Vec<&str> = expr.split_whitespace().collect();
        if fields.len() != 5 {
            return Err(format!(
                "cron needs 5 fields (minute hour dom month dow), got {}",
                fields.len()
            ));
        }
        let minutes = parse_field(fields[0], 0, 59)?;
        let hours = parse_field(fields[1], 0, 23)?;
        let days = parse_field(fields[2], 1, 31)?;
        let months = parse_field(fields[3], 1, 12)?;
        let weekdays = parse_field(fields[4], 0, 7)?
            .into_iter()
            .map(|d| d % 7) // 7 == Sunday == 0
            .collect::<Vec<_>>();
        Ok(Self {
            minutes,
            hours,
            days,
            months,
            weekdays: dedup(weekdays),
            dom_restricted: fields[2] != "*",
            dow_restricted: fields[4] != "*",
        })
    }

    /// True when `ts` (unix seconds) matches this spec in the process
    /// timezone (`TZ` env var; UTC when unset, which is the container
    /// default).
    pub fn matches(&self, ts: i64) -> bool {
        self.matches_in(ts, local_offset())
    }

    /// True when `ts` matches this spec at the given fixed offset.
    /// The offset form is the testable core; `matches` feeds it the process
    /// timezone. DST-observing zones degrade to their current fixed offset
    /// (good enough for minute/hour schedules across the daylight switch).
    pub fn matches_in(&self, ts: i64, offset: UtcOffset) -> bool {
        let dt = match time::OffsetDateTime::from_unix_timestamp(ts) {
            Ok(v) => v.to_offset(offset),
            Err(_) => return false,
        };
        let minute = dt.minute() as u32;
        let hour = dt.hour() as u32;
        let dom = dt.day() as u32;
        let month = u8::from(dt.month()) as u32;
        let dow = dt.weekday().number_days_from_sunday() as u32;

        if !self.minutes.contains(&minute)
            || !self.hours.contains(&hour)
            || !self.months.contains(&month)
        {
            return false;
        }
        let dom_ok = self.days.contains(&dom);
        let dow_ok = self.weekdays.contains(&dow);
        match (self.dom_restricted, self.dow_restricted) {
            // Both restricted => OR (cron convention).
            (true, true) => dom_ok || dow_ok,
            (true, false) => dom_ok,
            (false, true) => dow_ok,
            (false, false) => true,
        }
    }
}

/// The process timezone as a fixed offset, read through chrono's `Local`
/// (which decodes the `TZ` environment variable against the tzdata shipped
/// in the image). Falls back to UTC if the offset cannot be determined.
fn local_offset() -> UtcOffset {
    let secs = chrono::Local::now().offset().local_minus_utc();
    UtcOffset::from_whole_seconds(secs).unwrap_or(UtcOffset::UTC)
}

fn dedup(mut v: Vec<u32>) -> Vec<u32> {
    v.sort_unstable();
    v.dedup();
    v
}

fn parse_field(field: &str, min: u32, max: u32) -> Result<Vec<u32>, String> {
    let mut out = Vec::new();
    for part in field.split(',') {
        let (range_part, step) = match part.split_once('/') {
            Some((r, s)) => {
                let step: u32 = s.parse().map_err(|_| format!("bad step in {part:?}"))?;
                if step == 0 {
                    return Err(format!("step must be > 0 in {part:?}"));
                }
                (r, step)
            }
            None => (part, 1),
        };
        let (lo, hi) = if range_part == "*" {
            (min, max)
        } else if let Some((a, b)) = range_part.split_once('-') {
            (
                a.parse::<u32>()
                    .map_err(|_| format!("bad range in {part:?}"))?,
                b.parse::<u32>()
                    .map_err(|_| format!("bad range in {part:?}"))?,
            )
        } else {
            let v: u32 = range_part
                .parse()
                .map_err(|_| format!("bad value in {part:?}"))?;
            (v, v)
        };
        if lo < min || hi > max || lo > hi {
            return Err(format!("out of range [{min},{max}] in {part:?}"));
        }
        let mut v = lo;
        while v <= hi {
            out.push(v);
            v += step;
        }
    }
    if out.is_empty() {
        return Err(format!("empty field {field:?}"));
    }
    Ok(dedup(out))
}

#[cfg(test)]
mod proptests {
    use proptest::prelude::*;
    use time::UtcOffset;

    use super::CronSpec;

    proptest! {
        /// In-range quintuples always parse; the spec then matches exactly at
        /// its own wall-clock minute (UTC) and misses the next minute.
        #[test]
        fn roundtrip_minute_hour(
            (m, h) in (0u32..60, 0u32..24),
        ) {
            let spec = CronSpec::parse(&format!("{m} {h} * * *")).unwrap();
            // 2026-01-01T00:00:00Z + h hours + m minutes.
            let ts = 1767225600 + i64::from(h) * 3600 + i64::from(m) * 60;
            prop_assert!(spec.matches_in(ts, UtcOffset::UTC));
            prop_assert!(!spec.matches_in(ts + 60, UtcOffset::UTC));
        }

        /// No input panics the parser; accepted specs carry sorted,
        /// deduplicated, in-range minute/hour sets.
        #[test]
        fn parse_never_panics(s in "\\PC{0,64}") {
            let expr = s.split_whitespace().collect::<Vec<_>>().join(" ");
            if let Ok(spec) = CronSpec::parse(&expr) {
                let mut minutes = spec.minutes.clone();
                minutes.sort_unstable();
                minutes.dedup();
                prop_assert_eq!(&spec.minutes, &minutes);
                prop_assert!(spec.minutes.iter().all(|v| *v < 60));
                prop_assert!(spec.hours.iter().all(|v| *v < 24));
            }
        }

        /// Sunday folds: dow 7 behaves exactly like dow 0.
        #[test]
        fn seven_is_sunday(ts in 1_700_000_000i64..1_800_000_000) {
            let zero = CronSpec::parse("0 0 * * 0").unwrap();
            let seven = CronSpec::parse("0 0 * * 7").unwrap();
            prop_assert_eq!(
                zero.matches_in(ts, UtcOffset::UTC),
                seven.matches_in(ts, UtcOffset::UTC)
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::offset;

    // 2026-11-10T03:30:00Z
    const TS_0330: i64 = 1794281400;

    #[test]
    fn parses_and_matches_basic() {
        let spec = CronSpec::parse("30 3 * * *").expect("parse");
        assert!(spec.matches_in(TS_0330, offset!(UTC)));
        assert!(!spec.matches_in(TS_0330 + 60, offset!(UTC)));
    }

    #[test]
    fn timezone_offset_shifts_the_wall_clock() {
        let spec = CronSpec::parse("30 3 * * *").unwrap();
        // UTC+8: local 03:30 happens at 19:30Z the previous UTC day.
        let ts_local_0330 = TS_0330 - 8 * 3600;
        assert!(
            spec.matches_in(ts_local_0330, offset!(+8)),
            "03:30 wall clock in UTC+8 must match"
        );
        assert!(
            !spec.matches_in(TS_0330, offset!(+8)),
            "UTC 03:30 is 11:30 local in UTC+8 and must not match"
        );
    }

    #[test]
    fn supports_steps_ranges_lists() {
        let utc = offset!(UTC);
        assert!(
            CronSpec::parse("*/15 * * * *")
                .unwrap()
                .matches_in(TS_0330, utc)
        );
        assert!(
            CronSpec::parse("0-59/30 3 * * *")
                .unwrap()
                .matches_in(TS_0330, utc)
        );
        assert!(
            CronSpec::parse("0,30 3 * * *")
                .unwrap()
                .matches_in(TS_0330, utc)
        );
        assert!(
            !CronSpec::parse("5,10 3 * * *")
                .unwrap()
                .matches_in(TS_0330, utc)
        );
    }

    #[test]
    fn dow_accepts_seven_as_sunday() {
        // 2026-11-10 is a Tuesday (dow=2).
        let tue = CronSpec::parse("30 3 * * 2").unwrap();
        assert!(tue.matches_in(TS_0330, offset!(UTC)));
        let wrong = CronSpec::parse("30 3 * * 3").unwrap();
        assert!(!wrong.matches_in(TS_0330, offset!(UTC)));
    }

    #[test]
    fn rejects_bad_input() {
        for bad in [
            "",
            "* * * *",
            "60 * * * *",
            "* 24 * * *",
            "*/0 * * * *",
            "a * * * *",
        ] {
            assert!(CronSpec::parse(bad).is_err(), "{bad:?} should fail");
        }
    }

    #[test]
    fn dom_and_dow_are_ored_when_both_restricted() {
        // 10th of the month OR any Tuesday: 2026-11-10 matches both.
        let spec = CronSpec::parse("30 3 10 * 2").unwrap();
        assert!(spec.matches_in(TS_0330, offset!(UTC)));
        // 11th (Wednesday) matches neither.
        assert!(!spec.matches_in(TS_0330 + 86_400, offset!(UTC)));
    }
}
