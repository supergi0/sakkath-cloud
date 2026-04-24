use axum::{Json, extract::State};
use serde::Serialize;

#[derive(Serialize)]
pub struct TelemetryResponse {
    pub enabled: bool,
    pub hardware: HardwareTelemetry,
    pub requests: RequestTelemetry,
}

#[derive(Serialize)]
pub struct HardwareTelemetry {
    pub peak: ResourcePoint,
    pub average: ResourcePoint,
    pub latest: ResourcePoint,
}

#[derive(Serialize)]
pub struct ResourcePoint {
    pub cpu_percent: f64,
    pub memory_mb_used: f64,
    pub memory_percent: f64,
}

#[derive(Serialize)]
pub struct RequestTelemetry {
    pub max_requests_per_minute: i64,
    pub average_requests_per_minute: f64,
    pub current_requests_per_minute: i64,
    pub max_requests_per_hour: i64,
    pub average_requests_per_hour: f64,
    pub current_requests_per_hour: i64,
    pub top_endpoints_last_hour: Vec<CountByKey>,
}

#[derive(Serialize)]
pub struct CountByKey {
    pub key: String,
    pub count: i64,
}

pub async fn get_telemetry(State(state): State<crate::AppState>) -> Json<TelemetryResponse> {
    let hardware_peak: (Option<f64>, Option<f64>, Option<f64>) = sqlx::query_as(
        r#"SELECT MAX(cpu_percent), MAX(memory_mb_used), MAX(memory_percent)
           FROM telemetry_logs
           WHERE kind = 'system'"#,
    )
    .fetch_one(&state.db)
    .await
    .unwrap_or((None, None, None));

    let hardware_average: (Option<f64>, Option<f64>, Option<f64>) = sqlx::query_as(
        r#"SELECT AVG(cpu_percent), AVG(memory_mb_used), AVG(memory_percent)
           FROM telemetry_logs
           WHERE kind = 'system'"#,
    )
    .fetch_one(&state.db)
    .await
    .unwrap_or((None, None, None));

    let hardware_latest: Option<(Option<f64>, Option<f64>, Option<f64>)> = sqlx::query_as(
        r#"SELECT cpu_percent, memory_mb_used, memory_percent
           FROM telemetry_logs
           WHERE kind = 'system'
           ORDER BY created_at DESC, id DESC
           LIMIT 1"#,
    )
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);

    let minute_buckets: Vec<(i64,)> = sqlx::query_as(
        r#"SELECT COUNT(*) as count
           FROM telemetry_logs
           WHERE kind = 'request'
           GROUP BY strftime('%Y-%m-%d %H:%M', created_at)"#,
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let hour_buckets: Vec<(i64,)> = sqlx::query_as(
        r#"SELECT COUNT(*) as count
           FROM telemetry_logs
           WHERE kind = 'request'
           GROUP BY strftime('%Y-%m-%d %H', created_at)"#,
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let current_requests_per_minute: (i64,) = sqlx::query_as(
        r#"SELECT COUNT(*)
           FROM telemetry_logs
           WHERE kind = 'request'
             AND created_at >= datetime('now', '-1 minute')"#,
    )
    .fetch_one(&state.db)
    .await
    .unwrap_or((0,));

    let current_requests_per_hour: (i64,) = sqlx::query_as(
        r#"SELECT COUNT(*)
           FROM telemetry_logs
           WHERE kind = 'request'
             AND created_at >= datetime('now', '-1 hour')"#,
    )
    .fetch_one(&state.db)
    .await
    .unwrap_or((0,));

    let top_endpoints_last_hour = sqlx::query_as::<_, (String, i64)>(
        r#"SELECT path, COUNT(*) as count
           FROM telemetry_logs
           WHERE kind = 'request'
             AND path IS NOT NULL
             AND created_at >= datetime('now', '-1 hour')
           GROUP BY path
           ORDER BY count DESC, path ASC
           LIMIT 10"#,
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|(key, count)| CountByKey { key, count })
    .collect::<Vec<_>>();

    Json(TelemetryResponse {
        enabled: state.telemetry_enabled,
        hardware: HardwareTelemetry {
            peak: ResourcePoint {
                cpu_percent: hardware_peak.0.unwrap_or(0.0),
                memory_mb_used: hardware_peak.1.unwrap_or(0.0),
                memory_percent: hardware_peak.2.unwrap_or(0.0),
            },
            average: ResourcePoint {
                cpu_percent: hardware_average.0.unwrap_or(0.0),
                memory_mb_used: hardware_average.1.unwrap_or(0.0),
                memory_percent: hardware_average.2.unwrap_or(0.0),
            },
            latest: ResourcePoint {
                cpu_percent: hardware_latest
                    .as_ref()
                    .and_then(|row| row.0)
                    .unwrap_or(0.0),
                memory_mb_used: hardware_latest
                    .as_ref()
                    .and_then(|row| row.1)
                    .unwrap_or(0.0),
                memory_percent: hardware_latest
                    .as_ref()
                    .and_then(|row| row.2)
                    .unwrap_or(0.0),
            },
        },
        requests: RequestTelemetry {
            max_requests_per_minute: minute_buckets.iter().map(|row| row.0).max().unwrap_or(0),
            average_requests_per_minute: average_bucket(&minute_buckets),
            current_requests_per_minute: current_requests_per_minute.0,
            max_requests_per_hour: hour_buckets.iter().map(|row| row.0).max().unwrap_or(0),
            average_requests_per_hour: average_bucket(&hour_buckets),
            current_requests_per_hour: current_requests_per_hour.0,
            top_endpoints_last_hour,
        },
    })
}

fn average_bucket(rows: &[(i64,)]) -> f64 {
    if rows.is_empty() {
        return 0.0;
    }

    let total: i64 = rows.iter().map(|row| row.0).sum();
    total as f64 / rows.len() as f64
}
