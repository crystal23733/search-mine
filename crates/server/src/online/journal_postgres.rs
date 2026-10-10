use super::postgres::{outcome, reason, validate_duplicate};
use super::{ActiveMatch, FinishedMatch, SaveResult};
use liar_protocol::online::OnlineError;
use sqlx::{Connection, PgConnection};
use uuid::Uuid;

pub(super) fn validate_active(active: &ActiveMatch, now: i64) -> Result<(), OnlineError> {
    if active.id.is_nil()
        || active.rules_hash.len() != 64
        || !active
            .rules_hash
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        || !(0..=253402300799 - 604800).contains(&now)
        || active.players.iter().all(Option::is_none)
        || active.players.iter().flatten().any(Uuid::is_nil)
        || (active.players[0].is_some() && active.players[0] == active.players[1])
    {
        return Err(OnlineError::Malformed);
    }
    Ok(())
}
async fn final_exists(connection: &mut PgConnection, id: Uuid) -> Result<bool, OnlineError> {
    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM online_match_results WHERE id=$1)")
        .bind(id)
        .fetch_one(connection)
        .await
        .map_err(db_error)
}
async fn account_exists(connection: &mut PgConnection, account: Uuid) -> Result<bool, OnlineError> {
    Ok(
        sqlx::query_scalar::<_, Uuid>("SELECT id FROM auth_accounts WHERE id=$1 FOR KEY SHARE")
            .bind(account)
            .fetch_optional(connection)
            .await
            .map_err(db_error)?
            .is_some(),
    )
}
pub(super) async fn register(
    connection: &mut PgConnection,
    owner: Uuid,
    active: ActiveMatch,
    now: i64,
) -> Result<SaveResult, OnlineError> {
    let mut tx = connection.begin().await.map_err(db_error)?;
    if final_exists(&mut tx, active.id).await? {
        return Err(OnlineError::Malformed);
    }
    for account in active.players.iter().flatten() {
        if !account_exists(&mut tx, *account).await? {
            return Err(OnlineError::Unauthorized);
        }
    }
    let existing:Option<(String,Uuid,i64)>=sqlx::query_as("SELECT rules_hash,owner_id,EXTRACT(EPOCH FROM admitted_at)::bigint FROM online_active_matches WHERE id=$1 FOR UPDATE")
        .bind(active.id).fetch_optional(&mut *tx).await.map_err(db_error)?;
    if let Some((hash, original, at)) = existing {
        let players = survivors(&mut tx, active.id).await?;
        if hash != active.rules_hash
            || original != owner
            || at > now
            || players.len() != 2
            || players.iter().any(|(seat, account, bot)| {
                *account != active.players[*seat as usize] || *bot != account.is_none()
            })
        {
            return Err(OnlineError::Malformed);
        }
        tx.commit().await.map_err(db_error)?;
        return Ok(SaveResult::Duplicate);
    }
    sqlx::query("INSERT INTO online_active_matches(id,rules_hash,owner_id,admitted_at) VALUES($1,$2,$3,to_timestamp($4::double precision))")
        .bind(active.id).bind(active.rules_hash).bind(owner).bind(now).execute(&mut *tx).await.map_err(db_error)?;
    for (seat, account) in active.players.iter().enumerate() {
        sqlx::query("INSERT INTO online_active_players(match_id,seat,account_id,is_bot) VALUES($1,$2,$3,$4)")
            .bind(active.id).bind(seat as i16).bind(account).bind(account.is_none()).execute(&mut *tx).await.map_err(db_error)?;
    }
    tx.commit().await.map_err(db_error)?;
    Ok(SaveResult::Saved)
}
pub(super) async fn discard(
    connection: &mut PgConnection,
    owner: Uuid,
    id: Uuid,
) -> Result<SaveResult, OnlineError> {
    let mut tx = connection.begin().await.map_err(db_error)?;
    let existing: Option<Uuid> =
        sqlx::query_scalar("SELECT owner_id FROM online_active_matches WHERE id=$1 FOR UPDATE")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(db_error)?;
    if existing.is_some_and(|original| original != owner)
        || (existing.is_some() && final_exists(&mut tx, id).await?)
    {
        return Err(OnlineError::Malformed);
    }
    delete_active(&mut tx, id).await?;
    tx.commit().await.map_err(db_error)?;
    Ok(if existing.is_some() {
        SaveResult::Saved
    } else {
        SaveResult::Duplicate
    })
}

