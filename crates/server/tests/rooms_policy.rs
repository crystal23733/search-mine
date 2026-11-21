use liar_protocol::rooms::{RoomPhase, RoomVisibility};
use liar_server::{
    auth::Nickname,
    room_code::RoomCode,
    rooms::{RoomError, RoomPolicy, RoomSettings},
};
use uuid::Uuid;

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}
fn nick(n: u128) -> Nickname {
    Nickname::parse(&format!("Player {n}")).unwrap()
}
fn settings(title: &str, visibility: RoomVisibility, limit: u8) -> RoomSettings {
    RoomSettings::new(title, visibility, limit).unwrap()
}
fn code(n: u8) -> RoomCode {
    RoomCode::parse(&format!("ABCDEFG{}", char::from(b'2' + n))).unwrap()
}

#[test]
fn list_omits_private_rooms_and_prefers_open_rooms() {
    let mut rooms = RoomPolicy::new(8).unwrap();
    rooms
        .create(
            id(1),
            nick(1),
            id(11),
            code(0),
            settings("Private", RoomVisibility::Invite, 2),
            0,
        )
        .unwrap();
    rooms
        .create(
            id(2),
            nick(2),
            id(12),
            code(1),
            settings("Full", RoomVisibility::Public, 2),
            0,
        )
        .unwrap();
    rooms.join_public(id(3), nick(3), id(12), 0).unwrap();
    rooms
        .create(
            id(4),
            nick(4),
            id(13),
            code(2),
            settings("Open", RoomVisibility::Public, 4),
            0,
        )
        .unwrap();
    let cards = rooms.list(None, 0).unwrap().rooms;
    assert_eq!(
        cards
            .iter()
            .map(|room| room.title.as_str())
            .collect::<Vec<_>>(),
        ["Open", "Full"]
    );
    let serialized = serde_json::to_string(&cards).unwrap();
    assert!(!serialized.contains("Private"));
    assert!(!serialized.contains("ABCDEFG"));
    assert!(!serialized.contains("Player"));
    assert_eq!(
        rooms.join_public(id(5), nick(5), id(11), 0),
        Err(RoomError::NotFound)
    );
    assert_eq!(
        rooms
            .join_invite(id(5), nick(5), code(0), 0)
            .unwrap()
            .members
            .len(),
        2
    );
}

#[test]
fn ready_revisions_and_host_guard_freeze_one_start_reservation() {
    let mut rooms = RoomPolicy::new(8).unwrap();
    let room = id(10);
    let created = rooms
        .create(
            id(1),
            nick(1),
            room,
            code(0),
            settings("Open", RoomVisibility::Public, 4),
            0,
        )
        .unwrap();
    let joined = rooms.join_public(id(2), nick(2), room, 1).unwrap();
    assert_eq!(
        rooms.ready(id(1), room, &created.revision, true, 1),
        Err(RoomError::Stale)
    );
    assert_eq!(
        rooms.start(id(1), room, &joined.revision, id(20), 1),
        Err(RoomError::NotReady)
    );
    let host = rooms.ready(id(1), room, &joined.revision, true, 2).unwrap();
    let both = rooms.ready(id(2), room, &host.revision, true, 2).unwrap();
    assert_eq!(
        rooms.start(id(2), room, &both.revision, id(20), 2),
        Err(RoomError::Forbidden)
    );
    let pending = rooms.start(id(1), room, &both.revision, id(20), 2).unwrap();
    assert_eq!(pending.accounts, [id(1), id(2)]);
    assert_eq!(
        rooms.join_public(id(3), nick(3), room, 3),
        Err(RoomError::Busy)
    );
    let current = rooms.status(id(2), 3).unwrap().unwrap();
    let unready = rooms
        .ready(id(2), room, &current.revision, false, 3)
        .unwrap();
    assert_eq!(unready.phase, RoomPhase::Waiting);
    assert!(unready.members.iter().all(|member| !member.ready));
    assert_eq!(rooms.admit(pending.key, 4), Err(RoomError::Stale));
}

