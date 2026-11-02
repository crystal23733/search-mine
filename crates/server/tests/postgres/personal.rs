use super::*;
use liar_protocol::me::*;
use liar_server::me::*;
struct Clock(i64);
impl AuthClock for Clock {
    fn now(&self) -> i64 {
        self.0
    }
}
const START: i64 = 1791331200;
pub(super) async fn participant(pool: &sqlx::PgPool, now: i64) -> MeSession {
    let token = SecretToken::generate().unwrap();
    let store = PgAuthStore::new(pool.clone());
    let login = store
        .login(super::rights::account_write(
            Provider::Google,
            &Uuid::new_v4().to_string(),
            &token,
            now,
        ))
        .await
        .unwrap();
    let session = store.session(token.hash(), now).await.unwrap().unwrap();
    MeSession {
        account: login.account.id,
        revision: session.id,
        hash: token.hash(),
    }
}
pub(super) async fn seed_online(pool: &sqlx::PgPool, account: Uuid, id: Uuid, at: i64) {
    sqlx::query("INSERT INTO online_match_results(id,rules_hash,secret_seed,end_elapsed_ms,reason,recorded_at,seed_expires_at) VALUES($1,$2,$3,243000,'timeout',to_timestamp($4::double precision),to_timestamp($4::double precision)+interval '7 days')").bind(id).bind(liar_core::rules::RulesSnapshot::bundled().hash).bind([46u8;8].as_slice()).bind(at).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO online_match_players(match_id,seat,account_id,is_bot,outcome,opened_safe,mistakes,accusations,correct_accusations) VALUES($1,0,$2,false,'draw',4,0,0,0),($1,1,NULL,true,'draw',99,8,5,3)").bind(id).bind(account).execute(pool).await.unwrap();
}
pub(super) async fn fresh(pool: &sqlx::PgPool, session: MeSession, at: i64) {
    sqlx::query("UPDATE auth_sessions SET created_at=to_timestamp($2::double precision),authenticated_at=to_timestamp($2::double precision),expires_at=to_timestamp($2::double precision)+interval '30 days' WHERE id=$1").bind(session.revision).bind(at).execute(pool).await.unwrap();
}
pub(super) async fn completed(pool: &sqlx::PgPool) -> (MeSession, Uuid) {
    use liar_server::daily::*;
    let session = participant(pool, START + 60).await;
    let auth = PgAuthStore::new(pool.clone());
    auth.nickname(session.account, Nickname::parse("Daily Player").unwrap())
        .await
        .unwrap();
    let definition =
        PublishedDaily::new("2026-10-07", 1, liar_core::rules::RulesSnapshot::bundled()).unwrap();
    let fixture: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native-wasm.json"
    )))
    .unwrap();
    let evidence = NativeReplayVerifier
        .verify(&definition, &fixture["daily"]["replay"].to_string())
        .unwrap();
    let daily = PgDailyRepository::new(pool.clone(), Arc::new(Clock(START + 60)));
    daily.publish(definition).await.unwrap();
    let DailySubmission::Official(receipt) = daily
        .submit(
            DailySession {
                account: session.account,
                hash: session.hash,
            },
            Uuid::new_v4(),
            &evidence,
        )
        .await
        .unwrap()
    else {
        panic!()
    };
    sqlx::query("UPDATE auth_accounts SET nickname=NULL WHERE id=$1")
        .bind(session.account)
        .execute(pool)
        .await
        .unwrap();
    (session, receipt.id)
}
pub(super) async fn daily_history(
    reader: &PgMeReader,
    owner: MeSession,
) -> Result<MeHistory, MeError> {
    let read = reader
        .read(
            owner,
            MeQuery::History {
                kind: MeKind::Daily,
                opponent: MeOpponentFilter::All,
                after: None,
            },
        )
        .await?;
    if let MePayload::History(page) = read.payload {
        Ok(page)
    } else {
        Err(MeError::Unavailable)
    }
}
