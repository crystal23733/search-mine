use super::{
    EventId, EventSource, EventStoreError,
    postgres::{db, policy},
};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

fn id(value: Uuid) -> Result<EventId, EventStoreError> {
    EventId::parse(&value.to_string()).map_err(policy)
}
// Callers already hold account locks in UUID order, before participant/source locks.
async fn collect(
    tx: &mut Transaction<'_, Postgres>,
    source: EventSource,
    now: i64,
) -> Result<(), EventStoreError> {
    super::valid_timestamp(now).map_err(policy)?;
    let event = match super::pg_source::load(tx, id(Uuid::new_v4())?, source, now).await {
        Ok(event) => event,
        // Recovery can close a journal whose original game retention has expired.
        Err(EventStoreError::Expired) => return Ok(()),
        Err(error) => return Err(error),
    };
    super::pg_records::insert(tx, event, now).await?;
    Ok(())
}
pub(crate) async fn collect_online(
    tx: &mut Transaction<'_, Postgres>,
    match_id: Uuid,
    now: i64,
) -> Result<(), EventStoreError> {
    let accounts: Vec<Uuid> = sqlx::query_scalar("SELECT account_id FROM online_match_players WHERE match_id=$1 AND NOT is_bot ORDER BY account_id LIMIT 3")
        .bind(match_id).fetch_all(&mut **tx).await.map_err(db)?;
    if accounts.len() > 2 {
        return Err(EventStoreError::Invalid);
    }
    for account in accounts {
        collect(
            tx,
            EventSource::Online {
                account: id(account)?,
                match_id: id(match_id)?,
            },
            now,
        )
        .await?;
    }
    Ok(())
}
pub(crate) async fn collect_daily(
    tx: &mut Transaction<'_, Postgres>,
    account: Uuid,
    completion: Uuid,
    now: i64,
) -> Result<(), EventStoreError> {
    collect(
        tx,
        EventSource::Daily {
            account: id(account)?,
            completion_id: id(completion)?,
        },
        now,
    )
    .await
}