#[test]
fn late_joiner_waits_for_the_same_rooms_next_round() {
    let mut rooms = RoomPolicy::new(8).unwrap();
    let room = id(10);
    rooms
        .create(
            id(1),
            nick(1),
            room,
            code(0),
            settings("Open", RoomVisibility::Public, 4),
            0,
        )
        .unwrap();
    let joined = rooms.join_public(id(2), nick(2), room, 0).unwrap();
    let one = rooms.ready(id(1), room, &joined.revision, true, 1).unwrap();
    let two = rooms.ready(id(2), room, &one.revision, true, 1).unwrap();
    let first = rooms.start(id(1), room, &two.revision, id(20), 1).unwrap();
    rooms.admit(first.key, 2).unwrap();
    rooms.begin_round(first.key, 2).unwrap();
    let late = rooms.join_public(id(3), nick(3), room, 3).unwrap();
    assert_eq!(late.phase, RoomPhase::Playing);
    assert!(late.round.is_none());
    assert_eq!(
        rooms.status(id(1), 3).unwrap().unwrap().round.unwrap().seat,
        0
    );
    rooms.finish(first.key, 4).unwrap();
    let next = rooms.status(id(3), 4).unwrap().unwrap();
    assert_eq!(next.phase, RoomPhase::Results);
    assert!(next.members.iter().all(|entry| !entry.ready));
    let mut rev = next.revision;
    for account in [1, 2, 3] {
        rev = rooms
            .ready(id(account), room, &rev, true, 4)
            .unwrap()
            .revision;
    }
    let next = rooms.start(id(1), room, &rev, id(21), 4).unwrap();
    assert_eq!(next.accounts, [id(1), id(2), id(3)]);
    assert_ne!(next.key, first.key);
    assert_eq!(rooms.finish(first.key, 5), Err(RoomError::Stale));
}

#[test]
fn host_promotion_capacity_kick_and_idle_expiration_keep_members_consistent() {
    let mut rooms = RoomPolicy::new(10).unwrap();
    let room = id(10);
    rooms
        .create(
            id(1),
            nick(1),
            room,
            code(0),
            settings("Open", RoomVisibility::Public, 4),
            0,
        )
        .unwrap();
    for account in 2..=4 {
        rooms
            .join_public(id(account), nick(account), room, 0)
            .unwrap();
    }
    assert_eq!(
        rooms.join_public(id(5), nick(5), room, 0),
        Err(RoomError::Full)
    );
    assert_eq!(
        rooms.create(
            id(2),
            nick(2),
            id(12),
            code(1),
            settings("Other", RoomVisibility::Public, 4),
            0
        ),
        Err(RoomError::Busy)
    );
    let joined = rooms.status(id(1), 0).unwrap().unwrap();
    rooms.leave(id(1), room, &joined.revision, 1).unwrap();
    let promoted = rooms.status(id(2), 1).unwrap().unwrap();
    assert_eq!(promoted.own_seat, promoted.host_seat);
    assert!(promoted.members.iter().all(|member| !member.ready));
    assert_eq!(
        rooms.configure(
            id(2),
            room,
            &promoted.revision,
            settings("Small", RoomVisibility::Invite, 2),
            1
        ),
        Err(RoomError::Full)
    );
    let seat = promoted
        .members
        .iter()
        .find(|member| member.nickname == "Player 3")
        .unwrap()
        .seat;
    let kicked = rooms
        .kick(id(2), room, &promoted.revision, seat, 1)
        .unwrap();
    assert_eq!(kicked.members.len(), 2);
    assert_eq!(
        rooms.join_invite(id(3), nick(3), code(0), 1),
        Err(RoomError::NotFound)
    );
    assert!(rooms.status(id(2), 600000).unwrap().is_some());
    assert!(rooms.status(id(2), 600001).unwrap().is_none());
    assert_eq!(
        rooms.join_invite(id(4), nick(4), code(0), 600001),
        Err(RoomError::NotFound)
    );
}

#[test]
fn leaving_and_rejoining_does_not_restore_a_running_round_seat() {
    let mut rooms = RoomPolicy::new(8).unwrap();
    let room = id(10);
    rooms
        .create(
            id(1),
            nick(1),
            room,
            code(0),
            settings("Rerun", RoomVisibility::Public, 4),
            0,
        )
        .unwrap();
    let joined = rooms.join_public(id(2), nick(2), room, 0).unwrap();
    let one = rooms.ready(id(1), room, &joined.revision, true, 1).unwrap();
    let two = rooms.ready(id(2), room, &one.revision, true, 1).unwrap();
    let reservation = rooms.start(id(1), room, &two.revision, id(20), 1).unwrap();
    rooms.admit(reservation.key, 2).unwrap();
    rooms.begin_round(reservation.key, 2).unwrap();
    let first = rooms.status(id(1), 3).unwrap().unwrap();
    assert_eq!(first.round.as_ref().unwrap().seat, 0);
    assert_eq!(
        rooms.kick(id(1), room, &first.revision, 1, 3),
        Err(RoomError::Busy)
    );
    assert_eq!(
        rooms.configure(
            id(1),
            room,
            &first.revision,
            settings("Changed", RoomVisibility::Invite, 2),
            3
        ),
        Err(RoomError::Busy)
    );
    assert_eq!(
        rooms.ready(id(1), room, &first.revision, true, 3),
        Err(RoomError::Busy)
    );
    rooms.leave(id(1), room, &first.revision, 3).unwrap();
    let rejoined = rooms.join_public(id(1), nick(1), room, 4).unwrap();
    assert_eq!(rejoined.phase, RoomPhase::Playing);
    assert!(rejoined.round.is_none());
    assert!(
        rejoined
            .members
            .iter()
            .any(|member| member.nickname == "Player 1" && !member.in_round)
    );
    let promoted = rooms.status(id(2), 600_001).unwrap().unwrap();
    assert_eq!(promoted.own_seat, promoted.host_seat);
    assert_eq!(promoted.round.unwrap().seat, 1);
    assert_eq!(promoted.expires_at_ms, None);
    rooms.finish(reservation.key, 600_001).unwrap();
    assert!(
        rooms
            .status(id(2), 600_001)
            .unwrap()
            .unwrap()
            .expires_at_ms
            .is_some()
    );
}

