//! Isolated real-main load measurement. No production auth bypass or external target.
use super::{
    admission_restart::{User, user},
    result_owner::{Server, server_command, wait_ready},
    *,
};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use std::{collections::HashMap, time::Instant};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{Message, client::IntoClientRequest},
};
#[derive(Default)]
struct Samples {
    ack: Vec<f64>,
    applied_ack: Vec<f64>,
    attack_ack: Vec<f64>,
    player_seconds: f64,
    attacks: u64,
    rejected: u64,
    errors: u64,
    matched: usize,
}
async fn lobby(
    client: &reqwest::Client,
    origin: &str,
    user: &User,
    command: Value,
) -> Result<Value, ()> {
    let response = client
        .post(format!("{origin}/api/v1/lobby"))
        .header("origin", "https://game.example")
        .header("cookie", &user.cookie)
        .header("x-liar-csrf", &user.csrf)
        .json(&json!({"v":1,"command":command}))
        .send()
        .await
        .map_err(|_| ())?;
    if !response.status().is_success() {
        return Err(());
    }
    response.json().await.map_err(|_| ())
}
async fn play(origin: String, user: User, seconds: u64) -> Samples {
    let mut samples = Samples::default();
    let began = Instant::now();
    let work = async {
        let mut request = format!("{}/api/v1/ws", origin.replacen("http://", "ws://", 1))
            .into_client_request()
            .map_err(|_| ())?;
        request
            .headers_mut()
            .insert("origin", "https://game.example".parse().unwrap());
        request
            .headers_mut()
            .insert("cookie", user.cookie.parse().map_err(|_| ())?);
        let (mut socket, _) = tokio::time::timeout(Duration::from_secs(3), connect_async(request))
            .await
            .map_err(|_| ())?
            .map_err(|_| ())?;
        let stop = tokio::time::Instant::now() + Duration::from_secs(seconds);
        let mut ticks = tokio::time::interval(Duration::from_millis(100));
        ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut latest = Value::Null;
        let mut epoch = 0;
        let mut sequence = 0;
        let mut match_id = String::new();
        let mut pending = HashMap::<String, (Instant, bool)>::new();
        loop {
            tokio::select! {
                event=socket.next()=>{
                    let message=event.ok_or(())?.map_err(|_|())?;
                    match message {
                        Message::Text(text)=>{
                            let event:Value=serde_json::from_str(&text).map_err(|_|())?;
                            match_id=event["match_id"].as_str().ok_or(())?.to_owned();
                            let payload=&event["payload"];
                            if let Some(view)=payload.get("view") {latest=view.clone();}
                            match payload["type"].as_str() {
                                Some("snapshot")=>{epoch=payload["session_epoch"].as_u64().ok_or(())?;sequence=payload["last_client_seq"].as_u64().ok_or(())?;}
                                Some("ack")=>{
                                    let id=payload["command_id"].as_str().ok_or(())?;
                                    let (start,attack)=pending.remove(id).ok_or(())?;
                                    let elapsed=start.elapsed().as_secs_f64()*1000.0;
                                    samples.ack.push(elapsed);
                                    if payload["status"]=="applied" {samples.applied_ack.push(elapsed);if attack{samples.attacks+=1;samples.attack_ack.push(elapsed);}}
                                    else {samples.rejected+=1;}
                                }
                                Some("error")=>return Err(()),
                                _=>{}
                            }
                            if latest["phase"]=="finished" || latest["phase"]=="aborted" {break;}
                        }
                        Message::Ping(bytes)=>socket.send(Message::Pong(bytes)).await.map_err(|_|())?,
                        Message::Close(_)=>return Err(()),
                        _=>{}
                    }
                }
                _=ticks.tick()=>{
                    if tokio::time::Instant::now()>=stop {
                        if pending.is_empty(){break;}
                        if tokio::time::Instant::now()>=stop+Duration::from_secs(2){return Err(());}
                        continue;
                    }
                    if pending.values().any(|(at,_)|at.elapsed()>Duration::from_secs(2)){return Err(());}
                    if !pending.is_empty() || epoch==0 || latest["phase"]!="playing" || latest["own"]["stun_ms"].as_u64()!=Some(0) {continue;}
                    let gauge=latest["own"]["gauge"].as_u64().ok_or(())?;
                    let capacity=latest["rules"]["rules"]["gauge_capacity"].as_u64().ok_or(())?;
                    // A fixed mixed workload; no hidden board or duplicated game rule is used.
                    let attack=sequence%10==5 && gauge>=capacity;
                    let action=if attack {json!({"type":"attack"})} else {
                        let opening=sequence%10==0;
                        let cell=latest["own"]["cells"].as_array().ok_or(())?.iter().find(|c|c["state"]=="closed" && (!opening || c["flagged"]==false));
                        let Some(cell)=cell else {continue;};
                        json!({"type":if opening {"open"}else{"flag"},"cell":cell["cell"]})
                    };
                    sequence+=1;
                    let id=Uuid::new_v4().to_string();
                    let command=json!({"v":1,"match_id":match_id,"command_id":id,"client_seq":sequence,"session_epoch":epoch,"known_revision":latest["revision"],"action":action});
                    pending.insert(id,(Instant::now(),attack));
                    socket.send(Message::Text(command.to_string().into())).await.map_err(|_|())?;
                }
            }
        }
        let _ = socket.close(None).await;
        Ok::<_, ()>(())
    };
    if work.await.is_err() {
        samples.errors += 1;
    }
    samples.player_seconds = began.elapsed().as_secs_f64();
    samples
}
fn setting(name: &str, default: u64, min: u64, max: u64) -> u64 {
    let value = env::var(name).map_or(default, |v| v.parse().expect("Invalid load setting"));
    assert!(
        (min..=max).contains(&value),
        "Load setting outside bounded range"
    );
    value
}
fn percentile(values: &[f64], percent: usize) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    Some(values[(values.len() * percent).div_ceil(100).saturating_sub(1)])
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "isolated PostgreSQL and real-main workload; default CI is only a short smoke"]
async fn isolated_main_load_measurement() {
    let pairs = setting("LIAR_LOAD_PAIRS", 1, 1, 128) as usize;
    let seconds = setting("LIAR_LOAD_SECONDS", 8, 5, 300);
    let pool = auth_pool().await;
    let (mut command, origin) = server_command(&pool).await;
    // Only capacity controls are forwarded; never an origin, bind or external credential.
    let mut limits = serde_json::Map::new();
    for key in [
        "LIAR_ONLINE_MATCHES",
        "LIAR_ONLINE_CONNECTIONS",
        "LIAR_ONLINE_PROOF_WORKERS",
        "LIAR_LOBBY_CAPACITY",
        "LIAR_LOBBY_WORKERS",
        "LIAR_BOARD_CAPACITY",
        "LIAR_BOARD_WORKERS",
    ] {
        if let Ok(value) = env::var(key) {
            assert!(value.bytes().all(|b| b.is_ascii_digit()));
            command.env(key, &value);
            limits.insert(key.into(), json!(value));
        }
    }
    let mut server = Server(command.spawn().unwrap());
    wait_ready(&mut server, &origin).await;
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(3))
        .build()
        .unwrap();
    let mut users = Vec::new();
    for index in 0..pairs * 2 {
        users.push(user(&pool, &client, &origin, &format!("Load{index}")).await);
    }
    let started = Instant::now();
    let mut joins = tokio::task::JoinSet::new();
    for user in users {
        let client = client.clone();
        let origin = origin.clone();
        joins.spawn(async move {
            lobby(
                &client,
                &origin,
                &user,
                json!({"type":"queue_join","difficulty":"normal"}),
            )
            .await?;
            let deadline = tokio::time::Instant::now() + Duration::from_secs(8);
            loop {
                let state = lobby(&client, &origin, &user, json!({"type":"status"})).await?;
                match state["state"]["type"].as_str() {
                    Some("matched") => break,
                    Some("failed") => return Err(()),
                    _ => {}
                }
                if tokio::time::Instant::now() >= deadline {
                    return Err(());
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            Ok::<_, ()>(user)
        });
    }
    let mut sockets = tokio::task::JoinSet::new();
    let mut total = Samples::default();
    while let Some(result) = joins.join_next().await {
        match result {
            Ok(Ok(user)) => {
                total.matched += 1;
                sockets.spawn(play(origin.clone(), user, seconds));
            }
            _ => total.errors += 1,
        }
    }
    while let Some(result) = sockets.join_next().await {
        match result {
            Ok(sample) => {
                total.ack.extend(sample.ack);
                total.applied_ack.extend(sample.applied_ack);
                total.attack_ack.extend(sample.attack_ack);
                total.player_seconds += sample.player_seconds;
                total.attacks += sample.attacks;
                total.rejected += sample.rejected;
                total.errors += sample.errors;
            }
            Err(_) => total.errors += 1,
        }
    }
    total.ack.sort_by(f64::total_cmp);
    total.applied_ack.sort_by(f64::total_cmp);
    total.attack_ack.sort_by(f64::total_cmp);
    let source_commit = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned());
    let dirty = std::process::Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .map_or(true, |o| !o.status.success() || !o.stdout.is_empty());
    let report = json!({"version":1,"run_id":Uuid::new_v4(),"source_commit":source_commit,"worktree_dirty":dirty,"recorded_at":SystemAuthClock.now(),"profile":if cfg!(debug_assertions){"debug"}else{"release"},"os":env::consts::OS,"arch":env::consts::ARCH,"logical_cpus":std::thread::available_parallelism().unwrap().get(),"requested_pairs":pairs,"matched_players":total.matched,"duration_seconds":seconds,"elapsed_seconds":started.elapsed().as_secs_f64(),"limits":limits,"ack_samples":total.ack.len(),"applied_ack_samples":total.applied_ack.len(),"applied_ack_rtt_ms_p95":percentile(&total.applied_ack,95),"attack_rtt_ms_p95":percentile(&total.attack_ack,95),"player_seconds":total.player_seconds,"workload":"mixed_open_flag_attack_v1","ack_rtt_ms_p50":percentile(&total.ack,50),"ack_rtt_ms_p95":percentile(&total.ack,95),"ack_rtt_ms_p99":percentile(&total.ack,99),"applied_attacks":total.attacks,"rejected_commands":total.rejected,"errors":total.errors,"target":"isolated_real_main_loopback","capacity_verified":false,"oauth_exchange_measured":false});
    std::fs::create_dir_all(".tmp/load").unwrap();
    let file = format!(".tmp/load/{}.json", Uuid::new_v4());
    std::fs::write(&file, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    println!("Load observation: {file}\n{report}");
    drop(server);
    close_auth_pool(pool).await;
    // Stress runs preserve failures for analysis; the default smoke must actually drive inputs.
    if env::var_os("LIAR_LOAD_PAIRS").is_none() {
        assert_eq!(total.errors, 0);
        assert_eq!(total.matched, 2);
        assert!(!total.ack.is_empty());
    }
}
