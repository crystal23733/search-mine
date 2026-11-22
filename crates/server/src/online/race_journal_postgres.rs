use super::journal_postgres::{
    active_header, db_error, delete_active, final_exists, register_roster, survivors, valid_header,
};
use super::postgres::{outcome, reason};
use super::{ActiveRace, FinishedRace, SaveResult};
use liar_core::game::multiplayer::{MAX_RACE_PLAYERS, MIN_RACE_PLAYERS};
use liar_protocol::{
    game::{Outcome, PublicEndReason},
    online::OnlineError,
};
use sqlx::{Connection, PgConnection};
use uuid::Uuid;

fn valid_accounts(accounts: &[Uuid]) -> bool {
    (MIN_RACE_PLAYERS..=MAX_RACE_PLAYERS).contains(&accounts.len())
        && accounts
            .iter()
            .enumerate()
            .all(|(seat, account)| !account.is_nil() && !accounts[..seat].contains(account))
}

pub(super) fn validate_active(race: &ActiveRace, now: i64) -> Result<(), OnlineError> {
    if race.room_id.is_nil()
        || !valid_header(race.id, &race.rules_hash, now)
        || !valid_accounts(&race.accounts)
    {
        return Err(OnlineError::Malformed);
    }
    Ok(())
}

pub(super) async fn register(
    connection: &mut PgConnection,
    owner: Uuid,
    race: ActiveRace,
    now: i64,
) -> Result<SaveResult, OnlineError> {
    let accounts: Vec<_> = race.accounts.into_iter().map(Some).collect();
    register_roster(
        connection,
        owner,
        race.id,
        &race.rules_hash,
        Some(race.room_id),
        &accounts,
        now,
    )
    .await
}

pub(super) async fn start(
    connection: &mut PgConnection,
    owner: Uuid,
    id: Uuid,
    now: i64,
) -> Result<SaveResult, OnlineError> {
    let mut tx = connection.begin().await.map_err(db_error)?;
    let header = active_header(&mut tx, id)
        .await?
        .ok_or(OnlineError::Malformed)?;
    if header.owner != owner
        || header.room.is_none()
        || header.admitted_at > now
        || header.started_at.is_some_and(|at| at > now)
    {
        return Err(OnlineError::Malformed);
    }
    let saved = if header.started_at.is_some() {
        SaveResult::Duplicate
    } else {
        sqlx::query("UPDATE online_active_matches SET started_at=to_timestamp($2::double precision) WHERE id=$1")
            .bind(id).bind(now).execute(&mut *tx).await.map_err(db_error)?;
        SaveResult::Saved
    };
    tx.commit().await.map_err(db_error)?;
    Ok(saved)
}

fn ranked(reason: PublicEndReason) -> bool {
    matches!(
        reason,
        PublicEndReason::Clear | PublicEndReason::Timeout | PublicEndReason::Forfeit
    )
}

pub(super) fn validate_result(result: &FinishedRace, now: i64) -> Result<(), OnlineError> {
    let accounts: Vec<_> = result.players.iter().map(|player| player.account).collect();
    let ranked = ranked(result.reason);
    if result.room_id.is_nil()
        || !valid_header(result.id, &result.rules_hash, now)
        || !valid_accounts(&accounts)
        || result.ended_ms > 9007199254740991
        || (ranked && !result.players.iter().any(|player| player.rank == Some(1)))
        || result.players.iter().any(|player| {
            let stats = &player.stats;
            (player.rank.is_some() != (ranked && !player.forfeited))
                || player
                    .rank
                    .is_some_and(|rank| rank == 0 || usize::from(rank) > accounts.len())
                || stats.opened_safe > 256
                || stats.mistakes > 4096
                || stats.accusation_attempts > 4096
                || stats.correct_accusations > stats.accusation_attempts
        })
    {
        return Err(OnlineError::Malformed);
    }
    Ok(())
}

type StoredStanding = (
    i16,
    Uuid,
    Option<i16>,
    bool,
    bool,
    String,
    i16,
    i16,
    i16,
    i16,
);

type StoredRaceHeader = (String, Option<Vec<u8>>, Option<i64>, String, i16);

fn standings(result: &FinishedRace) -> Vec<StoredStanding> {
    result
        .players
        .iter()
        .enumerate()
        .map(|(seat, player)| {
            let tied = player.rank.is_some()
                && result
                    .players
                    .iter()
                    .filter(|entry| entry.rank == player.rank)
                    .count()
                    > 1;
            let status = match (result.reason, player.rank) {
                (PublicEndReason::Cancelled, _) => Outcome::Cancelled,
                (PublicEndReason::Abandoned | PublicEndReason::ServerFailure, _) => Outcome::Abort,
                (_, Some(1)) if tied => Outcome::Draw,
                (_, Some(1)) => Outcome::Win,
                _ => Outcome::Loss,
            };
            (
                seat as i16,
                player.account,
                player.rank.map(i16::from),
                tied,
                player.forfeited,
                outcome(status).to_owned(),
                player.stats.opened_safe as i16,
                player.stats.mistakes as i16,
                player.stats.accusation_attempts as i16,
                player.stats.correct_accusations as i16,
            )
        })
        .collect()
}

