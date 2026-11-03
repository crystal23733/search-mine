use super::{
    optional_postgres::{db, policy},
    *,
};
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

pub(super) async fn insert(
    tx: &mut Transaction<'_, Postgres>,
    input: &OptionalCapture,
    event: ProductEvent,
    received_at: i64,
) -> Result<StoredEvent, OptionalEventError> {
    let purpose = match event.purpose() {
        Purpose::Analytics => "analytics",
        Purpose::Advertising => "advertising",
        Purpose::Essential => return Err(OptionalEventError::Invalid),
    };
    let action =
        serde_json::to_value(&input.draft.event).map_err(|_| OptionalEventError::Invalid)?;
    let inserted = sqlx::query("INSERT INTO optional_product_events(id,owner_session_id,tab_key,consent_revision,purpose,occurred_at,received_at,locale,action) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9::text::jsonb) ON CONFLICT DO NOTHING")
        .bind(event.id().uuid())
        .bind(input.session.revision)
        .bind(input.draft.session_key.uuid())
        .bind(input.expected_revision)
        .bind(purpose)
        .bind(event.occurred_at())
        .bind(received_at)
        .bind(&input.draft.locale)
        .bind(action.to_string())
        .execute(&mut **tx)
        .await
        .map_err(db)?
        .rows_affected() == 1;
    let row = sqlx::query("SELECT owner_session_id,tab_key,consent_revision,purpose,occurred_at,received_at,locale,action::text AS action FROM optional_product_events WHERE id=$1")
        .bind(event.id().uuid())
        .fetch_one(&mut **tx)
        .await
        .map_err(db)?;
    if row.try_get::<Uuid, _>("owner_session_id").map_err(db)? != input.session.revision
        || row.try_get::<Uuid, _>("tab_key").map_err(db)? != input.draft.session_key.uuid()
        || row.try_get::<String, _>("purpose").map_err(db)? != purpose
        || row.try_get::<i64, _>("occurred_at").map_err(db)? != event.occurred_at()
        || row.try_get::<String, _>("locale").map_err(db)? != input.draft.locale
        || serde_json::from_str::<serde_json::Value>(
            &row.try_get::<String, _>("action").map_err(db)?,
        )
        .map_err(|_| OptionalEventError::Unavailable)?
            != action
    {
        return Err(OptionalEventError::Conflict);
    }
    let original: i64 = row.try_get("received_at").map_err(db)?;
    if row
        .try_get::<Uuid, _>("consent_revision")
        .map_err(db)?
        .is_nil()
    {
        return Err(OptionalEventError::Unavailable);
    }
    raw_event_alive(event.occurred_at(), original).map_err(|_| OptionalEventError::Unavailable)?;
    if original > received_at {
        return Err(OptionalEventError::Invalid);
    }
    raw_event_alive(event.occurred_at(), received_at).map_err(policy)?;
    Ok(StoredEvent {
        event,
        received_at: original,
        inserted,
    })
}
