use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::sync::{Arc, OnceLock};
use sysinfo::System;
use tokio::sync::Mutex;
use tokio::time::{self, Duration};

use crate::helpers::cache;

const TELEMETRY_QUEUE_KEY: &str = "telemetry:buffer";
const TELEMETRY_FLUSH_BATCH_SIZE: usize = 2_000;

static TELEMETRY_RUNTIME: OnceLock<Arc<TelemetryRuntime>> = OnceLock::new();

#[derive(Clone)]
struct TelemetryRuntime {
    enabled: bool,
    db: SqlitePool,
    fallback_queue: Arc<Mutex<Vec<TelemetryEntry>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TelemetryEntry {
    System {
        cpu_percent: f64,
        memory_mb_used: f64,
        memory_percent: f64,
    },
    Request {
        method: String,
        path: String,
        ip_address: String,
        status_code: i64,
    },
    Event {
        level: String,
        key: String,
        details: String,
    },
}

pub async fn init(db: SqlitePool, enabled: bool) {
    let runtime = TELEMETRY_RUNTIME.get_or_init(|| {
        Arc::new(TelemetryRuntime {
            enabled,
            db,
            fallback_queue: Arc::new(Mutex::new(Vec::new())),
        })
    });

    if enabled {
        spawn_worker(runtime.clone());
    }
}

pub fn record_request(method: String, path: String, ip_address: String, status_code: i64) {
    let Some(runtime) = TELEMETRY_RUNTIME.get().cloned() else {
        return;
    };

    if !runtime.enabled || path == "/telemetry" {
        return;
    }

    tokio::spawn(async move {
        enqueue(
            &runtime,
            TelemetryEntry::Request {
                method,
                path,
                ip_address,
                status_code,
            },
        )
        .await;
    });
}

pub fn record_info(key: impl Into<String>, details: impl Into<String>) {
    record_event("info".to_string(), key.into(), details.into());
}

pub fn record_error(key: impl Into<String>, details: impl Into<String>) {
    record_event("error".to_string(), key.into(), details.into());
}

fn record_event(level: String, key: String, details: String) {
    let Some(runtime) = TELEMETRY_RUNTIME.get().cloned() else {
        return;
    };

    match level.as_str() {
        "error" => {
            tracing::error!(event_key = %key, event_details = %details, "application event");
        }
        _ => {
            tracing::info!(event_key = %key, event_details = %details, "application event");
        }
    }

    if !runtime.enabled {
        return;
    }

    tokio::spawn(async move {
        enqueue(
            &runtime,
            TelemetryEntry::Event {
                level,
                key,
                details,
            },
        )
        .await;
    });
}

async fn enqueue(runtime: &TelemetryRuntime, entry: TelemetryEntry) {
    if cache::push_json_list(TELEMETRY_QUEUE_KEY, &entry).await {
        return;
    }

    runtime.fallback_queue.lock().await.push(entry);
}

fn spawn_worker(runtime: Arc<TelemetryRuntime>) {
    tokio::spawn(async move {
        let mut ticker = time::interval(Duration::from_secs(10));
        let mut system = System::new_all();

        loop {
            ticker.tick().await;

            system.refresh_all();

            let memory_used_mb = system.used_memory() as f64 / (1024.0 * 1024.0);
            let memory_percent = if system.total_memory() == 0 {
                0.0
            } else {
                (system.used_memory() as f64 / system.total_memory() as f64) * 100.0
            };

            enqueue(
                &runtime,
                TelemetryEntry::System {
                    cpu_percent: system.global_cpu_usage() as f64,
                    memory_mb_used: memory_used_mb,
                    memory_percent,
                },
            )
            .await;

            if let Err(error) = flush_entries(&runtime).await {
                tracing::warn!("Telemetry flush failed: {}", error);
            }
        }
    });
}

async fn flush_entries(runtime: &TelemetryRuntime) -> Result<(), sqlx::Error> {
    let mut entries = Vec::new();

    for _ in 0..TELEMETRY_FLUSH_BATCH_SIZE {
        match cache::pop_json_list::<TelemetryEntry>(TELEMETRY_QUEUE_KEY).await {
            Some(entry) => entries.push(entry),
            None => break,
        }
    }

    let fallback_entries = {
        let mut fallback = runtime.fallback_queue.lock().await;
        if fallback.is_empty() {
            Vec::new()
        } else {
            fallback.drain(..).collect::<Vec<_>>()
        }
    };
    entries.extend(fallback_entries);

    if entries.is_empty() {
        return Ok(());
    }

    let mut tx = runtime.db.begin().await?;
    for entry in entries {
        match entry {
            TelemetryEntry::System {
                cpu_percent,
                memory_mb_used,
                memory_percent,
            } => {
                sqlx::query(
                    "INSERT INTO telemetry_logs (kind, cpu_percent, memory_mb_used, memory_percent) VALUES ('system', ?, ?, ?)",
                )
                .bind(cpu_percent)
                .bind(memory_mb_used)
                .bind(memory_percent)
                .execute(&mut *tx)
                .await?;
            }
            TelemetryEntry::Request {
                method,
                path,
                ip_address,
                status_code,
            } => {
                sqlx::query(
                    "INSERT INTO telemetry_logs (kind, method, path, ip_address, status_code) VALUES ('request', ?, ?, ?, ?)",
                )
                .bind(method)
                .bind(path)
                .bind(ip_address)
                .bind(status_code)
                .execute(&mut *tx)
                .await?;
            }
            TelemetryEntry::Event {
                level,
                key,
                details,
            } => {
                sqlx::query(
                    "INSERT INTO telemetry_logs (kind, method, path, details) VALUES ('event', ?, ?, ?)",
                )
                .bind(level)
                .bind(key)
                .bind(details)
                .execute(&mut *tx)
                .await?;
            }
        }
    }
    tx.commit().await?;

    Ok(())
}
