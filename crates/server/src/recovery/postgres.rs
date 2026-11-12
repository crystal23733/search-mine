use super::model::{Erasure, Kind};
use super::{Ledger, RecoveryError, Snapshot, SnapshotMatch};
use crate::{
    auth::{AuthClock, SystemAuthClock},
    online::recovery_lock,
};
use sqlx::{PgPool, Postgres, Transaction};
use std::{future::Future, time::Duration};
use uuid::Uuid;
const LIMIT: Duration = Duration::from_secs(2);
fn db(_: sqlx::Error) -> RecoveryError {
    RecoveryError::Unavailable
}
async fn bounded<T>(
    work: impl Future<Output = Result<T, RecoveryError>>,
) -> Result<T, RecoveryError> {
    tokio::time::timeout(LIMIT, work)
        .await
        .unwrap_or(Err(RecoveryError::Unavailable))
}
pub async fn ensure_serving(pool: &PgPool) -> Result<(), RecoveryError> {
    bounded(async{
        let active:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM service_recovery_control WHERE singleton AND state='active' AND database_oid=(SELECT oid::bigint FROM pg_database WHERE datname=current_database()) AND schema_oid=current_schema()::regnamespace::oid::bigint)")
            .fetch_one(pool).await.map_err(db)?;
        if active {Ok(())}else{Err(RecoveryError::Blocked)}
    }).await
}
struct Operation<'a> {
    clock: &'a dyn AuthClock,
    last: i64,
    deadline: tokio::time::Instant,
}
impl<'a> Operation<'a> {
    fn new(clock: &'a dyn AuthClock) -> Result<Self, RecoveryError> {
        let now = clock.now();
        if !(0..=253402300799).contains(&now) {
            return Err(RecoveryError::Invalid);
        }
        Ok(Self {
            clock,
            last: now,
            deadline: tokio::time::Instant::now() + LIMIT,
        })
    }
    async fn check(&mut self, tx: &mut Transaction<'_, Postgres>) -> Result<(), RecoveryError> {
        let now = self.clock.now();
        if !(self.last..=253402300799).contains(&now) {
            return Err(RecoveryError::Invalid);
        }
        self.last = now;
        let millis = self
            .deadline
            .saturating_duration_since(tokio::time::Instant::now())
            .as_millis();
        if millis == 0 {
            return Err(RecoveryError::Unavailable);
        }
        sqlx::query("SELECT set_config('statement_timeout',$1,true),set_config('lock_timeout',$1,true),set_config('TimeZone','UTC',true)")
            .bind(format!("{millis}ms")).execute(&mut **tx).await.map_err(db)?;
        Ok(())
    }
    async fn own(&mut self, tx: &mut Transaction<'_, Postgres>) -> Result<(), RecoveryError> {
        self.check(tx).await?;
        if !recovery_lock(tx)
            .await
            .map_err(|_| RecoveryError::Unavailable)?
        {
            return Err(RecoveryError::Busy);
        }
        Ok(())
    }
}
/// Only a proof captured before migrations may initialize a factory-empty schema.
pub struct FreshSchema {
    empty: bool,
    database: i64,
    schema: i64,
}
impl FreshSchema {
    pub async fn inspect(pool: &PgPool) -> Result<Self, RecoveryError> {
        bounded(async{
            let (empty,database,schema):(bool,i64,i64)=sqlx::query_as("SELECT NOT EXISTS(SELECT 1 FROM pg_class WHERE relnamespace=current_schema()::regnamespace AND relkind IN ('r','p','f','v','m')),(SELECT oid::bigint FROM pg_database WHERE datname=current_database()),current_schema()::regnamespace::oid::bigint")
                .fetch_one(pool).await.map_err(db)?;
            Ok(Self{empty,database,schema})
        }).await
    }
    pub async fn initialize(self, pool: &PgPool) -> Result<(), RecoveryError> {
        if !self.empty {
            return Ok(());
        }
        bounded(async{
            let mut op=Operation::new(&SystemAuthClock)?;let mut tx=pool.begin().await.map_err(db)?;op.own(&mut tx).await?;
            let binding:(i64,i64)=sqlx::query_as("SELECT (SELECT oid::bigint FROM pg_database WHERE datname=current_database()),current_schema()::regnamespace::oid::bigint").fetch_one(&mut *tx).await.map_err(db)?;
            if binding!=(self.database,self.schema){return Err(RecoveryError::Blocked);}
            op.check(&mut tx).await?;
            let populated:bool=sqlx::query_scalar(POPULATED).fetch_one(&mut *tx).await.map_err(db)?;
            if populated{return Err(RecoveryError::Blocked);}
            op.check(&mut tx).await?;
            sqlx::query("INSERT INTO service_recovery_control VALUES(true,'active',$1,$2,$3,$4) ON CONFLICT(singleton) DO NOTHING")
                .bind(self.database).bind(self.schema).bind(Uuid::new_v4()).bind(op.last).execute(&mut *tx).await.map_err(db)?;
            op.check(&mut tx).await?;
            sqlx::query("INSERT INTO service_recovery_origin(singleton,source_id) SELECT true,recovery_id FROM service_recovery_control WHERE singleton AND state='active' AND database_oid=$1 AND schema_oid=$2 ON CONFLICT(singleton) DO NOTHING")
                .bind(self.database).bind(self.schema).execute(&mut *tx).await.map_err(db)?;
            op.check(&mut tx).await?;tx.commit().await.map_err(db)?;Ok(())
        }).await
    }
}
pub async fn quarantine(pool: &PgPool, clock: &dyn AuthClock) -> Result<Uuid, RecoveryError> {
    bounded(async{
        let mut op=Operation::new(clock)?;let mut tx=pool.begin().await.map_err(db)?;op.own(&mut tx).await?;
        op.check(&mut tx).await?;
        let id:Uuid=sqlx::query_scalar("INSERT INTO service_recovery_control VALUES(true,'quarantined',(SELECT oid::bigint FROM pg_database WHERE datname=current_database()),current_schema()::regnamespace::oid::bigint,$1,$2) ON CONFLICT(singleton) DO UPDATE SET state='quarantined', database_oid=EXCLUDED.database_oid,schema_oid=EXCLUDED.schema_oid,recovery_id=CASE WHEN service_recovery_control.state='quarantined' AND service_recovery_control.database_oid=EXCLUDED.database_oid AND service_recovery_control.schema_oid=EXCLUDED.schema_oid THEN service_recovery_control.recovery_id ELSE EXCLUDED.recovery_id END,updated_at=EXCLUDED.updated_at RETURNING recovery_id")
            .bind(Uuid::new_v4()).bind(op.last).fetch_one(&mut *tx).await.map_err(db)?;
        op.check(&mut tx).await?;tx.commit().await.map_err(db)?;Ok(id)
    }).await
}
/// Read committed rights without acquiring the live server's exclusive journal lock.
pub async fn capture(pool: &PgPool, clock: &dyn AuthClock) -> Result<Snapshot, RecoveryError> {
    read_source(pool, clock, |snapshot, _| Ok(snapshot)).await
}
pub async fn compare_snapshot(
    pool: &PgPool,
    clock: &dyn AuthClock,
    checkpoint: &Snapshot,
) -> Result<SnapshotMatch, RecoveryError> {
    read_source(pool, clock, |observed, now| {
        checkpoint.compare(&observed, now)
    })
    .await
}
async fn read_source<T>(
    pool: &PgPool,
    clock: &dyn AuthClock,
    read: impl FnOnce(Snapshot, i64) -> Result<T, RecoveryError>,
) -> Result<T, RecoveryError> {
    bounded(async {
        let mut op = Operation::new(clock)?;
        let captured_at = op.last;
        let mut tx = pool.begin().await.map_err(db)?;
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
            .execute(&mut *tx).await.map_err(db)?;
        op.check(&mut tx).await?;
        let source_id: Option<Uuid> = sqlx::query_scalar("SELECT origin.source_id FROM service_recovery_origin origin CROSS JOIN service_recovery_control gate WHERE origin.singleton AND gate.singleton AND gate.state='active' AND gate.database_oid=(SELECT oid::bigint FROM pg_database WHERE datname=current_database()) AND gate.schema_oid=current_schema()::regnamespace::oid::bigint")
            .fetch_optional(&mut *tx).await.map_err(db)?;
        let source_id = source_id.ok_or(RecoveryError::Blocked)?;
        op.check(&mut tx).await?;
        let rows: Vec<(String, Uuid, i64, i64)> = sqlx::query_as(CAPTURE)
            .bind(captured_at).fetch_all(&mut *tx).await.map_err(db)?;
        let entries = rows.into_iter().map(|(kind,id,deleted_at,expires_at)| Erasure {
            kind: if kind == "account" { Kind::Account } else { Kind::Identity },
            id, deleted_at, expires_at,
        }).collect();
        let snapshot = Snapshot { version: 1, source_id, captured_at, entries };
        op.check(&mut tx).await?;
        snapshot.to_bytes(op.last)?;
        let result = read(snapshot, op.last)?;
        tx.commit().await.map_err(db)?;
        Ok(result)
    }).await
}
const CAPTURE: &str = "SELECT kind,id,CASE WHEN deleted=trunc(deleted) THEN deleted::bigint ELSE -1 END,CASE WHEN expires=trunc(expires) THEN expires::bigint ELSE -1 END FROM (SELECT 'account' AS kind,account_id AS id,extract(epoch FROM deleted_at) AS deleted,extract(epoch FROM expires_at) AS expires FROM auth_deletion_tombstones WHERE expires_at>to_timestamp($1::double precision) UNION ALL SELECT 'identity',identity_id,extract(epoch FROM deleted_at),extract(epoch FROM expires_at) FROM auth_identity_tombstones WHERE expires_at>to_timestamp($1::double precision)) entries ORDER BY kind,id LIMIT 501";

