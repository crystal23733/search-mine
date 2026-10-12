use super::*;

async fn page(
    connection: &mut PgConnection,
    accepted: &Publication,
    now: i64,
    timed: bool,
) -> Result<Vec<RankedDailyCompletion>, DailyStoreError> {
    let mut query = sqlx::QueryBuilder::new(
        "SELECT c.id,c.daily_id,c.attempt_id,c.replay_digest,c.opened_safe,c.mistakes,c.replayed_elapsed_ms,c.verified_elapsed_ms,EXTRACT(EPOCH FROM c.completed_at)::bigint AS completed_at,EXTRACT(EPOCH FROM c.replay_expires_at)::bigint AS replay_expires_at,EXTRACT(EPOCH FROM c.record_expires_at)::bigint AS record_expires_at,a.nickname FROM daily_completions c JOIN auth_accounts a ON a.id=c.account_id WHERE c.daily_id=",
    );
    query
        .push_bind(accepted.definition.metadata().id())
        .push(" AND c.record_expires_at>to_timestamp(")
        .push_bind(now)
        .push("::double precision) AND a.nickname IS NOT NULL");
    if timed {
        query.push(" AND c.verified_elapsed_ms IS NOT NULL ORDER BY c.verified_elapsed_ms,c.mistakes,c.completed_at,c.id");
    } else {
        query.push(" ORDER BY c.mistakes,c.completed_at,c.id");
    }
    query
        .push(" LIMIT ")
        .push_bind((DAILY_RANKING_LIMIT + 1) as i64);
    let rows = query
        .build()
        .fetch_all(connection)
        .await
        .map_err(unavailable)?;
    rows.iter()
        .map(|row| {
            Ok(RankedDailyCompletion {
                nickname: row.try_get("nickname").map_err(unavailable)?,
                receipt: receipt(row, accepted, now)?,
            })
        })
        .collect()
}
impl DailyRankingReader for PgDailyRepository {
    fn read<'a>(
        &'a self,
        date: &'a str,
        version: u16,
    ) -> DailyFuture<'a, Result<Option<DailyRankingSnapshot>, DailyStoreError>> {
        Box::pin(bounded(async move {
            PublishedDaily::new(date, version, RulesSnapshot::bundled())
                .map_err(|_| DailyStoreError::Invalid)?;
            let now = self.clock.now();
            if now <= 0 {
                return Err(DailyStoreError::Unavailable);
            }
            let mut tx = self.pool.begin().await.map_err(unavailable)?;
            sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
                .execute(&mut *tx)
                .await
                .map_err(unavailable)?;
            let snapshot = if let Some(accepted) = by_date(&mut tx, date, version, now).await? {
                let time = page(&mut tx, &accepted, now, true).await?;
                let completion = page(&mut tx, &accepted, now, false).await?;
                Some(DailyRankingSnapshot {
                    definition: accepted.definition,
                    time,
                    completion,
                })
            } else {
                None
            };
            if self.clock.now() < now {
                return Err(DailyStoreError::Unavailable);
            }
            tx.commit().await.map_err(unavailable)?;
            Ok(snapshot)
        }))
    }
}
