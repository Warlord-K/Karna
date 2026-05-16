//! Cache helpers + key builders. Read-through cache via `get_or_set`;
//! writes invalidate inside `Database` so every mutation (API or agent)
//! drops the relevant keys.

use std::future::Future;

use anyhow::Result;
use serde::{de::DeserializeOwned, Serialize};
use tracing::warn;
use uuid::Uuid;

pub const DEFAULT_TTL_SECS: u64 = 600; // 10 minutes

/// Read-through cache. On miss (or any Redis error), falls through to `fetch`
/// and stores the result. Cache failures never fail the request.
pub async fn get_or_set<T, F, Fut>(
    redis: &redis::Client,
    key: &str,
    ttl_secs: u64,
    fetch: F,
) -> Result<T>
where
    T: Serialize + DeserializeOwned,
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<T>>,
{
    if let Ok(mut conn) = redis.get_multiplexed_async_connection().await {
        let cached: Option<String> = redis::cmd("GET")
            .arg(key)
            .query_async(&mut conn)
            .await
            .ok()
            .flatten();
        if let Some(json) = cached {
            match serde_json::from_str::<T>(&json) {
                Ok(value) => return Ok(value),
                Err(e) => warn!("cache deserialize failed for {key}: {e}; refetching"),
            }
        }
    }

    let value = fetch().await?;

    if let Ok(json) = serde_json::to_string(&value) {
        if let Ok(mut conn) = redis.get_multiplexed_async_connection().await {
            let _: Result<(), _> = redis::cmd("SET")
                .arg(key)
                .arg(&json)
                .arg("EX")
                .arg(ttl_secs)
                .query_async(&mut conn)
                .await;
        }
    }

    Ok(value)
}

/// Delete a single cache key. Best-effort; never fails the caller.
pub async fn invalidate(redis: &redis::Client, key: &str) {
    if let Ok(mut conn) = redis.get_multiplexed_async_connection().await {
        let _: Result<(), redis::RedisError> =
            redis::cmd("DEL").arg(key).query_async(&mut conn).await;
    }
}

/// Delete every key matching a glob pattern. Uses SCAN to avoid blocking Redis.
/// Best-effort; never fails the caller.
pub async fn invalidate_pattern(redis: &redis::Client, pattern: &str) {
    let Ok(mut conn) = redis.get_multiplexed_async_connection().await else {
        return;
    };
    let mut cursor: u64 = 0;
    loop {
        let res: Result<(u64, Vec<String>), _> = redis::cmd("SCAN")
            .arg(cursor)
            .arg("MATCH")
            .arg(pattern)
            .arg("COUNT")
            .arg(100)
            .query_async(&mut conn)
            .await;
        let (next, keys) = match res {
            Ok(v) => v,
            Err(_) => return,
        };
        if !keys.is_empty() {
            let _: Result<(), _> =
                redis::cmd("DEL").arg(&keys).query_async(&mut conn).await;
        }
        cursor = next;
        if cursor == 0 {
            break;
        }
    }
}

// --- Key builders ---

pub fn tasks_list_key(user_id: Uuid) -> String {
    format!("cache:tasks:list:{user_id}")
}

pub const TASKS_LIST_PATTERN: &str = "cache:tasks:list:*";

pub fn tasks_logs_key(task_id: Uuid) -> String {
    format!("cache:tasks:logs:{task_id}")
}

pub fn schedules_list_key(user_id: Uuid) -> String {
    format!("cache:schedules:list:{user_id}")
}

pub const SCHEDULES_LIST_PATTERN: &str = "cache:schedules:list:*";

pub fn schedule_runs_key(schedule_id: Uuid) -> String {
    format!("cache:schedules:runs:{schedule_id}")
}

pub fn schedule_run_logs_key(run_id: Uuid) -> String {
    format!("cache:schedules:run_logs:{run_id}")
}

pub const REPOS_LIST_KEY: &str = "cache:repos:list";
pub const CONFIG_KEY: &str = "cache:config";
pub const AGENTS_LIST_KEY: &str = "cache:agents:list";
