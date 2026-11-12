//! Controlled cutover: fence the authoritative source before touching the destination.
use super::postgres::snapshot_in;
use super::{RecoveryError, Snapshot, apply_snapshot};
use crate::{auth::AuthClock, events::*, online::recovery_lock};
use sqlx::{PgPool, Postgres, Transaction};
use std::{
    sync::{
        Arc,
        atomic::{AtomicI64, Ordering},
    },
    time::Duration,
};
use uuid::Uuid;
struct ForwardClock {
    inner: Arc<dyn AuthClock>,
    last: AtomicI64,
}
impl AuthClock for ForwardClock {
    fn now(&self) -> i64 {
        let now = self.inner.now();
        let previous = self.last.fetch_max(now, Ordering::SeqCst);
        if now < previous || now < 0 {
            self.last.store(i64::MAX, Ordering::SeqCst);
            -1
        } else {
            now
        }
    }
}
const STEP: Duration = Duration::from_secs(2);
fn unavailable(_: sqlx::Error) -> RecoveryError {
    RecoveryError::Unavailable
}
async fn policy(tx: &mut Transaction<'_, Postgres>) -> Result<(), RecoveryError> {
    sqlx::query("SELECT set_config('statement_timeout','1800ms',true),set_config('lock_timeout','1800ms',true)")
        .execute(&mut **tx).await.map_err(unavailable)?;
    Ok(())
}
async fn target_gate(
    tx: &mut Transaction<'_, Postgres>,
    incident: Uuid,
    source: Uuid,
) -> Result<(), RecoveryError> {
    if !recovery_lock(tx)
        .await
        .map_err(|_| RecoveryError::Unavailable)?
    {
        return Err(RecoveryError::Busy);
    }
    let valid: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM service_recovery_control g CROSS JOIN service_recovery_origin o WHERE g.singleton AND o.singleton AND g.state='quarantined' AND g.recovery_id=$1 AND o.source_id=$2 AND g.database_oid=(SELECT oid::bigint FROM pg_database WHERE datname=current_database()) AND g.schema_oid=current_schema()::regnamespace::oid::bigint)")
        .bind(incident).bind(source).fetch_one(&mut **tx).await.map_err(unavailable)?;
    if !valid {
        return Err(RecoveryError::Blocked);
    }
    Ok(())
}
async fn fence(
    pool: &PgPool,
    clock: &dyn AuthClock,
    checkpoint: &Snapshot,
) -> Result<(), RecoveryError> {
    tokio::time::timeout(STEP,async {
        checkpoint.validate(clock.now())?;
        let mut tx=pool.begin().await.map_err(unavailable)?;
        policy(&mut tx).await?;
        if !recovery_lock(&mut tx).await.map_err(|_| RecoveryError::Unavailable)? {return Err(RecoveryError::Busy);}
        // Wait for committed rights writers and prevent new ones until the sealed trigger is visible.
        sqlx::query("LOCK TABLE auth_accounts,auth_identities,auth_deletion_tombstones,auth_identity_tombstones IN SHARE MODE")
            .execute(&mut *tx).await.map_err(unavailable)?;
        let source: Option<Uuid> = sqlx::query_scalar("SELECT o.source_id FROM service_recovery_origin o CROSS JOIN service_recovery_control g WHERE o.singleton AND g.singleton AND g.state IN ('active','sealed') AND g.database_oid=(SELECT oid::bigint FROM pg_database WHERE datname=current_database()) AND g.schema_oid=current_schema()::regnamespace::oid::bigint FOR UPDATE OF g")
            .fetch_optional(&mut *tx).await.map_err(unavailable)?;
        let source=source.ok_or(RecoveryError::Blocked)?;
        let at=clock.now();
        let current=snapshot_in(&mut tx,source,at).await?;
        checkpoint.compare(&current,clock.now())?;
        sqlx::query("UPDATE service_recovery_control SET state='sealed',updated_at=$1 WHERE singleton AND state='active'")
            .bind(at).execute(&mut *tx).await.map_err(unavailable)?;
        checkpoint.validate(clock.now())?;
        tx.commit().await.map_err(unavailable)?;
        Ok(())
    }).await.unwrap_or(Err(RecoveryError::Unavailable))
}
/// Requires a live original source; an old checkpoint alone can never authorize activation.
pub async fn resume_from_source(
    source: &PgPool,
    target: &PgPool,
    clock: Arc<dyn AuthClock>,
    incident: Uuid,
    checkpoint: &Snapshot,
) -> Result<u16, RecoveryError> {
    let clock: Arc<dyn AuthClock> = Arc::new(ForwardClock {
        inner: clock,
        last: AtomicI64::new(0),
    });
    if incident.is_nil() {
        return Err(RecoveryError::Invalid);
    }
    checkpoint.validate(clock.now())?;
    // Validate the destination before taking down a healthy source on an operator typo.
    tokio::time::timeout(STEP, async {
        let mut tx = target.begin().await.map_err(unavailable)?;
        policy(&mut tx).await?;
        target_gate(&mut tx, incident, checkpoint.source_id).await?;
        tx.rollback().await.map_err(unavailable)
    })
    .await
    .unwrap_or(Err(RecoveryError::Unavailable))?;
    fence(source, clock.as_ref(), checkpoint).await?;
    let applied = apply_snapshot(target, clock.as_ref(), incident, checkpoint).await?;
    let reapers: [Box<dyn EventRetentionStore>; 4] = [
        Box::new(PgEventRetention::new(target.clone(), clock.clone())),
        Box::new(PgOptionalEventRetention::new(target.clone(), clock.clone())),
        Box::new(PgAggregateRetention::new(target.clone(), clock.clone())),
        Box::new(PgSourceRetention::new(target.clone(), clock.clone())),
    ];
    tokio::time::timeout(Duration::from_secs(60), async {
        for reaper in reapers {
            loop {
                let batch = reaper
                    .reap(STEP)
                    .await
                    .map_err(|_| RecoveryError::Unavailable)?;
                if !batch.has_more {
                    break;
                }
                // Locked expired rows must block release instead of being skipped indefinitely.
                if batch.deleted == 0 {
                    return Err(RecoveryError::Busy);
                }
            }
        }
        Ok(())
    })
    .await
    .unwrap_or(Err(RecoveryError::Unavailable))?;
    tokio::time::timeout(STEP,async {
        checkpoint.validate(clock.now())?;
        let mut tx=target.begin().await.map_err(unavailable)?;policy(&mut tx).await?;
        target_gate(&mut tx,incident,checkpoint.source_id).await?;
        sqlx::query("UPDATE service_recovery_control SET state='active',updated_at=$1 WHERE singleton AND recovery_id=$2")
            .bind(clock.now()).bind(incident).execute(&mut *tx).await.map_err(unavailable)?;
        tx.commit().await.map_err(unavailable)?;Ok(applied)
    }).await.unwrap_or(Err(RecoveryError::Unavailable))
}
