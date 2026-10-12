use super::*;
use liar_core::rules::RulesSnapshot;
use uuid::Uuid;

const NOW: i64 = 1_791_331_260;
fn definition() -> PublishedDaily {
    PublishedDaily::new("2026-10-07", 1, RulesSnapshot::bundled()).unwrap()
}
fn row(index: u16, elapsed: Option<u32>) -> RankedDailyCompletion {
    let definition = definition();
    let rules = &definition.rules().rules;
    RankedDailyCompletion {
        nickname: format!("Player {index}"),
        receipt: DailyReceipt {
            id: Uuid::from_u128(u128::from(index) + 1),
            attempt: Uuid::from_u128(u128::from(index) + 100),
            metadata: definition.metadata().clone(),
            opened_safe: (rules.board_spec().area() - usize::from(rules.mines)) as u32,
            mistakes: 0,
            replayed_elapsed_ms: 3200,
            verified_elapsed_ms: elapsed,
            completed_at: NOW - 10,
        },
    }
}
fn snapshot() -> DailyRankingSnapshot {
    DailyRankingSnapshot {
        definition: definition(),
        time: vec![],
        completion: vec![],
    }
}
#[test]
fn projection_only_exposes_positions_nickname_errors_and_verified_time() {
    let mut value = snapshot();
    value.time.push(row(1, Some(5000)));
    value.completion.push(row(2, None));
    let reply = value.project("2026-10-07", 1, NOW).unwrap();
    let json = serde_json::to_value(reply).unwrap();
    assert_eq!(
        json["time"],
        serde_json::json!({"entries":[{"position":1,"nickname":"Player 1","mistakes":0,"elapsed_ms":5000}],"has_more":false})
    );
    assert_eq!(
        json["completion"],
        serde_json::json!({"entries":[{"position":1,"nickname":"Player 2","mistakes":0}],"has_more":false})
    );
    assert_eq!(
        json["metadata"],
        serde_json::to_value(definition().metadata()).unwrap()
    );
    assert!(empty_rankings().metadata.is_none());
}
#[test]
fn accepted_snapshot_and_fifty_row_limit_with_more_are_preserved() {
    let mut value = snapshot();
    let mut rules = RulesSnapshot::bundled().rules;
    rules.duration_ms += 1;
    value.definition =
        PublishedDaily::new("2026-10-07", 1, RulesSnapshot::from_rules(rules).unwrap()).unwrap();
    let metadata = value.definition.metadata().clone();
    for index in 0..51 {
        let mut timed = row(index, Some(5000 + u32::from(index)));
        timed.receipt.metadata = metadata.clone();
        let mut completed = row(index, None);
        completed.receipt.metadata = metadata.clone();
        value.time.push(timed);
        value.completion.push(completed);
    }
    let reply = value.project("2026-10-07", 1, NOW).unwrap();
    assert_eq!(reply.metadata, Some(metadata));
    assert_eq!(reply.time.entries.len(), 50);
    assert_eq!(reply.completion.entries.len(), 50);
    assert!(reply.time.has_more && reply.completion.has_more);
    assert_eq!(reply.time.entries[49].position, 50);
}
#[test]
fn malformed_stale_missing_time_duplicate_and_unsorted_rows_fail_closed() {
    for mode in 0..17 {
        let mut value = snapshot();
        let mut first = row(1, Some(5000));
        match mode {
            0 => first.nickname = "<script>".into(),
            1 => first.nickname = "Cafe\u{301}".into(),
            2 => first.receipt.id = Uuid::nil(),
            3 => first.receipt.completed_at = NOW + 1,
            4 => first.receipt.completed_at = NOW - 90 * 86400,
            5 => first.receipt.verified_elapsed_ms = None,
            6 => first.receipt.verified_elapsed_ms = Some(3000),
            7 => first.receipt.verified_elapsed_ms = Some(definition().rules().rules.duration_ms),
            8 => first.receipt.metadata.date = "2026-10-06".into(),
            9 => first.receipt.opened_safe = 0,
            10 => first.receipt.replayed_elapsed_ms = definition().rules().rules.duration_ms,
            11 => value.time.push(row(1, Some(4000))),
            12 => value.time.push(row(2, Some(6000))),
            13 => {
                value.time = (0..52).map(|i| row(i, Some(5000 + u32::from(i)))).collect();
            }
            14 => {
                value.completion.push(row(2, None));
                value.completion.push(row(1, None));
            }
            15 => first.receipt.attempt = Uuid::nil(),
            16 => first.receipt.mistakes = u16::MAX,
            _ => unreachable!(),
        }
        value.time.push(first);
        assert!(value.project("2026-10-07", 1, NOW).is_err(), "mode {mode}");
    }
    assert!(snapshot().project("2026-10-06", 1, NOW).is_err());
    assert!(snapshot().project("bad", 1, NOW).is_err());
    assert!(snapshot().project("2026-10-07", 1, 0).is_err());
}