#[test]
fn preparation_failure_or_deadline_frees_the_room_without_reusing_the_roster() {
    let mut rooms = RoomPolicy::new(8).unwrap();
    let room = id(10);
    rooms
        .create(
            id(1),
            nick(1),
            room,
            code(0),
            settings("Rerun", RoomVisibility::Public, 4),
            0,
        )
        .unwrap();
    rooms.join_public(id(2), nick(2), room, 0).unwrap();
    for attempt in 0..3 {
        let now = 1 + 5000 * attempt;
        let current = rooms.status(id(1), now).unwrap().unwrap();
        let one = rooms
            .ready(id(1), room, &current.revision, true, now)
            .unwrap();
        let same = rooms.ready(id(1), room, &one.revision, true, now).unwrap();
        assert_eq!(same.revision, one.revision);
        let two = rooms.ready(id(2), room, &one.revision, true, now).unwrap();
        let start = rooms
            .start(
                id(1),
                room,
                &two.revision,
                id(20 + u128::from(attempt)),
                now,
            )
            .unwrap();
        let key = start.key;
        assert_eq!(
            rooms.admit(
                liar_server::rooms::RoundKey {
                    generation: key.generation + 1,
                    ..key
                },
                now
            ),
            Err(RoomError::Stale)
        );
        match attempt {
            0 => rooms.fail_preparation(key, now).unwrap(),
            1 => assert_eq!(rooms.admit(key, start.expires_at_ms), Err(RoomError::Stale)),
            _ => {
                let leave = rooms.status(id(2), now).unwrap().unwrap();
                rooms.leave(id(2), room, &leave.revision, now).unwrap();
            }
        }
        let observed = rooms
            .status(
                id(1),
                now.max(if attempt == 1 { start.expires_at_ms } else { 0 }),
            )
            .unwrap()
            .unwrap();
        assert_eq!(observed.phase, RoomPhase::Waiting);
        assert!(observed.members.iter().all(|member| !member.ready));
    }
}

#[test]
fn invalid_inputs_and_room_capacity_do_not_change_existing_membership() {
    assert!(matches!(RoomPolicy::new(0), Err(RoomError::Capacity)));
    for title in ["x", " 앞", "bad\nname", "space ", "a\u{202e}b", "<room>"] {
        assert_eq!(
            RoomSettings::new(title, RoomVisibility::Public, 4),
            Err(RoomError::Invalid)
        );
    }
    assert_eq!(
        RoomSettings::new("Good", RoomVisibility::Public, 5),
        Err(RoomError::Invalid)
    );
    let valid = || settings("Cafe\u{301}", RoomVisibility::Public, 2);
    let mut rooms = RoomPolicy::new(2).unwrap();
    assert_eq!(
        rooms.create(Uuid::nil(), nick(1), id(10), code(0), valid(), 0),
        Err(RoomError::Invalid)
    );
    let created = rooms
        .create(id(1), nick(1), id(10), code(0), valid(), 1)
        .unwrap();
    assert_eq!(created.options.title, "Café");
    assert_eq!(
        rooms.create(id(2), nick(2), id(10), code(1), valid(), 1),
        Err(RoomError::Conflict)
    );
    assert_eq!(
        rooms.create(id(2), nick(2), id(11), code(0), valid(), 1),
        Err(RoomError::Conflict)
    );
    assert_eq!(
        rooms.join_public(Uuid::nil(), nick(1), id(10), 1),
        Err(RoomError::Invalid)
    );
    let joined = rooms.join_public(id(2), nick(2), id(10), 1).unwrap();
    assert_eq!(
        rooms.join_invite(id(2), nick(2), code(0), 1).unwrap(),
        joined
    );
    assert_eq!(
        rooms.create(id(3), nick(3), id(12), code(1), valid(), 1),
        Err(RoomError::Capacity)
    );
    assert_eq!(
        rooms.leave(id(3), id(10), &joined.revision, 1),
        Err(RoomError::NotFound)
    );
    assert_eq!(
        rooms.kick(id(1), id(10), &joined.revision, 0, 1),
        Err(RoomError::Forbidden)
    );
    assert_eq!(
        rooms.kick(id(1), id(10), &joined.revision, 3, 1),
        Err(RoomError::NotFound)
    );
    assert_eq!(
        rooms.start(id(1), id(10), &joined.revision, Uuid::nil(), 1),
        Err(RoomError::Invalid)
    );
    assert_eq!(rooms.advance(u64::MAX), Err(RoomError::Invalid));
    assert_eq!(rooms.advance(0), Err(RoomError::Invalid));
    let changed = rooms
        .configure(
            id(1),
            id(10),
            &joined.revision,
            settings("New name", RoomVisibility::Invite, 3),
            2,
        )
        .unwrap();
    assert_eq!(changed.options.title, "New name");
    assert!(rooms.list(None, 2).unwrap().rooms.is_empty());
    rooms.leave(id(2), id(10), &changed.revision, 3).unwrap();
    let last = rooms.status(id(1), 3).unwrap().unwrap();
    rooms.leave(id(1), id(10), &last.revision, 3).unwrap();
    assert_eq!(
        rooms.join_invite(id(3), nick(3), code(0), 3),
        Err(RoomError::NotFound)
    );
}

