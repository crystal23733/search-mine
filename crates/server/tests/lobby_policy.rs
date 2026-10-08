use liar_core::bot::Difficulty;
use liar_server::lobby::*;
use uuid::Uuid;
fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}
fn code() -> RoomCode {
    RoomCode::parse("ABCD2345").unwrap()
}
fn only(lobby: &LobbyState) -> MatchReservation {
    let all = lobby.reservations();
    assert_eq!(all.len(), 1);
    all[0]
}
#[test]
fn human_before_deadline_is_reserved_once_and_duplicate_join_is_idempotent() {
    let mut lobby = LobbyState::new(4).unwrap();
    let first = lobby.join(id(1), id(11), Difficulty::Easy, 0).unwrap();
    assert_eq!(
        first,
        lobby.join(id(1), id(11), Difficulty::Easy, 0).unwrap()
    );
    lobby.join(id(2), id(12), Difficulty::Hard, 9999).unwrap();
    let reservation = only(&lobby);
    assert_eq!(reservation.opponent, OpponentKind::Human);
    assert_eq!(reservation.players, [Some(id(1)), Some(id(2))]);
    lobby.tick(10000).unwrap();
    assert_eq!(only(&lobby).key, reservation.key);
    assert!(lobby.join(id(1), id(13), Difficulty::Easy, 10000).is_err());
    assert!(lobby.commit(reservation.key, 10001).is_ok());
    assert!(lobby.commit(reservation.key, 10001).is_err());
    assert!(lobby.status(id(1)).is_none());
}
#[test]
fn human_at_deadline_does_not_steal_the_expired_tickets_bot() {
    let mut lobby = LobbyState::new(4).unwrap();
    lobby.join(id(1), id(11), Difficulty::Hard, 0).unwrap();
    lobby.join(id(2), id(12), Difficulty::Easy, 10000).unwrap();
    let bot = only(&lobby);
    assert_eq!(bot.opponent, OpponentKind::Bot);
    assert_eq!(bot.players, [Some(id(1)), None]);
    assert_eq!(bot.difficulty, Difficulty::Hard);
    assert!(matches!(
        lobby.status(id(2)),
        Some(LobbyStatus::Queued {
            deadline: 20000,
            ..
        })
    ));
    lobby.tick(10001).unwrap();
    assert_eq!(only(&lobby).key, bot.key);
}
#[test]
fn cancelled_queue_and_late_board_completion_cannot_start() {
    let mut lobby = LobbyState::new(4).unwrap();
    lobby.join(id(1), id(11), Difficulty::Normal, 0).unwrap();
    lobby.cancel(id(1), 9999).unwrap();
    lobby.tick(10000).unwrap();
    assert!(lobby.reservations().is_empty());
    lobby
        .join(id(1), id(12), Difficulty::Normal, 10000)
        .unwrap();
    lobby
        .join(id(2), id(13), Difficulty::Normal, 10001)
        .unwrap();
    let old = only(&lobby);
    lobby.cancel(id(1), 10002).unwrap();
    assert!(lobby.commit(old.key, 10002).is_err());
    assert!(matches!(
        lobby.status(id(2)),
        Some(LobbyStatus::Queued { .. })
    ));
    lobby.tick(20001).unwrap();
    let new = only(&lobby);
    assert_eq!(new.players, [Some(id(2)), None]);
    assert_ne!(old.key, new.key);
}
#[test]
fn room_requires_two_authenticated_owners_and_two_ready_values() {
    let mut lobby = LobbyState::new(4).unwrap();
    let first = lobby.create_room(id(1), id(21), code(), 0).unwrap();
    assert_eq!(first, lobby.create_room(id(1), id(21), code(), 0).unwrap());
    lobby.ready(id(1), id(21), true, 1).unwrap();
    assert!(lobby.reservations().is_empty());
    let joined = lobby.join_room(id(2), code(), 2).unwrap();
    assert_eq!(joined, lobby.join_room(id(2), code(), 2).unwrap());
    assert!(matches!(
        lobby.join_room(id(3), code(), 2),
        Err(LobbyError::Full)
    ));
    assert!(lobby.ready(id(3), id(21), true, 2).is_err());
    lobby.ready(id(2), id(21), true, 3).unwrap();
    let reservation = only(&lobby);
    assert_eq!(reservation.players, [Some(id(1)), Some(id(2))]);
    assert_eq!(reservation.opponent, OpponentKind::Human);
    lobby.ready(id(2), id(21), true, 4).unwrap();
    assert_eq!(only(&lobby).key, reservation.key);
    lobby.commit(reservation.key, 5).unwrap();
    assert!(lobby.join_room(id(3), code(), 5).is_err());
}
#[test]
fn unready_invalidates_old_generation_even_when_the_same_room_readies_again() {
    let mut lobby = LobbyState::new(4).unwrap();
    lobby.create_room(id(1), id(21), code(), 0).unwrap();
    lobby.join_room(id(2), code(), 1).unwrap();
    lobby.ready(id(1), id(21), true, 1).unwrap();
    lobby.ready(id(2), id(21), true, 2).unwrap();
    let old = only(&lobby);
    lobby.ready(id(2), id(21), false, 3).unwrap();
    assert!(lobby.commit(old.key, 3).is_err());
    lobby.ready(id(2), id(21), true, 4).unwrap();
    let fresh = only(&lobby);
    assert_eq!(old.key.entity, fresh.key.entity);
    assert_ne!(old.key.generation, fresh.key.generation);
    assert!(lobby.commit(old.key, 4).is_err());
    assert!(lobby.commit(fresh.key, 4).is_ok());
}
#[test]
fn host_leave_promotes_remaining_friend_and_resets_ready() {
    let mut lobby = LobbyState::new(4).unwrap();
    lobby.create_room(id(1), id(21), code(), 0).unwrap();
    lobby.join_room(id(2), code(), 1).unwrap();
    lobby.ready(id(1), id(21), true, 1).unwrap();
    lobby.cancel(id(1), 2).unwrap();
    assert!(matches!(
        lobby.status(id(2)),
        Some(LobbyStatus::Room {
            own_seat: 0,
            ready: [false, false],
            occupied: [true, false],
            ..
        })
    ));
    lobby.cancel(id(2), 3).unwrap();
    assert!(lobby.join_room(id(3), code(), 4).is_err());
}
#[test]
fn room_and_preparation_expire_at_the_exact_boundary() {
    let mut lobby = LobbyState::new(4).unwrap();
    lobby.create_room(id(1), id(21), code(), 0).unwrap();
    assert!(lobby.join_room(id(2), code(), 600000).is_err());
    assert!(lobby.status(id(1)).is_none());
    lobby.join(id(1), id(11), Difficulty::Easy, 600000).unwrap();
    lobby.tick(610000).unwrap();
    let prepared = only(&lobby);
    assert_eq!(prepared.expires_at, 615000);
    assert!(lobby.commit(prepared.key, 615000).is_err());
    assert!(lobby.status(id(1)).is_none());
}
#[test]
fn admission_rejects_busy_accounts_live_id_code_collisions_and_capacity() {
    let mut lobby = LobbyState::new(2).unwrap();
    lobby.create_room(id(1), id(21), code(), 0).unwrap();
    assert!(matches!(
        lobby.join(id(1), id(11), Difficulty::Normal, 0),
        Err(LobbyError::Busy)
    ));
    assert!(matches!(
        lobby.create_room(id(2), id(22), code(), 0),
        Err(LobbyError::Collision)
    ));
    assert!(lobby.join(id(2), id(21), Difficulty::Normal, 0).is_err());
    lobby.join(id(2), id(11), Difficulty::Normal, 0).unwrap();
    assert!(matches!(
        lobby.join(id(3), id(12), Difficulty::Normal, 0),
        Err(LobbyError::Capacity)
    ));
    assert!(
        lobby
            .create_room(id(2), id(22), RoomCode::parse("ZZZZ2345").unwrap(), 0)
            .is_err()
    );
    assert!(lobby.join_room(id(2), code(), 0).is_err());
    lobby.cancel(id(2), 0).unwrap();
    lobby.join_room(id(2), code(), 0).unwrap();
}
#[test]
fn invalid_identity_code_clock_and_capacity_do_not_admit_a_member() {
    for invalid in [
        "",
        "ABCD1234",
        "abcd2345",
        "ABCDEFGI",
        "ABCDEFG0",
        "ABCDEFG",
        "ABCDEFGHX",
        "ABCD💣",
    ] {
        assert!(RoomCode::parse(invalid).is_err());
    }
    assert_eq!(code().as_str(), "ABCD2345");
    assert!(LobbyState::new(0).is_err());
    assert!(LobbyState::new(4097).is_err());
    let mut lobby = LobbyState::new(4).unwrap();
    assert!(
        lobby
            .join(Uuid::nil(), id(11), Difficulty::Normal, 0)
            .is_err()
    );
    assert!(
        lobby
            .join(id(1), Uuid::nil(), Difficulty::Normal, 0)
            .is_err()
    );
    assert!(lobby.create_room(Uuid::nil(), id(21), code(), 0).is_err());
    assert!(lobby.create_room(id(1), Uuid::nil(), code(), 0).is_err());
    lobby.tick(100).unwrap();
    assert!(matches!(lobby.tick(99), Err(LobbyError::InvalidTime)));
    assert!(lobby.tick(u64::MAX).is_err());
    assert!(lobby.status(id(1)).is_none());
    assert!(lobby.cancel(id(1), 100).is_ok());
}
#[test]
fn restored_waiter_matches_the_oldest_existing_waiter_without_another_join() {
    let mut lobby = LobbyState::new(4).unwrap();
    lobby.join(id(1), id(11), Difficulty::Normal, 0).unwrap();
    lobby.join(id(2), id(12), Difficulty::Normal, 1).unwrap();
    let first = only(&lobby);
    lobby.join(id(3), id(13), Difficulty::Normal, 2).unwrap();
    lobby.cancel(id(1), 3).unwrap();
    let replacement = only(&lobby);
    assert_ne!(first.key, replacement.key);
    assert_eq!(replacement.players, [Some(id(2)), Some(id(3))]);
}
#[test]
fn second_ticket_id_remains_reserved_during_asynchronous_preparation() {
    let mut lobby = LobbyState::new(4).unwrap();
    lobby.join(id(1), id(11), Difficulty::Normal, 0).unwrap();
    lobby.join(id(2), id(12), Difficulty::Normal, 1).unwrap();
    assert!(matches!(
        lobby.join(id(3), id(12), Difficulty::Normal, 2),
        Err(LobbyError::Collision)
    ));
}
