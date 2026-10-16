use liar_server::{
    auth::{AuthClock, SystemAuthClock},
    events::*,
};
use serde_json::{Value, json};
use sqlx::postgres::PgPoolOptions;
use std::{process::ExitCode, sync::Arc, time::Duration};

#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args == ["--help"] {
        println!("metrics <read|refresh> <weekly_nsm|d7> <YYYY-MM-DD>");
        return ExitCode::SUCCESS;
    }
    match run(&args).await {
        Ok(output) => {
            println!("{output}");
            ExitCode::SUCCESS
        }
        Err(code) => {
            eprintln!("{}", json!({"code":code}));
            ExitCode::FAILURE
        }
    }
}
async fn run(args: &[String]) -> Result<Value, &'static str> {
    if args.len() != 3 || !matches!(args[0].as_str(), "read" | "refresh") {
        return Err("invalid_arguments");
    }
    let kind = MetricKind::parse(&args[1]).map_err(|_| "invalid_arguments")?;
    let period = UtcDay::parse(&args[2]).map_err(|_| "invalid_arguments")?;
    let url = std::env::var("DATABASE_URL").map_err(|_| "configuration_missing")?;
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(Duration::from_secs(1))
        .connect_lazy(&url)
        .map_err(|_| "configuration_invalid")?;
    let store = PgAggregateStore::new(pool.clone(), Arc::new(SystemAuthClock));
    let result = if args[0] == "refresh" {
        store
            .refresh(kind, period, Duration::from_secs(2))
            .await
            .map(Some)
    } else {
        store.read(kind, period).await
    };
    let output = match result {
        Ok(Some(report)) => operator_view(report, SystemAuthClock.now()),
        Ok(None) => Ok(json!({"status":"missing","metric":kind,"period":period})),
        Err(MetricReadError::Policy(EventError::Expired)) if args[0] == "read" => Ok(expired_view(
            kind,
            period,
            aggregate_expires_at(period).map_err(|_| "invalid_period")?,
        )),
        Err(error) => Err(code(error)),
    };
    pool.close().await;
    output
}
fn operator_view(report: AggregateReport, now: i64) -> Result<Value, &'static str> {
    let stale = match report.stale(now) {
        Ok(stale) => stale,
        Err(EventError::Expired) => {
            return Ok(expired_view(
                report.snapshot.kind(),
                report.period,
                report.expires_at,
            ));
        }
        Err(_) => return Err("invalid_clock"),
    };
    let rate = match &report.snapshot {
        MetricSnapshot::D7(r) => r.rate(),
        _ => None,
    };
    Ok(json!({"status":"available","report":report,"stale":stale,"d7_rate":rate}))
}
fn expired_view(kind: MetricKind, period: UtcDay, expires_at: i64) -> Value {
    json!({"status":"expired","metric":kind,"period":period,"expires_at":expires_at})
}
fn code(error: MetricReadError) -> &'static str {
    match error {
        MetricReadError::Unavailable => "unavailable",
        MetricReadError::Policy(EventError::Incomplete) => "incomplete",
        MetricReadError::Policy(EventError::Capacity) => "capacity",
        MetricReadError::Policy(EventError::Conflict) => "conflict",
        MetricReadError::Policy(EventError::Expired) => "expired",
        MetricReadError::Policy(_) => "invalid",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn expiry_between_database_read_and_operator_output_is_an_expired_view() {
        let week = UtcDay::parse("2026-10-05").unwrap();
        let at = week.start() + 8 * 86400;
        let report = AggregateReport::new(
            MetricSnapshot::WeeklyNsm(WeeklyNsm {
                week,
                observed_through: week.start() + 7 * 86400,
                complete: true,
                participants: 0,
                repeat_accounts: 0,
                human_participations: 0,
                bot_participations: 0,
            }),
            at,
        )
        .unwrap();
        assert_eq!(
            operator_view(report.clone(), at).unwrap()["status"],
            "available"
        );
        assert_eq!(operator_view(report.clone(), at - 1), Err("invalid_clock"));
        let view = operator_view(report.clone(), report.expires_at).unwrap();
        assert_eq!(view["status"], "expired");
        assert!(view.get("report").is_none());
    }
}