async fn duplicate(
    connection: &mut PgConnection,
    result: &FinishedRace,
    expected: &[StoredStanding],
) -> Result<(), OnlineError> {
    let row: Option<StoredRaceHeader> = sqlx::query_as("SELECT rules_hash,secret_seed,end_elapsed_ms,reason,participant_count FROM online_match_results WHERE id=$1 AND room_id=$2")
        .bind(result.id).bind(result.room_id).fetch_optional(&mut *connection).await.map_err(db_error)?;
    let Some((hash, seed, elapsed, stored_reason, count)) = row else {
        return Err(OnlineError::Malformed);
    };
    if hash != result.rules_hash
        || seed.is_some_and(|seed| seed != result.seed)
        || elapsed != Some(result.ended_ms as i64)
        || stored_reason != reason(result.reason)
        || count as usize != expected.len()
    {
        return Err(OnlineError::Malformed);
    }
    let rows: Vec<StoredStanding> = sqlx::query_as("SELECT seat,account_id,rank,tied,forfeited,outcome,opened_safe,mistakes,accusations,correct_accusations FROM online_match_players WHERE match_id=$1 ORDER BY seat")
        .bind(result.id).fetch_all(connection).await.map_err(db_error)?;
    if rows
        .iter()
        .any(|row| expected.get(row.0 as usize) != Some(row))
    {
        return Err(OnlineError::Malformed);
    }
    Ok(())
}

pub(super) async fn finish(
    connection: &mut PgConnection,
    owner: Uuid,
    result: FinishedRace,
    now: i64,
) -> Result<SaveResult, OnlineError> {
    let mut tx = connection.begin().await.map_err(db_error)?;
    let active = active_header(&mut tx, result.id).await?;
    let expected = standings(&result);
    if final_exists(&mut tx, result.id).await? {
        if active.is_some() {
            return Err(OnlineError::Malformed);
        }
        duplicate(&mut tx, &result, &expected).await?;
        tx.commit().await.map_err(db_error)?;
        return Ok(SaveResult::Duplicate);
    }
    let active = active.ok_or(OnlineError::Malformed)?;
    let at = active.started_at.ok_or(OnlineError::Malformed)?;
    let count = expected.len() as i16;
    if active.hash != result.rules_hash
        || active.owner != owner
        || at > now
        || active.room != Some(result.room_id)
        || active.count != count
    {
        return Err(OnlineError::Malformed);
    }
    let players = survivors(&mut tx, result.id, active.room, count).await?;
    if players
        .iter()
        .any(|(seat, account, _)| *account != Some(expected[*seat as usize].1))
    {
        return Err(OnlineError::Malformed);
    }
    let seed = (now < at + 604800).then(|| result.seed.to_vec());
    sqlx::query("INSERT INTO online_match_results(id,rules_hash,secret_seed,end_elapsed_ms,reason,recorded_at,seed_expires_at,retention_started_at,room_id,participant_count) VALUES($1,$2,$3,$4,$5,to_timestamp($6::double precision),to_timestamp($7::double precision),to_timestamp($8::double precision),$9,$10)")
        .bind(result.id).bind(result.rules_hash).bind(seed).bind(result.ended_ms as i64).bind(reason(result.reason)).bind(now).bind(at+604800).bind(at).bind(result.room_id).bind(count)
        .execute(&mut *tx).await.map_err(db_error)?;
    for (seat, _, _) in players {
        let (_, account, rank, tied, forfeited, status, opened, mistakes, attempts, correct) =
            &expected[seat as usize];
        sqlx::query("INSERT INTO online_match_players(match_id,seat,account_id,is_bot,outcome,opened_safe,mistakes,accusations,correct_accusations,participant_count,rank,tied,forfeited) VALUES($1,$2,$3,false,$4,$5,$6,$7,$8,$9,$10,$11,$12)")
            .bind(result.id).bind(seat).bind(account).bind(status).bind(opened).bind(mistakes).bind(attempts).bind(correct).bind(count).bind(rank).bind(tied).bind(forfeited)
            .execute(&mut *tx).await.map_err(db_error)?;
    }
    crate::events::collect_online(&mut tx, result.id, now)
        .await
        .map_err(|_| OnlineError::Unavailable)?;
    delete_active(&mut tx, result.id).await?;
    tx.commit().await.map_err(db_error)?;
    Ok(SaveResult::Saved)
}