#[test]
fn public_room_pages_are_bounded_and_use_an_opaque_id_cursor() {
    use liar_protocol::rooms::{ROOM_CODE_ALPHABET, RoomListCursor};
    let mut rooms = RoomPolicy::new(30).unwrap();
    for (i, &letter) in ROOM_CODE_ALPHABET.iter().take(25).enumerate() {
        let candidate = format!(
            "ABCDEF{}{}",
            char::from(letter),
            char::from(ROOM_CODE_ALPHABET[31])
        );
        rooms
            .create(
                id(i as u128 + 1),
                nick(i as u128 + 1),
                id(i as u128 + 100),
                RoomCode::parse(&candidate).unwrap(),
                settings("Open", RoomVisibility::Public, 2),
                0,
            )
            .unwrap();
    }
    let first = rooms.list(None, 0).unwrap();
    assert_eq!(first.rooms.len(), 20);
    let second = rooms.list(first.next.as_ref(), 0).unwrap();
    assert_eq!(second.rooms.len(), 5);
    assert!(second.next.is_none());
    assert!(first.rooms.iter().all(|older| {
        !second
            .rooms
            .iter()
            .any(|newer| newer.room_id == older.room_id)
    }));
    assert_eq!(
        rooms.list(
            Some(&RoomListCursor {
                accepting: true,
                room_id: "invalid".into()
            }),
            0
        ),
        Err(RoomError::Invalid)
    );
}

#[test]
fn countdown_failure_reopens_the_same_waiting_room() {
    let mut rooms = RoomPolicy::new(8).unwrap();
    let room = id(10);
    rooms
        .create(
            id(1),
            nick(1),
            room,
            code(0),
            settings("Rerun", RoomVisibility::Public, 4),
            0,
        )
        .unwrap();
    let joined = rooms.join_public(id(2), nick(2), room, 0).unwrap();
    let one = rooms.ready(id(1), room, &joined.revision, true, 1).unwrap();
    let two = rooms.ready(id(2), room, &one.revision, true, 1).unwrap();
    let reservation = rooms.start(id(1), room, &two.revision, id(20), 1).unwrap();
    rooms.admit(reservation.key, 2).unwrap();
    let loading = rooms.status(id(1), 2).unwrap().unwrap();
    assert_eq!(loading.phase, RoomPhase::Countdown);
    assert_eq!(loading.round.unwrap().round_id, id(20).to_string());
    assert_eq!(
        rooms.join_public(id(3), nick(3), room, 2),
        Err(RoomError::Busy)
    );
    assert_eq!(rooms.finish(reservation.key, 2), Err(RoomError::Stale));
    rooms.cancel_countdown(reservation.key, 3).unwrap();
    assert_eq!(rooms.begin_round(reservation.key, 3), Err(RoomError::Stale));
    let waiting = rooms.status(id(1), 3).unwrap().unwrap();
    assert_eq!(waiting.phase, RoomPhase::Waiting);
    assert!(waiting.round.is_none());
    assert!(waiting.members.iter().all(|member| !member.ready));
    assert_eq!(waiting.room_id, loading.room_id);
}