pub async fn apply(
    pool: &PgPool,
    clock: &dyn AuthClock,
    ledger: &Ledger,
) -> Result<u16, RecoveryError> {
    apply_input(pool, clock, Input::Ledger(ledger)).await
}
pub async fn apply_snapshot(
    pool: &PgPool,
    clock: &dyn AuthClock,
    incident: Uuid,
    snapshot: &Snapshot,
) -> Result<u16, RecoveryError> {
    apply_input(pool, clock, Input::Snapshot { incident, snapshot }).await
}
enum Input<'a> {
    Ledger(&'a Ledger),
    Snapshot {
        incident: Uuid,
        snapshot: &'a Snapshot,
    },
}
impl Input<'_> {
    fn validate(&self, now: i64) -> Result<(), RecoveryError> {
        match self {
            Self::Ledger(ledger) => ledger.validate(now),
            Self::Snapshot { incident, snapshot } => {
                if incident.is_nil() {
                    return Err(RecoveryError::Invalid);
                }
                snapshot.validate(now)
            }
        }
    }
    fn incident(&self) -> Uuid {
        match self {
            Self::Ledger(l) => l.recovery_id,
            Self::Snapshot { incident, .. } => *incident,
        }
    }
    fn entries(&self) -> &[Erasure] {
        match self {
            Self::Ledger(l) => &l.entries,
            Self::Snapshot { snapshot, .. } => &snapshot.entries,
        }
    }
}
async fn apply_input(
    pool: &PgPool,
    clock: &dyn AuthClock,
    input: Input<'_>,
) -> Result<u16, RecoveryError> {
    bounded(async{
        let mut op=Operation::new(clock)?;input.validate(op.last)?;
        let mut tx=pool.begin().await.map_err(db)?;op.own(&mut tx).await?;
        op.check(&mut tx).await?;
        let id:Option<Uuid>=sqlx::query_scalar("SELECT recovery_id FROM service_recovery_control WHERE singleton AND state='quarantined' AND database_oid=(SELECT oid::bigint FROM pg_database WHERE datname=current_database()) AND schema_oid=current_schema()::regnamespace::oid::bigint FOR UPDATE")
            .fetch_optional(&mut *tx).await.map_err(db)?;
        if id!=Some(input.incident()){return Err(RecoveryError::Blocked);}
        if let Input::Snapshot { snapshot, .. } = &input {
            op.check(&mut tx).await?;
            let source: Option<Uuid> = sqlx::query_scalar("SELECT source_id FROM service_recovery_origin WHERE singleton FOR SHARE")
                .fetch_optional(&mut *tx).await.map_err(db)?;
            if source != Some(snapshot.source_id) { return Err(RecoveryError::Blocked); }
        }
        let mut entries=input.entries().iter().filter(|e| e.expires_at>op.last).collect::<Vec<_>>();
        entries.sort_by_key(|e|e.id);
        for entry in &entries{
            let (record,remove)=match entry.kind{
                Kind::Account=>("INSERT INTO auth_deletion_tombstones(account_id,deleted_at,expires_at) VALUES($1,to_timestamp($2::double precision),to_timestamp($3::double precision)) ON CONFLICT(account_id) DO NOTHING","DELETE FROM auth_accounts WHERE id=$1"),
                Kind::Identity=>("INSERT INTO auth_identity_tombstones(identity_id,deleted_at,expires_at) VALUES($1,to_timestamp($2::double precision),to_timestamp($3::double precision)) ON CONFLICT(identity_id) DO NOTHING","DELETE FROM auth_identities WHERE id=$1"),
            };
            op.check(&mut tx).await?;
            sqlx::query(record).bind(entry.id).bind(entry.deleted_at).bind(entry.expires_at).execute(&mut *tx).await.map_err(db)?;
            op.check(&mut tx).await?;sqlx::query(remove).bind(entry.id).execute(&mut *tx).await.map_err(db)?;
        }
        for query in ["DELETE FROM auth_sessions","DELETE FROM auth_transactions","DELETE FROM auth_credentials","DELETE FROM auth_apple_revoke_queue","DELETE FROM auth_apple_notification_receipts"]{
            op.check(&mut tx).await?;sqlx::query(query).execute(&mut *tx).await.map_err(db)?;
        }
        for query in ["DELETE FROM auth_deletion_tombstones WHERE expires_at<=to_timestamp($1::double precision)","DELETE FROM auth_identity_tombstones WHERE expires_at<=to_timestamp($1::double precision)"]{
            op.check(&mut tx).await?;sqlx::query(query).bind(op.last).execute(&mut *tx).await.map_err(db)?;
        }
        op.check(&mut tx).await?;
        sqlx::query("INSERT INTO metric_collection(singleton,started_at,coverage_since) VALUES(true,$1,$1) ON CONFLICT(singleton) DO UPDATE SET coverage_since=GREATEST(metric_collection.coverage_since,EXCLUDED.coverage_since)")
            .bind(op.last).execute(&mut *tx).await.map_err(db)?;
        op.check(&mut tx).await?;input.validate(op.last)?;
        if entries.iter().any(|e| e.expires_at<=op.last) { return Err(RecoveryError::Invalid); }
        tx.commit().await.map_err(db)?;
        Ok(entries.len() as u16)
    }).await
}
const POPULATED: &str = "SELECT EXISTS(SELECT 1 FROM service_recovery_origin UNION ALL SELECT 1 FROM auth_accounts UNION ALL SELECT 1 FROM auth_identities UNION ALL SELECT 1 FROM auth_credentials UNION ALL SELECT 1 FROM auth_sessions UNION ALL SELECT 1 FROM auth_transactions UNION ALL SELECT 1 FROM auth_deletion_tombstones UNION ALL SELECT 1 FROM auth_identity_tombstones UNION ALL SELECT 1 FROM auth_apple_revoke_queue UNION ALL SELECT 1 FROM auth_apple_notification_receipts UNION ALL SELECT 1 FROM online_match_results UNION ALL SELECT 1 FROM online_match_players UNION ALL SELECT 1 FROM online_active_matches UNION ALL SELECT 1 FROM online_active_players UNION ALL SELECT 1 FROM daily_definitions UNION ALL SELECT 1 FROM daily_completions UNION ALL SELECT 1 FROM product_events UNION ALL SELECT 1 FROM optional_product_events UNION ALL SELECT 1 FROM session_consents UNION ALL SELECT 1 FROM metric_collection UNION ALL SELECT 1 FROM product_metric_aggregates)";
