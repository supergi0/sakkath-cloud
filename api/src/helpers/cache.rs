use serde::{Serialize, de::DeserializeOwned};
use std::sync::OnceLock;
use tokio::sync::Mutex;

static REDIS_CLIENT: OnceLock<Mutex<Option<redis::aio::MultiplexedConnection>>> = OnceLock::new();

// Cache key prefixes
const STANDINGS_PREFIX: &str = "standings";
const INTERMEDIATE_STANDINGS_PREFIX: &str = "standings_intermediate";
const SCHEDULE_PREFIX: &str = "schedule";
const ROUND_PREFIX: &str = "round_pairings";
const PLAYER_STATS_KEY: &str = "player_stats";
const DEFAULT_TTL: u64 = 300; // 5 minutes

// Initialize redis connection (call once at startup)
pub async fn init_redis() -> bool {
    let url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
    match redis::Client::open(url.as_str()) {
        Ok(client) => match client.get_multiplexed_async_connection().await {
            Ok(conn) => {
                let lock = REDIS_CLIENT.get_or_init(|| Mutex::new(None));
                *lock.lock().await = Some(conn);
                tracing::info!("Redis connected at {}", url);
                true
            }
            Err(e) => {
                tracing::warn!("Redis unavailable ({}), running without cache", e);
                REDIS_CLIENT.get_or_init(|| Mutex::new(None));
                false
            }
        },
        Err(e) => {
            tracing::warn!("Redis client error ({}), running without cache", e);
            REDIS_CLIENT.get_or_init(|| Mutex::new(None));
            false
        }
    }
}

async fn get_conn() -> Option<redis::aio::MultiplexedConnection> {
    REDIS_CLIENT.get()?.lock().await.clone()
}

// Generic get: returns None if cache miss or redis down
pub async fn get<T: DeserializeOwned>(key: &str) -> Option<T> {
    let mut conn = get_conn().await?;
    let val: Option<String> = redis::cmd("GET")
        .arg(key)
        .query_async(&mut conn)
        .await
        .ok()?;
    val.and_then(|s| serde_json::from_str(&s).ok())
}

// Generic set with TTL
pub async fn set<T: Serialize>(key: &str, value: &T, ttl_secs: u64) {
    if let Some(mut conn) = get_conn().await
        && let Ok(json) = serde_json::to_string(value)
    {
        let _: Result<(), _> = redis::cmd("SETEX")
            .arg(key)
            .arg(ttl_secs)
            .arg(json)
            .query_async(&mut conn)
            .await;
    }
}

// Delete a key
pub async fn del(key: &str) {
    if let Some(mut conn) = get_conn().await {
        let _: Result<(), _> = redis::cmd("DEL").arg(key).query_async(&mut conn).await;
    }
}

// Delete all keys matching a pattern
pub async fn del_pattern(pattern: &str) {
    if let Some(mut conn) = get_conn().await {
        let keys: Vec<String> = redis::cmd("KEYS")
            .arg(pattern)
            .query_async(&mut conn)
            .await
            .unwrap_or_default();
        for key in keys {
            let _: Result<(), _> = redis::cmd("DEL").arg(&key).query_async(&mut conn).await;
        }
    }
}

// Push a JSON value onto a Redis list.
pub async fn push_json_list<T: Serialize>(key: &str, value: &T) -> bool {
    if let Some(mut conn) = get_conn().await
        && let Ok(json) = serde_json::to_string(value)
    {
        let result: Result<i64, _> = redis::cmd("RPUSH")
            .arg(key)
            .arg(json)
            .query_async(&mut conn)
            .await;
        return result.is_ok();
    }

    false
}

// Pop a JSON value from the head of a Redis list.
pub async fn pop_json_list<T: DeserializeOwned>(key: &str) -> Option<T> {
    let mut conn = get_conn().await?;
    let value: Option<String> = redis::cmd("LPOP")
        .arg(key)
        .query_async(&mut conn)
        .await
        .ok()?;

    value.and_then(|entry| serde_json::from_str(&entry).ok())
}

