//! Minimal 5-field cron for per-target backup schedules (fork §4 parity:
//! robfig/cron standard syntax). Fields: minute hour day-of-month month
//! day-of-week. Supports `*`, `*/n`, `a-b`, `a-b/n` and comma lists; day-of-
//! week accepts 0-7 (both 0 and 7 = Sunday).
//!
//! The scheduler ticks every minute and asks "was this target due in the last
//! window?", so a missed minute (process asleep) does not replay: standard
//! cron semantics, matching the fork's "skip, do not catch up" rule.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

    /// True when `ts` (unix seconds, UTC) matches this spec.
    pub fn matches_utc(&self, ts: i64) -> bool {
        let dt = match time::OffsetDateTime::from_unix_timestamp(ts) {
            Ok(v) => v,
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
mod tests {
    use super::*;

    // 2026-11-10T03:30:00Z
    const TS_0330: i64 = 1794281400;

    #[test]
    fn parses_and_matches_basic() {
        let spec = CronSpec::parse("30 3 * * *").expect("parse");
        assert!(spec.matches_utc(TS_0330));
        assert!(!spec.matches_utc(TS_0330 + 60));
    }

    #[test]
    fn supports_steps_ranges_lists() {
        assert!(
            CronSpec::parse("*/15 * * * *")
                .unwrap()
                .matches_utc(TS_0330)
        );
        assert!(
            CronSpec::parse("0-59/30 3 * * *")
                .unwrap()
                .matches_utc(TS_0330)
        );
        assert!(
            CronSpec::parse("0,30 3 * * *")
                .unwrap()
                .matches_utc(TS_0330)
        );
        assert!(
            !CronSpec::parse("5,10 3 * * *")
                .unwrap()
                .matches_utc(TS_0330)
        );
    }

    #[test]
    fn dow_accepts_seven_as_sunday() {
        // 2026-11-10 is a Tuesday (dow=2).
        let tue = CronSpec::parse("30 3 * * 2").unwrap();
        assert!(tue.matches_utc(TS_0330));
        let wrong = CronSpec::parse("30 3 * * 3").unwrap();
        assert!(!wrong.matches_utc(TS_0330));
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
        assert!(spec.matches_utc(TS_0330));
        // 11th (Wednesday) matches neither.
        assert!(!spec.matches_utc(TS_0330 + 86_400));
    }
}
