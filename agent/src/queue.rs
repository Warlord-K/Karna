use anyhow::Result;
use redis::AsyncCommands;
use uuid::Uuid;

const LOCK_TTL_SECS: u64 = 1800; // 30 minutes
const HEARTBEAT_SECS: u64 = 60;

/// Try to acquire an exclusive lock on a task. Returns true if acquired.
/// Uses Redis SET NX EX for atomic claim.
pub async fn try_lock(client: &redis::Client, task_id: Uuid, worker_id: &str) -> Result<bool> {
    let mut conn = client.get_multiplexed_async_connection().await?;
    let key = lock_key(task_id);

    let result: Option<String> = redis::cmd("SET")
        .arg(&key)
        .arg(worker_id)
        .arg("NX")
        .arg("EX")
        .arg(LOCK_TTL_SECS)
        .query_async(&mut conn)
        .await?;

    Ok(result.is_some())
}

/// Extend the lock TTL (heartbeat while working).
pub async fn heartbeat(client: &redis::Client, task_id: Uuid) -> Result<()> {
    let mut conn = client.get_multiplexed_async_connection().await?;
    let key = lock_key(task_id);
    conn.expire::<_, ()>(&key, LOCK_TTL_SECS as i64).await?;
    Ok(())
}

/// Release the lock when done.
pub async fn release(client: &redis::Client, task_id: Uuid) -> Result<()> {
    let mut conn = client.get_multiplexed_async_connection().await?;
    let key = lock_key(task_id);
    conn.del::<_, ()>(&key).await?;
    Ok(())
}

/// Check if a task is currently locked by any worker.
pub async fn is_locked(client: &redis::Client, task_id: Uuid) -> Result<bool> {
    let mut conn = client.get_multiplexed_async_connection().await?;
    let key = lock_key(task_id);
    let exists: bool = conn.exists(&key).await?;
    Ok(exists)
}

fn lock_key(task_id: Uuid) -> String {
    format!("task_lock:{task_id}")
}

// --- Generalized lock helpers (for schedules, etc.) ---

/// Try to acquire an exclusive lock on an arbitrary key. Returns true if acquired.
pub async fn try_lock_key(client: &redis::Client, key: &str, worker_id: &str) -> Result<bool> {
    let mut conn = client.get_multiplexed_async_connection().await?;

    let result: Option<String> = redis::cmd("SET")
        .arg(key)
        .arg(worker_id)
        .arg("NX")
        .arg("EX")
        .arg(LOCK_TTL_SECS)
        .query_async(&mut conn)
        .await?;

    Ok(result.is_some())
}

/// Release an arbitrary lock key.
pub async fn release_key(client: &redis::Client, key: &str) -> Result<()> {
    let mut conn = client.get_multiplexed_async_connection().await?;
    conn.del::<_, ()>(key).await?;
    Ok(())
}

/// Check if a Redis key exists (used to detect schedule triggers).
pub async fn key_exists(client: &redis::Client, key: &str) -> Result<bool> {
    let mut conn = client.get_multiplexed_async_connection().await?;
    let exists: bool = conn.exists(key).await?;
    Ok(exists)
}

/// Delete a Redis key.
pub async fn delete_key(client: &redis::Client, key: &str) -> Result<()> {
    let mut conn = client.get_multiplexed_async_connection().await?;
    conn.del::<_, ()>(key).await?;
    Ok(())
}

/// Generate a unique worker ID for this process.
pub fn worker_id() -> String {
    let hostname = hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_else(|_| "unknown".to_string());
    let pid = std::process::id();
    format!("{hostname}-{pid}")
}
