use opentelemetry::KeyValue;
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_otlp::{LogExporter, SpanExporter};
use opentelemetry_sdk::{logs::LoggerProvider, runtime, trace::TracerProvider, Resource};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::sync::{Arc, OnceLock};
use sysinfo::System;
use tokio::sync::Mutex;
use tokio::time::{self, Duration};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use crate::helpers::cache;

// Kept alive for the process lifetime; Drop flushes all pending OTel batches.
pub struct OtelGuard {
    logger_provider: Option<LoggerProvider>,
    tracer_provider: Option<TracerProvider>,
}

impl Drop for OtelGuard {
    fn drop(&mut self) {
        if let Some(p) = self.tracer_provider.take() {
            p.shutdown().ok();
        }
        if let Some(p) = self.logger_provider.take() {
            p.shutdown().ok();
        }
    }
}

/// Initialises tracing + optional OTel log export to Grafana.
///
/// When `OTEL_EXPORTER_OTLP_ENDPOINT` is set, every `tracing::info!` /
/// `warn!` / `error!` call is forwarded to Grafana Cloud via OTLP HTTP in
/// addition to stdout. Falls back to plain stdout when the env var is absent.
pub fn init_otel() -> OtelGuard {
    if std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").is_err() {
        tracing_subscriber::fmt()
            .with_target(false)
            .with_level(true)
            .with_max_level(tracing::Level::INFO)
            .init();
        return OtelGuard {
            logger_provider: None,
            tracer_provider: None,
        };
    }

    let resource = Resource::new(vec![KeyValue::new(
        "service.name",
        std::env::var("OTEL_SERVICE_NAME").unwrap_or_else(|_| "sakkath-api".to_string()),
    )]);

    // --- Logs ---
    let log_exporter = LogExporter::builder()
        .with_http()
        .build()
        .expect("OTel log exporter");

    let logger_provider = LoggerProvider::builder()
        .with_resource(resource.clone())
        .with_batch_exporter(log_exporter, runtime::Tokio)
        .build();

    // --- Traces ---
    let span_exporter = SpanExporter::builder()
        .with_http()
        .build()
        .expect("OTel span exporter");

    let tracer_provider = TracerProvider::builder()
        .with_resource(resource)
        .with_batch_exporter(span_exporter, runtime::Tokio)
        .build();

    opentelemetry::global::set_tracer_provider(tracer_provider.clone());

    let tracer = tracer_provider.tracer("sakkath-api");

    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer().with_target(false))
        .with(OpenTelemetryTracingBridge::new(&logger_provider))
        .with(tracing_opentelemetry::layer().with_tracer(tracer))
        .init();

    OtelGuard {
        logger_provider: Some(logger_provider),
        tracer_provider: Some(tracer_provider),
    }
}


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

            let cpu = system.global_cpu_usage() as f64;
            let memory_used_mb = system.used_memory() as f64 / (1024.0 * 1024.0);
            let memory_percent = if system.total_memory() == 0 {
                0.0
            } else {
                (system.used_memory() as f64 / system.total_memory() as f64) * 100.0
            };

            tracing::info!(
                cpu_percent = cpu,
                memory_mb = memory_used_mb,
                memory_percent = memory_percent,
                "system heartbeat"
            );

            enqueue(
                &runtime,
                TelemetryEntry::System {
                    cpu_percent: cpu,
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
