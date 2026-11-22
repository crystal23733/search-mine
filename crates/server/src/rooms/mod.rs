//! Room membership and round reservations. I/O and session leases live in the service layer.
mod projection;
use crate::{auth::Nickname, room_code::RoomCode};
use liar_core::game::multiplayer::{MAX_RACE_PLAYERS, MIN_RACE_PLAYERS};
use liar_protocol::rooms::{
    MAX_ROOM_TITLE_BYTES, RoomList, RoomListCursor, RoomOptions, RoomPhase, RoomView,
    RoomVisibility,
};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use uuid::Uuid;

const IDLE_MS: u64 = 600_000;
const PREPARE_MS: u64 = 5_000;
const MAX_EXCLUSIONS: usize = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RoomError {
    Invalid,
    NotFound,
    Forbidden,
    Full,
    Busy,
    Capacity,
    Conflict,
    Stale,
    NotReady,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoomSettings(RoomOptions);

impl RoomSettings {
    pub fn new(title: &str, visibility: RoomVisibility, capacity: u8) -> Result<Self, RoomError> {
        if !(MIN_RACE_PLAYERS..=MAX_RACE_PLAYERS).contains(&usize::from(capacity)) {
            return Err(RoomError::Invalid);
        }
        let title = crate::game_name::normalize(title, 32, MAX_ROOM_TITLE_BYTES)
            .ok_or(RoomError::Invalid)?;
        Ok(Self(RoomOptions {
            title,
            visibility,
            capacity,
        }))
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct RoundKey {
    pub room_id: Uuid,
    pub round_id: Uuid,
    pub generation: u64,
}

// The accounts identify real authentication leases and are never serialized as a room view.
#[derive(Clone, Eq, PartialEq)]
pub struct RoundReservation {
    pub key: RoundKey,
    pub accounts: Vec<Uuid>,
    pub expires_at_ms: u64,
}
impl std::fmt::Debug for RoundReservation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RoundReservation")
            .field("key", &self.key)
            .field("seat_count", &self.accounts.len())
            .field("expires_at_ms", &self.expires_at_ms)
            .finish()
    }
}

struct Member {
    account: Uuid,
    seat: u8,
    nickname: Nickname,
    ready: bool,
    round_generation: Option<u64>,
}

enum Phase {
    Waiting,
    Preparing(RoundReservation),
    Countdown(RoundReservation),
    Playing(RoundReservation),
    Results(RoundReservation),
}

struct Room {
    id: Uuid,
    code: RoomCode,
    settings: RoomSettings,
    members: Vec<Member>,
    removed: BTreeSet<Uuid>,
    revision: u64,
    last_change: u64,
    phase: Phase,
}

impl Room {
    fn touch(&mut self, now: u64) {
        self.revision += 1;
        self.last_change = now;
    }

    fn clear_ready(&mut self) {
        for member in &mut self.members {
            member.ready = false;
        }
    }

    fn expires_at(&self) -> Option<u64> {
        if matches!(self.phase, Phase::Playing(_)) {
            None
        } else {
            Some(self.last_change + IDLE_MS)
        }
    }

    fn phase(&self) -> RoomPhase {
        match self.phase {
            Phase::Waiting => RoomPhase::Waiting,
            Phase::Preparing(_) => RoomPhase::Preparing,
            Phase::Countdown(_) => RoomPhase::Countdown,
            Phase::Playing(_) => RoomPhase::Playing,
            Phase::Results(_) => RoomPhase::Results,
        }
    }

    fn accepting(&self) -> bool {
        !matches!(self.phase, Phase::Preparing(_) | Phase::Countdown(_))
            && self.members.len() < usize::from(self.settings.0.capacity)
    }

    fn require_host(&self, caller: Uuid) -> Result<(), RoomError> {
        if self
            .members
            .first()
            .is_some_and(|member| member.account == caller)
        {
            Ok(())
        } else {
            Err(RoomError::Forbidden)
        }
    }

    fn validate_revision(&self, revision: &str) -> Result<(), RoomError> {
        if self.revision.to_string() == revision {
            Ok(())
        } else {
            Err(RoomError::Stale)
        }
    }

    fn next_member_seat(&self) -> Result<u8, RoomError> {
        // A room seat identifies a member. Round seats are allocated from the frozen roster.
        (0..MAX_RACE_PLAYERS as u8)
            .find(|seat| self.members.iter().all(|member| member.seat != *seat))
            .ok_or(RoomError::Full)
    }

    fn reset_preparation(&mut self) {
        if matches!(self.phase, Phase::Preparing(_) | Phase::Countdown(_)) {
            self.phase = Phase::Waiting;
        }
        self.clear_ready();
    }
}

pub struct RoomPolicy {
    rooms: BTreeMap<Uuid, Room>,
    membership: HashMap<Uuid, Uuid>,
    capacity: usize,
    generation: u64,
    now: u64,
}

impl RoomPolicy {
    pub fn new(capacity: usize) -> Result<Self, RoomError> {
        if !(1..=4096).contains(&capacity) {
            return Err(RoomError::Capacity);
        }
        Ok(Self {
            rooms: BTreeMap::new(),
            membership: HashMap::new(),
            capacity,
            generation: 0,
            now: 0,
        })
    }

    pub fn advance(&mut self, now: u64) -> Result<(), RoomError> {
        if now < self.now || now.checked_add(IDLE_MS).is_none() {
            return Err(RoomError::Invalid);
        }
        self.now = now;
        let membership = &mut self.membership;
        self.rooms.retain(|_, room| {
            if room.expires_at().is_some_and(|at| now >= at) {
                for member in &room.members {
                    membership.remove(&member.account);
                }
                return false;
            }
            if matches!(&room.phase, Phase::Preparing(prepared) if now >= prepared.expires_at_ms) {
                room.phase = Phase::Waiting;
                room.clear_ready();
                room.revision += 1;
            }
            true
        });
        Ok(())
    }

    pub fn status(&mut self, caller: Uuid, now: u64) -> Result<Option<RoomView>, RoomError> {
        self.advance(now)?;
        Ok(self.current(caller))
    }

    pub(crate) fn current(&self, caller: Uuid) -> Option<RoomView> {
        self.membership
            .get(&caller)
            .and_then(|id| self.rooms.get(id))
            .map(|room| room.view(caller))
    }

    pub(crate) fn contains(&self, account: Uuid) -> bool {
        self.membership.contains_key(&account)
    }

    pub(crate) fn members(&self) -> usize {
        self.membership.len()
    }

    pub(crate) fn has_room(&self, id: Uuid) -> bool {
        self.rooms.contains_key(&id)
    }

    pub(crate) fn preparation(&self, account: Uuid) -> Option<RoundReservation> {
        let room = self.rooms.get(self.membership.get(&account)?)?;
        match &room.phase {
            Phase::Preparing(reservation) => Some(reservation.clone()),
            _ => None,
        }
    }

    pub(crate) fn is_preparing(&self, key: RoundKey) -> bool {
        self.rooms.get(&key.room_id).is_some_and(
            |room| matches!(&room.phase, Phase::Preparing(reservation) if reservation.key == key),
        )
    }

    pub(crate) fn names(&self, reservation: &RoundReservation) -> Option<Vec<Nickname>> {
        let room = self.rooms.get(&reservation.key.room_id)?;
        reservation
            .accounts
            .iter()
            .map(|account| {
                room.members
                    .iter()
                    .find(|member| member.account == *account)
                    .map(|member| member.nickname.clone())
            })
            .collect()
    }

    pub(crate) fn round_member(&self, key: RoundKey, account: Uuid) -> bool {
        self.rooms.get(&key.room_id).is_some_and(|room| {
            matches!(&room.phase, Phase::Countdown(round) | Phase::Playing(round) if round.key == key)
                && room.members.iter().any(|member| member.account == account && member.round_generation == Some(key.generation))
        })
    }

    pub fn list(
        &mut self,
        cursor: Option<&RoomListCursor>,
        now: u64,
    ) -> Result<RoomList, RoomError> {
        self.advance(now)?;
        projection::list(self.rooms.values(), cursor)
    }

    pub fn create(
        &mut self,
        caller: Uuid,
        nickname: Nickname,
        id: Uuid,
        code: RoomCode,
        settings: RoomSettings,
        now: u64,
    ) -> Result<RoomView, RoomError> {
        self.advance(now)?;
        if caller.is_nil() || id.is_nil() {
            return Err(RoomError::Invalid);
        }
        if self.membership.contains_key(&caller) {
            return Err(RoomError::Busy);
        }
        if self.rooms.contains_key(&id) || self.rooms.values().any(|room| room.code == code) {
            return Err(RoomError::Conflict);
        }
        self.check_capacity()?;
        let room = Room {
            id,
            code,
            settings,
            members: vec![Member {
                account: caller,
                seat: 0,
                nickname,
                ready: false,
                round_generation: None,
            }],
            removed: BTreeSet::new(),
            revision: 1,
            last_change: now,
            phase: Phase::Waiting,
        };
        let view = room.view(caller);
        self.membership.insert(caller, id);
        self.rooms.insert(id, room);
        Ok(view)
    }

    pub fn join_public(
        &mut self,
        caller: Uuid,
        nickname: Nickname,
        id: Uuid,
        now: u64,
    ) -> Result<RoomView, RoomError> {
        self.advance(now)?;
        if self
            .rooms
            .get(&id)
            .is_none_or(|room| room.settings.0.visibility != RoomVisibility::Public)
        {
            return Err(RoomError::NotFound);
        }
        self.join(caller, nickname, id, now)
    }

    pub fn join_invite(
        &mut self,
        caller: Uuid,
        nickname: Nickname,
        code: RoomCode,
        now: u64,
    ) -> Result<RoomView, RoomError> {
        self.advance(now)?;
        let id = self
            .rooms
            .values()
            .find(|room| room.code == code)
            .map(|room| room.id)
            .ok_or(RoomError::NotFound)?;
        self.join(caller, nickname, id, now)
    }

    fn join(
        &mut self,
        caller: Uuid,
        nickname: Nickname,
        id: Uuid,
        now: u64,
    ) -> Result<RoomView, RoomError> {
        if caller.is_nil() {
            return Err(RoomError::Invalid);
        }
        let room = self.rooms.get(&id).ok_or(RoomError::NotFound)?;
        if room.removed.contains(&caller) {
            return Err(RoomError::NotFound);
        }
        if let Some(&current) = self.membership.get(&caller) {
            return if current == id {
                Ok(room.view(caller))
            } else {
                Err(RoomError::Busy)
            };
        }
        if matches!(room.phase, Phase::Preparing(_) | Phase::Countdown(_)) {
            return Err(RoomError::Busy);
        }
        if !room.accepting() {
            return Err(RoomError::Full);
        }
        let seat = room.next_member_seat()?;
        self.check_capacity()?;
        let room = self.rooms.get_mut(&id).ok_or(RoomError::NotFound)?;
        room.members.push(Member {
            account: caller,
            seat,
            nickname,
            ready: false,
            round_generation: None,
        });
        room.clear_ready();
        room.touch(now);
        self.membership.insert(caller, id);
        Ok(room.view(caller))
    }

    pub fn ready(
        &mut self,
        caller: Uuid,
        id: Uuid,
        revision: &str,
        ready: bool,
        now: u64,
    ) -> Result<RoomView, RoomError> {
        self.advance(now)?;
        let room = self.member_room(caller, id, revision)?;
        if matches!(room.phase, Phase::Playing(_) | Phase::Countdown(_))
            || ready && matches!(room.phase, Phase::Preparing(_))
        {
            return Err(RoomError::Busy);
        }
        if matches!(room.phase, Phase::Preparing(_)) {
            room.reset_preparation();
            room.touch(now);
        } else {
            let member = room
                .members
                .iter_mut()
                .find(|member| member.account == caller)
                .ok_or(RoomError::NotFound)?;
            if member.ready != ready {
                member.ready = ready;
                if matches!(room.phase, Phase::Results(_)) {
                    room.phase = Phase::Waiting;
                }
                room.touch(now);
            }
        }
        Ok(room.view(caller))
    }

    pub fn configure(
        &mut self,
        caller: Uuid,
        id: Uuid,
        revision: &str,
        settings: RoomSettings,
        now: u64,
    ) -> Result<RoomView, RoomError> {
        self.advance(now)?;
        let room = self.member_room(caller, id, revision)?;
        room.require_host(caller)?;
        if !matches!(room.phase, Phase::Waiting | Phase::Results(_)) {
            return Err(RoomError::Busy);
        }
        if usize::from(settings.0.capacity) < room.members.len() {
            return Err(RoomError::Full);
        }
        if room.settings != settings {
            room.settings = settings;
            room.clear_ready();
            room.touch(now);
        }
        Ok(room.view(caller))
    }

    pub fn kick(
        &mut self,
        caller: Uuid,
        id: Uuid,
        revision: &str,
        seat: u8,
        now: u64,
    ) -> Result<RoomView, RoomError> {
        self.advance(now)?;
        let room = self.member_room(caller, id, revision)?;
        room.require_host(caller)?;
        if !matches!(room.phase, Phase::Waiting | Phase::Results(_)) {
            return Err(RoomError::Busy);
        }
        let target = room
            .members
            .iter()
            .find(|member| member.seat == seat)
            .ok_or(RoomError::NotFound)?
            .account;
        if target == caller {
            return Err(RoomError::Forbidden);
        }
        if room.removed.len() >= MAX_EXCLUSIONS {
            return Err(RoomError::Capacity);
        }
        room.removed.insert(target);
        room.members.retain(|member| member.account != target);
        room.clear_ready();
        room.touch(now);
        let view = room.view(caller);
        self.membership.remove(&target);
        Ok(view)
    }

    pub fn leave(
        &mut self,
        caller: Uuid,
        id: Uuid,
        revision: &str,
        now: u64,
    ) -> Result<(), RoomError> {
        self.advance(now)?;
        self.member_room(caller, id, revision)?;
        self.evict(caller, now);
        Ok(())
    }

    pub(crate) fn evict(&mut self, caller: Uuid, now: u64) {
        let Some(id) = self.membership.remove(&caller) else {
            return;
        };
        let Some(room) = self.rooms.get_mut(&id) else {
            return;
        };
        room.members.retain(|member| member.account != caller);
        room.reset_preparation();
        room.touch(now);
        if room.members.is_empty() {
            self.rooms.remove(&id);
        }
    }

    pub fn start(
        &mut self,
        caller: Uuid,
        id: Uuid,
        revision: &str,
        round_id: Uuid,
        now: u64,
    ) -> Result<RoundReservation, RoomError> {
        self.advance(now)?;
        if round_id.is_nil() {
            return Err(RoomError::Invalid);
        }
        let generation = self.generation.checked_add(1).ok_or(RoomError::Capacity)?;
        let room = self.member_room(caller, id, revision)?;
        room.require_host(caller)?;
        if !matches!(room.phase, Phase::Waiting) {
            return Err(RoomError::Busy);
        }
        if room.members.len() < MIN_RACE_PLAYERS || room.members.iter().any(|member| !member.ready)
        {
            return Err(RoomError::NotReady);
        }
        let reservation = RoundReservation {
            key: RoundKey {
                room_id: id,
                round_id,
                generation,
            },
            accounts: room.members.iter().map(|member| member.account).collect(),
            expires_at_ms: now + PREPARE_MS,
        };
        room.phase = Phase::Preparing(reservation.clone());
        room.touch(now);
        self.generation = generation;
        Ok(reservation)
    }

    pub fn admit(&mut self, key: RoundKey, now: u64) -> Result<(), RoomError> {
        self.advance(now)?;
        let room = self.rooms.get_mut(&key.room_id).ok_or(RoomError::Stale)?;
        let Phase::Preparing(prepared) = &room.phase else {
            return Err(RoomError::Stale);
        };
        if prepared.key != key {
            return Err(RoomError::Stale);
        }
        room.phase = Phase::Countdown(prepared.clone());
        for member in &mut room.members {
            member.round_generation = Some(key.generation);
        }
        room.clear_ready();
        room.touch(now);
        Ok(())
    }

    pub fn begin_round(&mut self, key: RoundKey, now: u64) -> Result<(), RoomError> {
        self.advance(now)?;
        let room = self.countdown_room(key)?;
        let Phase::Countdown(round) = &room.phase else {
            return Err(RoomError::Stale);
        };
        room.phase = Phase::Playing(round.clone());
        room.revision += 1;
        Ok(())
    }

    pub fn cancel_countdown(&mut self, key: RoundKey, now: u64) -> Result<(), RoomError> {
        self.advance(now)?;
        let room = self.countdown_room(key)?;
        room.reset_preparation();
        room.revision += 1;
        Ok(())
    }

    fn countdown_room(&mut self, key: RoundKey) -> Result<&mut Room, RoomError> {
        let room = self.rooms.get_mut(&key.room_id).ok_or(RoomError::Stale)?;
        if matches!(&room.phase, Phase::Countdown(round) if round.key == key) {
            Ok(room)
        } else {
            Err(RoomError::Stale)
        }
    }

    pub fn fail_preparation(&mut self, key: RoundKey, now: u64) -> Result<(), RoomError> {
        self.advance(now)?;
        let room = self.rooms.get_mut(&key.room_id).ok_or(RoomError::Stale)?;
        if !matches!(&room.phase, Phase::Preparing(prepared) if prepared.key == key) {
            return Err(RoomError::Stale);
        }
        room.reset_preparation();
        room.revision += 1;
        Ok(())
    }

    pub fn finish(&mut self, key: RoundKey, now: u64) -> Result<(), RoomError> {
        self.advance(now)?;
        let room = self.rooms.get_mut(&key.room_id).ok_or(RoomError::Stale)?;
        let Phase::Playing(round) = &room.phase else {
            return Err(RoomError::Stale);
        };
        if round.key != key {
            return Err(RoomError::Stale);
        }
        room.phase = Phase::Results(round.clone());
        room.clear_ready();
        room.touch(now);
        Ok(())
    }

    fn member_room(
        &mut self,
        caller: Uuid,
        id: Uuid,
        revision: &str,
    ) -> Result<&mut Room, RoomError> {
        if self.membership.get(&caller) != Some(&id) {
            return Err(RoomError::NotFound);
        }
        let room = self.rooms.get_mut(&id).ok_or(RoomError::NotFound)?;
        room.validate_revision(revision)?;
        Ok(room)
    }

    fn check_capacity(&self) -> Result<(), RoomError> {
        if self.membership.len() >= self.capacity {
            Err(RoomError::Capacity)
        } else {
            Ok(())
        }
    }
}
