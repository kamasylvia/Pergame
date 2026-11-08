//! Snowflake identifiers and generator.
//!
//! JSON wire format mirrors the Go backend (`types.Snowflake`): ids are
//! *serialized as strings*; deserialization accepts both strings and numbers.
//! The generator mirrors `backend/pkg/snowflake` bit layout:
//! `(ms-epoch)<<22 | worker(5) | process(5) | increment(12)`.

use std::sync::Mutex;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub const SNOWFLAKE_EPOCH_MS: i64 = 1_609_459_200_000; // 2021-01-01T00:00:00Z

const WORKER_ID_BITS: u32 = 5;
const PROCESS_ID_BITS: u32 = 5;
const INCREMENT_BITS: u32 = 12;
const TIMESTAMP_SHIFT: u32 = WORKER_ID_BITS + PROCESS_ID_BITS + INCREMENT_BITS;
const WORKER_ID_SHIFT: u32 = PROCESS_ID_BITS + INCREMENT_BITS;
const PROCESS_ID_SHIFT: u32 = INCREMENT_BITS;
const MAX_INCREMENT: u64 = (1 << INCREMENT_BITS) - 1;
const MAX_WORKER_ID: u64 = (1 << WORKER_ID_BITS) - 1;
const MAX_PROCESS_ID: u64 = (1 << PROCESS_ID_BITS) - 1;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Snowflake(pub u64);

impl Snowflake {
    pub fn from_u64(v: u64) -> Self {
        Self(v)
    }

    pub fn as_i64(self) -> i64 {
        self.0 as i64
    }

    pub fn from_i64(v: i64) -> Self {
        Self(v as u64)
    }

    pub fn parse(s: &str) -> Option<Self> {
        s.parse::<u64>().ok().map(Self)
    }
}

impl std::fmt::Display for Snowflake {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Serialize for Snowflake {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for Snowflake {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct V;
        impl serde::de::Visitor<'_> for V {
            type Value = Snowflake;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("snowflake as string or number")
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Snowflake, E> {
                v.parse::<u64>().map(Snowflake).map_err(E::custom)
            }
            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Snowflake, E> {
                Ok(Snowflake(v))
            }
            fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Snowflake, E> {
                u64::try_from(v).map(Snowflake).map_err(E::custom)
            }
        }
        deserializer.deserialize_any(V)
    }
}

// --- sqlx: stored as PostgreSQL BIGINT ---
impl sqlx::Type<sqlx::Postgres> for Snowflake {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        <i64 as sqlx::Type<sqlx::Postgres>>::type_info()
    }
}

impl<'r> sqlx::Decode<'r, sqlx::Postgres> for Snowflake {
    fn decode(
        value: sqlx::postgres::PgValueRef<'r>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync + 'static>> {
        Ok(Self::from_i64(
            <i64 as sqlx::Decode<sqlx::Postgres>>::decode(value)?,
        ))
    }
}

impl<'q> sqlx::Encode<'q, sqlx::Postgres> for Snowflake {
    fn encode_by_ref(
        &self,
        buf: &mut sqlx::postgres::PgArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
        <i64 as sqlx::Encode<sqlx::Postgres>>::encode(self.as_i64(), buf)
    }
}

impl sqlx::postgres::PgHasArrayType for Snowflake {
    fn array_type_info() -> sqlx::postgres::PgTypeInfo {
        <i64 as sqlx::postgres::PgHasArrayType>::array_type_info()
    }
}

/// Generator matching the Go implementation's layout and wrap behavior.
pub struct SnowflakeGenerator {
    epoch: i64,
    worker_id: u64,
    process_id: u64,
    state: Mutex<GeneratorState>,
}

struct GeneratorState {
    increment: u64,
    last_timestamp: i64,
}

impl SnowflakeGenerator {
    /// `worker_id` defaults to the pid, `process_id` to the ppid (Go parity).
    pub fn new(epoch: i64) -> Self {
        Self::with_ids(
            epoch,
            (std::process::id() as u64) & MAX_WORKER_ID,
            (parent_pid() as u64) & MAX_PROCESS_ID,
        )
    }

    pub fn with_ids(epoch: i64, worker_id: u64, process_id: u64) -> Self {
        Self {
            epoch,
            worker_id,
            process_id,
            state: Mutex::new(GeneratorState {
                increment: 0,
                last_timestamp: 0,
            }),
        }
    }

    pub fn generate(&self) -> Snowflake {
        let mut state = self.state.lock().unwrap();
        // Clock is read under the lock so timestamps never regress in call order.
        let mut timestamp = now_millis();

        state.increment = (state.increment + 1) & MAX_INCREMENT;
        if state.increment == 0 {
            // Wrapped: advance to a later millisecond so recently handed-out
            // ids can never recur (busy-wait matches the Go behavior).
            while timestamp <= state.last_timestamp {
                timestamp = now_millis();
            }
        }
        state.last_timestamp = timestamp;

        let elapsed = (timestamp - self.epoch).max(0) as u64;
        Snowflake(
            (elapsed << TIMESTAMP_SHIFT)
                | (self.worker_id << WORKER_ID_SHIFT)
                | (self.process_id << PROCESS_ID_SHIFT)
                | state.increment,
        )
    }
}

fn now_millis() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn parent_pid() -> u32 {
    // std has no ppid; fall back to 0 (only reduces process-id entropy).
    #[cfg(target_os = "linux")]
    {
        std::fs::read_to_string("/proc/self/stat")
            .ok()
            .and_then(|s| {
                // field 4 (ppid); fields are space-separated, field 2 (comm) may
                // contain spaces inside parentheses, so split after the last ')'.
                s.rsplit_once(')').map(|(_, rest)| {
                    rest.split_whitespace()
                        .next()
                        .and_then(|p| p.parse::<u32>().ok())
                })?
            })
            .unwrap_or(0)
    }
    #[cfg(not(target_os = "linux"))]
    {
        unsafe { libc_ppid() }
    }
}

#[cfg(not(target_os = "linux"))]
unsafe fn libc_ppid() -> u32 {
    // No libc crate in deps; use sysinfo-free approximation via `ps`.
    std::process::Command::new("ps")
        .args(["-o", "ppid=", "-p", &std::process::id().to_string()])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.trim().parse::<u32>().ok())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn serializes_as_string() {
        let v = serde_json::to_string(&Snowflake(12345)).unwrap();
        assert_eq!(v, "\"12345\"");
    }

    #[test]
    fn deserializes_string_and_number() {
        let a: Snowflake = serde_json::from_str("\"42\"").unwrap();
        let b: Snowflake = serde_json::from_str("42").unwrap();
        assert_eq!(a, b);
        assert_eq!(a.0, 42);
    }

    #[test]
    fn generates_unique_ids_across_wrap() {
        let generator = SnowflakeGenerator::with_ids(SNOWFLAKE_EPOCH_MS, 1, 1);
        let seen: HashSet<u64> = (0..3 * (MAX_INCREMENT as usize + 1) + 7)
            .map(|_| generator.generate().0)
            .collect();
        assert_eq!(seen.len(), 3 * (MAX_INCREMENT as usize + 1) + 7);
    }

    #[test]
    fn ids_are_time_ordered() {
        let generator = SnowflakeGenerator::with_ids(SNOWFLAKE_EPOCH_MS, 1, 1);
        let a = generator.generate().0;
        let b = generator.generate().0;
        assert!(a < b);
    }
}