type Survivor = (i16, Option<Uuid>, bool);
async fn survivors(connection: &mut PgConnection, id: Uuid) -> Result<Vec<Survivor>, OnlineError> {
    // Lock accounts before participant rows so account erasure cannot deadlock with a final.
    let accounts:Vec<Uuid>=sqlx::query_scalar("SELECT account_id FROM online_active_players WHERE match_id=$1 AND account_id IS NOT NULL ORDER BY account_id")
        .bind(id).fetch_all(&mut *connection).await.map_err(db_error)?;
    for account in accounts {
        let _ = account_exists(connection, account).await?;
    }
    let rows:Vec<Survivor>=sqlx::query_as("SELECT seat,account_id,is_bot FROM online_active_players WHERE match_id=$1 ORDER BY seat FOR UPDATE")
        .bind(id).fetch_all(connection).await.map_err(db_error)?;
    if rows.len() > 2
        || rows
            .iter()
            .any(|(seat, account, bot)| !(0..=1).contains(seat) || *bot != account.is_none())
    {
        return Err(OnlineError::Malformed);
    }
    Ok(rows)
}
async fn delete_active(connection: &mut PgConnection, id: Uuid) -> Result<(), OnlineError> {
    sqlx::query("DELETE FROM online_active_matches WHERE id=$1")
        .bind(id)
        .execute(connection)
        .await
        .map_err(db_error)?;
    Ok(())
}
pub(super) async fn finish(
    connection: &mut PgConnection,
    owner: Uuid,
    result: FinishedMatch,
    now: i64,
) -> Result<SaveResult, OnlineError> {
    let mut tx = connection.begin().await.map_err(db_error)?;
    let active:Option<(String,Uuid,i64)>=sqlx::query_as("SELECT rules_hash,owner_id,EXTRACT(EPOCH FROM admitted_at)::bigint FROM online_active_matches WHERE id=$1 FOR UPDATE")
        .bind(result.id).fetch_optional(&mut *tx).await.map_err(db_error)?;
    if final_exists(&mut tx, result.id).await? {
        if active.is_some() {
            return Err(OnlineError::Malformed);
        }
        validate_duplicate(&mut tx, &result).await?;
        tx.commit().await.map_err(db_error)?;
        return Ok(SaveResult::Duplicate);
    }
    let Some((hash, original, at)) = active else {
        return Err(OnlineError::Malformed);
    };
    if hash != result.rules_hash || original != owner || at > now {
        return Err(OnlineError::Malformed);
    }
    let players = survivors(&mut tx, result.id).await?;
    if players
        .iter()
        .any(|(seat, account, _)| result.players[*seat as usize].account != *account)
    {
        return Err(OnlineError::Malformed);
    }
    let seed = (now < at + 604800).then(|| result.seed.to_vec());
    sqlx::query("INSERT INTO online_match_results(id,rules_hash,secret_seed,end_elapsed_ms,reason,recorded_at,seed_expires_at,retention_started_at) VALUES($1,$2,$3,$4,$5,to_timestamp($6::double precision),to_timestamp($7::double precision),to_timestamp($8::double precision))")
        .bind(result.id).bind(result.rules_hash).bind(seed).bind(result.ended_ms as i64).bind(reason(result.reason)).bind(now).bind(at+604800).bind(at)
        .execute(&mut *tx).await.map_err(db_error)?;
    for (seat, account, bot) in players {
        let player = &result.players[seat as usize];
        sqlx::query("INSERT INTO online_match_players(match_id,seat,account_id,is_bot,outcome,opened_safe,mistakes,accusations,correct_accusations) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9)")
            .bind(result.id).bind(seat).bind(account).bind(bot).bind(outcome(player.outcome)).bind(player.opened_safe as i16)
            .bind(player.mistakes as i16).bind(player.accusations as i16).bind(player.correct_accusations as i16)
            .execute(&mut *tx).await.map_err(db_error)?;
    }
    delete_active(&mut tx, result.id).await?;
    tx.commit().await.map_err(db_error)?;
    Ok(SaveResult::Saved)
}
pub(super) async fn recover(connection: &mut PgConnection, now: i64) -> Result<(), OnlineError> {
    if !(0..=253402300799).contains(&now) {
        return Err(OnlineError::Malformed);
    }
    let bounded: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM online_active_matches LIMIT 10001")
        .fetch_all(&mut *connection)
        .await
        .map_err(db_error)?;
    if bounded.len() > 10000 {
        return Err(OnlineError::Malformed);
    }
    loop {
        let count = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            recover_batch(connection, now),
        )
        .await
        .map_err(|_| OnlineError::Unavailable)??;
        if count == 0 {
            return Ok(());
        }
    }
}
async fn recover_batch(connection: &mut PgConnection, now: i64) -> Result<usize, OnlineError> {
    let mut tx = connection.begin().await.map_err(db_error)?;
    let active:Vec<(Uuid,String,i64)>=sqlx::query_as("SELECT id,rules_hash,EXTRACT(EPOCH FROM admitted_at)::bigint FROM online_active_matches ORDER BY admitted_at,id LIMIT 64 FOR UPDATE")
        .fetch_all(&mut *tx).await.map_err(db_error)?;
    for (id, hash, at) in &active {
        if *at > now || final_exists(&mut tx, *id).await? {
            return Err(OnlineError::Malformed);
        }
        let players = survivors(&mut tx, *id).await?;
        sqlx::query("INSERT INTO online_match_results(id,rules_hash,secret_seed,end_elapsed_ms,reason,recorded_at,seed_expires_at,retention_started_at) VALUES($1,$2,NULL,NULL,'server_failure',to_timestamp($3::double precision),to_timestamp($4::double precision),to_timestamp($5::double precision))")
            .bind(id).bind(hash).bind(now).bind(at+604800).bind(at).execute(&mut *tx).await.map_err(db_error)?;
        for (seat, account, bot) in players {
            sqlx::query("INSERT INTO online_match_players(match_id,seat,account_id,is_bot,outcome,opened_safe,mistakes,accusations,correct_accusations) VALUES($1,$2,$3,$4,'abort',NULL,NULL,NULL,NULL)")
                .bind(id).bind(seat).bind(account).bind(bot).execute(&mut *tx).await.map_err(db_error)?;
        }
        delete_active(&mut tx, *id).await?;
    }
    tx.commit().await.map_err(db_error)?;
    Ok(active.len())
}
fn db_error(_: sqlx::Error) -> OnlineError {
    OnlineError::Unavailable
}