// Increment a Redis counter and attach TTL on first write.
pub async fn incr_with_ttl(key: &str, ttl_secs: u64) -> Option<i64> {
    let mut conn = get_conn().await?;
    let value: i64 = redis::cmd("INCR")
        .arg(key)
        .query_async(&mut conn)
        .await
        .ok()?;

    if value == 1 {
        let _: Result<bool, _> = redis::cmd("EXPIRE")
            .arg(key)
            .arg(ttl_secs)
            .query_async(&mut conn)
            .await;
    }

    Some(value)
}

// --- Domain-specific cache helpers ---

// Standings cache key for a division
fn standings_key(division: i64) -> String {
    format!("{}:{}", STANDINGS_PREFIX, division)
}

// Intermediate standings cache key for a division
fn intermediate_standings_key(division: i64) -> String {
    format!("{}:{}", INTERMEDIATE_STANDINGS_PREFIX, division)
}

// Schedule cache key for division + optional round
fn schedule_key(division: i64, round: Option<i64>, status: Option<&str>) -> String {
    let status = status.unwrap_or("all");
    match round {
        Some(r) => format!("{}:{}:{}:{}", SCHEDULE_PREFIX, division, r, status),
        None => format!("{}:{}:{}", SCHEDULE_PREFIX, division, status),
    }
}

// Round pairings key
fn round_key(division: i64, round: i64) -> String {
    format!("{}:{}:{}", ROUND_PREFIX, division, round)
}

// Get cached standings
pub async fn get_standings<T: DeserializeOwned>(division: i64) -> Option<T> {
    get(&standings_key(division)).await
}

// Set standings cache
pub async fn set_standings<T: Serialize>(division: i64, data: &T) {
    set(&standings_key(division), data, DEFAULT_TTL).await;
}

// Get cached intermediate standings
pub async fn get_intermediate_standings<T: DeserializeOwned>(division: i64) -> Option<T> {
    get(&intermediate_standings_key(division)).await
}

// Set intermediate standings cache
pub async fn set_intermediate_standings<T: Serialize>(division: i64, data: &T) {
    set(&intermediate_standings_key(division), data, DEFAULT_TTL).await;
}

// Get cached schedule
pub async fn get_schedule<T: DeserializeOwned>(
    division: i64,
    round: Option<i64>,
    status: Option<&str>,
) -> Option<T> {
    get(&schedule_key(division, round, status)).await
}

// Set schedule cache
pub async fn set_schedule<T: Serialize>(
    division: i64,
    round: Option<i64>,
    status: Option<&str>,
    data: &T,
) {
    set(&schedule_key(division, round, status), data, DEFAULT_TTL).await;
}

// Get cached player stats
pub async fn get_player_stats<T: DeserializeOwned>() -> Option<T> {
    get(PLAYER_STATS_KEY).await
}

// Set player stats cache
pub async fn set_player_stats<T: Serialize>(data: &T) {
    set(PLAYER_STATS_KEY, data, DEFAULT_TTL).await;
}

// Invalidate player stats cache
pub async fn invalidate_player_stats() {
    del(PLAYER_STATS_KEY).await;
}

// Get cached round pairings
pub async fn get_round_pairings<T: DeserializeOwned>(division: i64, round: i64) -> Option<T> {
    get(&round_key(division, round)).await
}

// Set round pairings cache
pub async fn set_round_pairings<T: Serialize>(division: i64, round: i64, data: &T) {
    set(&round_key(division, round), data, DEFAULT_TTL).await;
}

// Invalidate all caches for a division (call on write operations)
pub async fn invalidate_division(division: i64) {
    del(&standings_key(division)).await;
    del(&intermediate_standings_key(division)).await;
    del_pattern(&format!("{}:{}:*", SCHEDULE_PREFIX, division)).await;
    del_pattern(&format!("{}:{}:*", ROUND_PREFIX, division)).await;
}

// Invalidate everything (nuclear option)
pub async fn invalidate_all() {
    for div in 0..=1 {
        invalidate_division(div).await;
    }
}
