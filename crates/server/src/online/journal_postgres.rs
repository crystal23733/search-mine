use super::postgres::{outcome, reason, validate_duplicate};
use super::{ActiveMatch, FinishedMatch, SaveResult};
use liar_protocol::online::OnlineError;
use sqlx::{Connection, PgConnection};
use uuid::Uuid;

pub(super) fn validate_active(active: &ActiveMatch, now: i64) -> Result<(), OnlineError> {
    if !valid_header(active.id, &active.rules_hash, now)
        || active.players.iter().all(Option::is_none)
        || active.players.iter().flatten().any(Uuid::is_nil)
        || (active.players[0].is_some() && active.players[0] == active.players[1])
    {
        return Err(OnlineError::Malformed);
    }
    Ok(())
}

pub(super) fn valid_header(id: Uuid, hash: &str, now: i64) -> bool {
    !id.is_nil()
        && hash.len() == 64
        && hash
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        && valid_start_time(now)
}

pub(super) fn valid_start_time(now: i64) -> bool {
    (0..=253402300799 - 604800).contains(&now)
}

pub(super) async fn final_exists(
    connection: &mut PgConnection,
    id: Uuid,
) -> Result<bool, OnlineError> {
    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM online_match_results WHERE id=$1)")
        .bind(id)
        .fetch_one(connection)
        .await
        .map_err(db_error)
}
async fn account_exists(connection: &mut PgConnection, account: Uuid) -> Result<bool, OnlineError> {
    Ok(
        sqlx::query_scalar::<_, Uuid>("SELECT id FROM auth_accounts WHERE id=$1 FOR UPDATE")
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
    register_roster(
        connection,
        owner,
        active.id,
        &active.rules_hash,
        None,
        &active.players,
        now,
    )
    .await
}

pub(super) async fn register_roster(
    connection: &mut PgConnection,
    owner: Uuid,
    id: Uuid,
    rules_hash: &str,
    room: Option<Uuid>,
    roster: &[Option<Uuid>],
    now: i64,
) -> Result<SaveResult, OnlineError> {
    let mut tx = connection.begin().await.map_err(db_error)?;
    if final_exists(&mut tx, id).await? {
        return Err(OnlineError::Malformed);
    }
    let mut accounts: Vec<Uuid> = roster.iter().flatten().copied().collect();
    accounts.sort_unstable();
    for account in accounts {
        if !account_exists(&mut tx, account).await? {
            return Err(OnlineError::Unauthorized);
        }
    }
    let count = roster.len() as i16;
    if let Some(existing) = active_header(&mut tx, id).await? {
        if existing.hash != rules_hash
            || existing.owner != owner
            || existing.admitted_at > now
            || existing.room != room
            || existing.count != count
        {
            return Err(OnlineError::Malformed);
        }
        let players = survivors(&mut tx, id, room, count).await?;
        if players.len() != roster.len()
            || players
                .iter()
                .any(|(seat, account, _)| *account != roster[*seat as usize])
        {
            return Err(OnlineError::Malformed);
        }
        tx.commit().await.map_err(db_error)?;
        return Ok(SaveResult::Duplicate);
    }
    sqlx::query("INSERT INTO online_active_matches(id,rules_hash,owner_id,admitted_at,room_id,participant_count) VALUES($1,$2,$3,to_timestamp($4::double precision),$5,$6)")
        .bind(id).bind(rules_hash).bind(owner).bind(now).bind(room).bind(count).execute(&mut *tx).await.map_err(db_error)?;
    for (seat, account) in roster.iter().enumerate() {
        sqlx::query("INSERT INTO online_active_players(match_id,seat,account_id,is_bot,participant_count) VALUES($1,$2,$3,$4,$5)")
            .bind(id).bind(seat as i16).bind(account).bind(account.is_none()).bind(count).execute(&mut *tx).await.map_err(db_error)?;
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

pub(super) struct ActiveHeader {
    pub(super) hash: String,
    pub(super) owner: Uuid,
    pub(super) admitted_at: i64,
    pub(super) started_at: Option<i64>,
    pub(super) room: Option<Uuid>,
    pub(super) count: i16,
}

type ActiveHeaderRow = (String, Uuid, i64, Option<i64>, Option<Uuid>, i16);

pub(super) async fn active_header(
    connection: &mut PgConnection,
    id: Uuid,
) -> Result<Option<ActiveHeader>, OnlineError> {
    let row: Option<ActiveHeaderRow> = sqlx::query_as("SELECT rules_hash,owner_id,EXTRACT(EPOCH FROM admitted_at)::bigint,EXTRACT(EPOCH FROM started_at)::bigint,room_id,participant_count FROM online_active_matches WHERE id=$1 FOR UPDATE")
        .bind(id).fetch_optional(connection).await.map_err(db_error)?;
    Ok(row.map(
        |(hash, owner, admitted_at, started_at, room, count)| ActiveHeader {
            hash,
            owner,
            admitted_at,
            started_at,
            room,
            count,
        },
    ))
}

type Survivor = (i16, Option<Uuid>, bool);
pub(super) async fn survivors(
    connection: &mut PgConnection,
    id: Uuid,
    room: Option<Uuid>,
    count: i16,
) -> Result<Vec<Survivor>, OnlineError> {
    // Lock accounts before participant rows so account erasure cannot deadlock with a final.
    let accounts:Vec<Uuid>=sqlx::query_scalar("SELECT account_id FROM online_active_players WHERE match_id=$1 AND account_id IS NOT NULL ORDER BY account_id")
        .bind(id).fetch_all(&mut *connection).await.map_err(db_error)?;
    for account in accounts {
        let _ = account_exists(connection, account).await?;
    }
    let rows:Vec<Survivor>=sqlx::query_as("SELECT seat,account_id,is_bot FROM online_active_players WHERE match_id=$1 ORDER BY seat FOR UPDATE")
        .bind(id).fetch_all(connection).await.map_err(db_error)?;
    if rows.len() > count as usize
        || rows.iter().any(|(seat, account, bot)| {
            !(0..count).contains(seat) || *bot != account.is_none() || (room.is_some() && *bot)
        })
    {
        return Err(OnlineError::Malformed);
    }
    Ok(rows)
}
pub(super) async fn delete_active(
    connection: &mut PgConnection,
    id: Uuid,
) -> Result<(), OnlineError> {
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
    let active = active_header(&mut tx, result.id).await?;
    if final_exists(&mut tx, result.id).await? {
        if active.is_some() {
            return Err(OnlineError::Malformed);
        }
        validate_duplicate(&mut tx, &result).await?;
        tx.commit().await.map_err(db_error)?;
        return Ok(SaveResult::Duplicate);
    }
    let Some(active) = active else {
        return Err(OnlineError::Malformed);
    };
    let at = active.admitted_at;
    if active.hash != result.rules_hash
        || active.owner != owner
        || at > now
        || active.room.is_some()
    {
        return Err(OnlineError::Malformed);
    }
    let players = survivors(&mut tx, result.id, None, active.count).await?;
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
    crate::events::collect_online(&mut tx, result.id, now)
        .await
        .map_err(|_| OnlineError::Unavailable)?;
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
    let active:Vec<(Uuid,String,i64,Option<Uuid>,i16)>=sqlx::query_as("SELECT id,rules_hash,EXTRACT(EPOCH FROM COALESCE(started_at,admitted_at))::bigint,room_id,participant_count FROM online_active_matches ORDER BY admitted_at,id LIMIT 64 FOR UPDATE")
        .fetch_all(&mut *tx).await.map_err(db_error)?;
    let ids: Vec<Uuid> = active.iter().map(|(id, ..)| *id).collect();
    let accounts: Vec<Uuid> = sqlx::query_scalar("SELECT DISTINCT account_id FROM online_active_players WHERE match_id=ANY($1) AND account_id IS NOT NULL ORDER BY account_id")
        .bind(&ids).fetch_all(&mut *tx).await.map_err(db_error)?;
    for account in accounts {
        let _ = account_exists(&mut tx, account).await?;
    }
    for (id, hash, at, room, count) in &active {
        if *at > now || final_exists(&mut tx, *id).await? {
            return Err(OnlineError::Malformed);
        }
        let players = survivors(&mut tx, *id, *room, *count).await?;
        sqlx::query("INSERT INTO online_match_results(id,rules_hash,secret_seed,end_elapsed_ms,reason,recorded_at,seed_expires_at,retention_started_at,room_id,participant_count) VALUES($1,$2,NULL,NULL,'server_failure',to_timestamp($3::double precision),to_timestamp($4::double precision),to_timestamp($5::double precision),$6,$7)")
            .bind(id).bind(hash).bind(now).bind(at+604800).bind(at).bind(room).bind(count).execute(&mut *tx).await.map_err(db_error)?;
        for (seat, account, bot) in players {
            sqlx::query("INSERT INTO online_match_players(match_id,seat,account_id,is_bot,outcome,opened_safe,mistakes,accusations,correct_accusations,participant_count) VALUES($1,$2,$3,$4,'abort',NULL,NULL,NULL,NULL,$5)")
                .bind(id).bind(seat).bind(account).bind(bot).bind(count).execute(&mut *tx).await.map_err(db_error)?;
        }
        crate::events::collect_online(&mut tx, *id, now)
            .await
            .map_err(|_| OnlineError::Unavailable)?;
        delete_active(&mut tx, *id).await?;
    }
    tx.commit().await.map_err(db_error)?;
    Ok(active.len())
}
pub(super) fn db_error(_: sqlx::Error) -> OnlineError {
    OnlineError::Unavailable
}
