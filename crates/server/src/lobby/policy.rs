use liar_core::bot::Difficulty;
use std::collections::{HashMap, VecDeque};
use uuid::Uuid;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RoomCode([u8; 8]);
impl RoomCode {
    pub fn parse(value: &str) -> Result<Self, LobbyError> {
        let bytes: [u8; 8] = value
            .as_bytes()
            .try_into()
            .map_err(|_| LobbyError::Invalid)?;
        if !bytes
            .iter()
            .all(|b| b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789".contains(b))
        {
            return Err(LobbyError::Invalid);
        }
        Ok(Self(bytes))
    }
    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&self.0).expect("Validated ASCII")
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LobbyError {
    Invalid,
    InvalidTime,
    Busy,
    NotFound,
    Full,
    Collision,
    Capacity,
    Stale,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ReservationKey {
    pub entity: Uuid,
    pub generation: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpponentKind {
    Human,
    Bot,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct MatchReservation {
    pub key: ReservationKey,
    pub players: [Option<Uuid>; 2],
    pub difficulty: Difficulty,
    pub opponent: OpponentKind,
    pub expires_at: u64,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LobbyStatus {
    Queued {
        id: Uuid,
        deadline: u64,
        difficulty: Difficulty,
    },
    Room {
        id: Uuid,
        code: RoomCode,
        own_seat: usize,
        occupied: [bool; 2],
        ready: [bool; 2],
        expires_at: u64,
    },
    Preparing {
        key: ReservationKey,
        opponent: OpponentKind,
    },
}
#[derive(Clone, Copy)]
struct Ticket {
    id: Uuid,
    account: Uuid,
    deadline: u64,
    difficulty: Difficulty,
    order: u64,
}
struct Room {
    id: Uuid,
    code: RoomCode,
    players: [Option<Uuid>; 2],
    ready: [bool; 2],
    expires_at: u64,
    reservation: Option<ReservationKey>,
}
enum Source {
    Queue(Vec<Ticket>),
    Room(RoomCode),
}
struct Reservation {
    value: MatchReservation,
    source: Source,
}
pub struct LobbyState {
    capacity: usize,
    now: u64,
    order: u64,
    generation: u64,
    waiting: VecDeque<Ticket>,
    rooms: HashMap<RoomCode, Room>,
    starting: HashMap<ReservationKey, Reservation>,
}
impl LobbyState {
    pub fn new(capacity: usize) -> Result<Self, LobbyError> {
        if !(1..=4096).contains(&capacity) {
            return Err(LobbyError::Capacity);
        }
        Ok(Self {
            capacity,
            now: 0,
            order: 0,
            generation: 0,
            waiting: VecDeque::new(),
            rooms: HashMap::new(),
            starting: HashMap::new(),
        })
    }
    pub fn join(
        &mut self,
        account: Uuid,
        id: Uuid,
        difficulty: Difficulty,
        now: u64,
    ) -> Result<LobbyStatus, LobbyError> {
        self.tick(now)?;
        identity(account, id)?;
        if let Some(status) = self.status(account) {
            if matches!(status,LobbyStatus::Queued{id:old,difficulty:old_difficulty,..} if old==id && old_difficulty==difficulty)
            {
                return Ok(status);
            }
            return Err(LobbyError::Busy);
        }
        self.admit(id)?;
        let order = self.order.checked_add(1).ok_or(LobbyError::Capacity)?;
        let ticket = Ticket {
            id,
            account,
            deadline: now + 10000,
            difficulty,
            order,
        };
        if let Some(first) = self.waiting.front().copied() {
            self.reserve_queue(first, Some(ticket), now)?;
            self.waiting.pop_front();
        } else {
            self.waiting.push_back(ticket);
        }
        self.order = order;
        Ok(self
            .status(account)
            .expect("Admitted account has a location"))
    }
    pub fn create_room(
        &mut self,
        account: Uuid,
        id: Uuid,
        code: RoomCode,
        now: u64,
    ) -> Result<LobbyStatus, LobbyError> {
        self.tick(now)?;
        identity(account, id)?;
        if self
            .rooms
            .get(&code)
            .is_some_and(|room| room.id == id && room.players[0] == Some(account))
        {
            return Ok(self.status(account).expect("Room owner has a location"));
        }
        if self.status(account).is_some() {
            return Err(LobbyError::Busy);
        }
        if self.rooms.contains_key(&code) {
            return Err(LobbyError::Collision);
        }
        self.admit(id)?;
        self.rooms.insert(
            code,
            Room {
                id,
                code,
                players: [Some(account), None],
                ready: [false; 2],
                expires_at: now + 600000,
                reservation: None,
            },
        );
        Ok(self.status(account).expect("Room owner has a location"))
    }
    pub fn join_room(
        &mut self,
        account: Uuid,
        code: RoomCode,
        now: u64,
    ) -> Result<LobbyStatus, LobbyError> {
        self.tick(now)?;
        if account.is_nil() {
            return Err(LobbyError::Invalid);
        }
        let room = self.rooms.get(&code).ok_or(LobbyError::NotFound)?;
        if room.players.contains(&Some(account)) {
            return Ok(self.status(account).expect("Room member has a location"));
        }
        if self.status(account).is_some() {
            return Err(LobbyError::Busy);
        }
        if room.players[1].is_some() {
            return Err(LobbyError::Full);
        }
        if self.used() >= self.capacity {
            return Err(LobbyError::Capacity);
        }
        let room = self.rooms.get_mut(&code).expect("Checked room exists");
        room.players[1] = Some(account);
        Ok(self.status(account).expect("Room member has a location"))
    }
    pub fn ready(
        &mut self,
        account: Uuid,
        id: Uuid,
        ready: bool,
        now: u64,
    ) -> Result<LobbyStatus, LobbyError> {
        self.tick(now)?;
        let (code, seat) = self
            .rooms
            .iter()
            .find_map(|(code, room)| {
                (room.id == id)
                    .then(|| {
                        room.players
                            .iter()
                            .position(|p| *p == Some(account))
                            .map(|seat| (*code, seat))
                    })
                    .flatten()
            })
            .ok_or(LobbyError::NotFound)?;
        let room = self.rooms.get_mut(&code).expect("Member room exists");
        if !ready && let Some(key) = room.reservation.take() {
            self.starting.remove(&key);
        }
        room.ready[seat] = ready;
        if room.ready == [true; 2]
            && room.players.iter().all(Option::is_some)
            && room.reservation.is_none()
        {
            let players = room.players;
            let expires_at = room.expires_at.min(now + 5000);
            let key = self.key(id)?;
            self.starting.insert(
                key,
                Reservation {
                    value: MatchReservation {
                        key,
                        players,
                        difficulty: Difficulty::Normal,
                        opponent: OpponentKind::Human,
                        expires_at,
                    },
                    source: Source::Room(code),
                },
            );
            self.rooms
                .get_mut(&code)
                .expect("Member room exists")
                .reservation = Some(key);
        }
        Ok(self.status(account).expect("Room member has a location"))
    }
    pub fn cancel(&mut self, account: Uuid, now: u64) -> Result<(), LobbyError> {
        self.tick(now)?;
        self.waiting.retain(|ticket| ticket.account != account);
        if let Some(code) = self
            .rooms
            .iter()
            .find_map(|(code, room)| room.players.contains(&Some(account)).then_some(*code))
        {
            let room = self.rooms.get_mut(&code).expect("Member room exists");
            if let Some(key) = room.reservation.take() {
                self.starting.remove(&key);
            }
            room.players = if room.players[0] == Some(account) {
                [room.players[1], None]
            } else {
                [room.players[0], None]
            };
            room.ready = [false; 2];
            if room.players[0].is_none() {
                self.rooms.remove(&code);
            }
        } else if let Some(key) = self
            .starting
            .iter()
            .find_map(|(key, res)| res.value.players.contains(&Some(account)).then_some(*key))
        {
            let res = self.starting.remove(&key).expect("Reservation exists");
            if let Source::Queue(tickets) = res.source {
                for ticket in tickets
                    .into_iter()
                    .filter(|ticket| ticket.account != account)
                {
                    self.waiting.push_back(ticket);
                }
                self.waiting
                    .make_contiguous()
                    .sort_by_key(|ticket| ticket.order);
            }
        }
        self.tick(now)?;
        Ok(())
    }
    pub fn tick(&mut self, now: u64) -> Result<(), LobbyError> {
        if now < self.now || now > u64::MAX - 600000 {
            return Err(LobbyError::InvalidTime);
        }
        self.now = now;
        let expired: Vec<_> = self
            .starting
            .iter()
            .filter_map(|(key, res)| (now >= res.value.expires_at).then_some(*key))
            .collect();
        for key in expired {
            let res = self
                .starting
                .remove(&key)
                .expect("Expired reservation exists");
            if let Source::Room(code) = res.source {
                self.rooms.remove(&code);
            }
        }
        self.rooms.retain(|_, room| now < room.expires_at);
        while let Some(ticket) = self.waiting.front().copied() {
            if now < ticket.deadline {
                break;
            }
            self.reserve_queue(ticket, None, now)?;
            self.waiting.pop_front();
        }
        while self.waiting.len() >= 2 {
            self.reserve_queue(self.waiting[0], Some(self.waiting[1]), now)?;
            self.waiting.pop_front();
            self.waiting.pop_front();
        }
        Ok(())
    }
    pub fn status(&self, account: Uuid) -> Option<LobbyStatus> {
        if let Some(res) = self
            .starting
            .values()
            .find(|res| res.value.players.contains(&Some(account)))
        {
            return Some(LobbyStatus::Preparing {
                key: res.value.key,
                opponent: res.value.opponent,
            });
        }
        if let Some(ticket) = self.waiting.iter().find(|ticket| ticket.account == account) {
            return Some(LobbyStatus::Queued {
                id: ticket.id,
                deadline: ticket.deadline,
                difficulty: ticket.difficulty,
            });
        }
        self.rooms.values().find_map(|room| {
            room.players
                .iter()
                .position(|p| *p == Some(account))
                .map(|own_seat| LobbyStatus::Room {
                    id: room.id,
                    code: room.code,
                    own_seat,
                    occupied: room.players.map(|p| p.is_some()),
                    ready: room.ready,
                    expires_at: room.expires_at,
                })
        })
    }
    pub fn reservations(&self) -> Vec<MatchReservation> {
        self.starting.values().map(|res| res.value).collect()
    }
    pub fn commit(
        &mut self,
        key: ReservationKey,
        now: u64,
    ) -> Result<MatchReservation, LobbyError> {
        self.tick(now)?;
        let reservation = self.starting.remove(&key).ok_or(LobbyError::Stale)?;
        if let Source::Room(code) = reservation.source {
            self.rooms.remove(&code);
        }
        Ok(reservation.value)
    }
    fn key(&mut self, entity: Uuid) -> Result<ReservationKey, LobbyError> {
        self.generation = self.generation.checked_add(1).ok_or(LobbyError::Capacity)?;
        Ok(ReservationKey {
            entity,
            generation: self.generation,
        })
    }
    fn reserve_queue(
        &mut self,
        first: Ticket,
        second: Option<Ticket>,
        now: u64,
    ) -> Result<(), LobbyError> {
        let key = self.key(first.id)?;
        let mut tickets = vec![first];
        if let Some(ticket) = second {
            tickets.push(ticket);
        }
        self.starting.insert(
            key,
            Reservation {
                value: MatchReservation {
                    key,
                    players: [Some(first.account), second.map(|ticket| ticket.account)],
                    difficulty: first.difficulty,
                    opponent: if second.is_some() {
                        OpponentKind::Human
                    } else {
                        OpponentKind::Bot
                    },
                    expires_at: now + 5000,
                },
                source: Source::Queue(tickets),
            },
        );
        Ok(())
    }
    fn used(&self) -> usize {
        self.waiting.len()
            + self
                .rooms
                .values()
                .map(|r| r.players.iter().flatten().count())
                .sum::<usize>()
            + self
                .starting
                .values()
                .filter(|res| matches!(res.source, Source::Queue(_)))
                .map(|res| res.value.players.iter().flatten().count())
                .sum::<usize>()
    }
    fn admit(&self, id: Uuid) -> Result<(), LobbyError> {
        if self.used() >= self.capacity {
            return Err(LobbyError::Capacity);
        }
        if self.waiting.iter().any(|t| t.id == id)
            || self.rooms.values().any(|r| r.id == id)
            || self.starting.iter().any(|(key, res)| {
                key.entity == id
                    || match &res.source {
                        Source::Queue(tickets) => tickets.iter().any(|t| t.id == id),
                        Source::Room(_) => false,
                    }
            })
        {
            return Err(LobbyError::Collision);
        }
        Ok(())
    }
}
fn identity(account: Uuid, id: Uuid) -> Result<(), LobbyError> {
    if account.is_nil() || id.is_nil() {
        Err(LobbyError::Invalid)
    } else {
        Ok(())
    }
}
