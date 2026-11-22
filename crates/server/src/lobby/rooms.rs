use super::{LobbyAuthentication, LobbyService, RoomCodeSource};
use crate::{
    auth::Nickname,
    online::ConnectionAuthority,
    room_code::RoomCode,
    rooms::{RoomError, RoomPolicy, RoomSettings, RoundKey},
};
use liar_protocol::{
    online::OnlineError,
    rooms::{RoomEntrance, RoomIntent, RoomList, RoomListCursor, RoomView, RoomWireError},
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::watch;
use uuid::Uuid;

const REQUESTS_PER_SESSION: usize = 32;
const MAX_REQUESTS: usize = 4096;
const REQUEST_LIFETIME_MS: u64 = 120_000;

type Response = Result<Option<RoomView>, RoomWireError>;

struct Reply {
    intent: RoomIntent,
    response: Response,
    expires_at: u64,
}

pub(super) struct RoomState {
    pub(super) policy: RoomPolicy,
    pub(super) leases: HashMap<Uuid, ConnectionAuthority>,
    pub(super) jobs: HashMap<RoundKey, watch::Sender<bool>>,
    pub(super) failures: HashMap<Uuid, RoomWireError>,
    replies: HashMap<([u8; 32], Uuid), Reply>,
}

impl RoomState {
    pub(super) fn new(capacity: usize) -> Result<Self, OnlineError> {
        Ok(Self {
            policy: RoomPolicy::new(capacity).map_err(|_| OnlineError::Capacity)?,
            leases: HashMap::new(),
            jobs: HashMap::new(),
            failures: HashMap::new(),
            replies: HashMap::new(),
        })
    }

    pub(super) fn contains(&self, account: Uuid) -> bool {
        self.policy.contains(account)
    }

    pub(super) fn members(&self) -> usize {
        self.policy.members()
    }

    pub(super) fn advance(
        &mut self,
        authentication: &LobbyAuthentication,
        now: u64,
    ) -> Result<(), OnlineError> {
        self.policy
            .advance(now)
            .map_err(|_| OnlineError::Unavailable)?;
        if !self.leases.is_empty() {
            let time = authentication.clock.now();
            for (account, lease) in &self.leases {
                if self.policy.contains(*account)
                    && authentication
                        .authorities
                        .with_authority(lease, time, || ())
                        .is_err()
                {
                    self.policy.evict(*account, now);
                }
            }
            self.release_inactive();
        }
        self.replies.retain(|_, reply| now < reply.expires_at);
        self.jobs.retain(|key, _| self.policy.is_preparing(*key));
        self.failures.retain(|room, _| self.policy.has_room(*room));
        Ok(())
    }

    fn current(&self, account: Uuid) -> Option<RoomView> {
        let mut room = self.policy.current(account)?;
        room.start_error = id(&room.room_id)
            .ok()
            .and_then(|id| self.failures.get(&id).copied());
        Some(room)
    }

    fn release_inactive(&mut self) {
        self.leases
            .retain(|account, _| self.policy.contains(*account));
    }

    fn change(
        &mut self,
        authority: &ConnectionAuthority,
        nickname: &Nickname,
        intent: RoomIntent,
        entry: Result<(), RoomWireError>,
        codes: &dyn RoomCodeSource,
        now: u64,
    ) -> Response {
        let account = authority.account();
        let request = match &intent {
            RoomIntent::Status => return Ok(self.current(account)),
            RoomIntent::List { .. } => return Err(RoomWireError::Malformed),
            RoomIntent::Create { request_id, .. }
            | RoomIntent::Join { request_id, .. }
            | RoomIntent::Ready { request_id, .. }
            | RoomIntent::Configure { request_id, .. }
            | RoomIntent::Start { request_id, .. }
            | RoomIntent::Kick { request_id, .. }
            | RoomIntent::Leave { request_id, .. } => id(request_id)?,
        };
        let session = authority.session_hash();
        if let Some(saved) = self.replies.get(&(session, request)) {
            if saved.intent != intent {
                return Err(RoomWireError::Conflict);
            }
            if matches!(&saved.response, Ok(Some(room)) if self.policy.current(account).is_none_or(|current| current.room_id != room.room_id))
            {
                return Err(RoomWireError::NotFound);
            }
            return saved.response.clone();
        }
        if self.replies.len() >= MAX_REQUESTS
            || self
                .replies
                .keys()
                .filter(|(owner, _)| *owner == session)
                .count()
                >= REQUESTS_PER_SESSION
        {
            return Err(RoomWireError::RateLimited);
        }
        let result = self.apply(account, nickname, &intent, entry, codes, now);
        if self.policy.contains(account) {
            self.leases.insert(account, authority.clone());
        }
        self.replies.insert(
            (session, request),
            Reply {
                intent,
                response: result.clone(),
                expires_at: now.saturating_add(REQUEST_LIFETIME_MS),
            },
        );
        result
    }

    fn apply(
        &mut self,
        account: Uuid,
        nickname: &Nickname,
        intent: &RoomIntent,
        entry: Result<(), RoomWireError>,
        codes: &dyn RoomCodeSource,
        now: u64,
    ) -> Response {
        let policy = &mut self.policy;
        let view = match intent {
            RoomIntent::Create { options, .. } => {
                entry?;
                let settings = settings(options)?;
                let id = Uuid::new_v4();
                for _ in 0..8 {
                    let code = codes.code().map_err(online_error)?;
                    match policy.create(account, nickname.clone(), id, code, settings.clone(), now)
                    {
                        Err(RoomError::Conflict) => continue,
                        result => return result.map(Some).map_err(room_error),
                    }
                }
                return Err(RoomWireError::Unavailable);
            }
            RoomIntent::Join { entrance, .. } => {
                entry?;
                match entrance {
                    RoomEntrance::Public { room_id } => {
                        policy.join_public(account, nickname.clone(), id(room_id)?, now)
                    }
                    RoomEntrance::Invite { code } => policy.join_invite(
                        account,
                        nickname.clone(),
                        RoomCode::parse(code).map_err(|_| RoomWireError::Malformed)?,
                        now,
                    ),
                }
            }
            RoomIntent::Ready {
                room_id,
                revision,
                ready,
                ..
            } => policy.ready(account, id(room_id)?, revision, *ready, now),
            RoomIntent::Configure {
                room_id,
                revision,
                options,
                ..
            } => policy.configure(account, id(room_id)?, revision, settings(options)?, now),
            RoomIntent::Kick {
                room_id,
                revision,
                seat,
                ..
            } => policy.kick(account, id(room_id)?, revision, *seat, now),
            RoomIntent::Leave {
                room_id, revision, ..
            } => {
                return policy
                    .leave(account, id(room_id)?, revision, now)
                    .map(|()| None)
                    .map_err(room_error);
            }
            RoomIntent::Start {
                room_id, revision, ..
            } => {
                let room_id = id(room_id)?;
                policy
                    .start(account, room_id, revision, Uuid::new_v4(), now)
                    .map_err(room_error)?;
                self.failures.remove(&room_id);
                return Ok(self.current(account));
            }
            RoomIntent::List { .. } | RoomIntent::Status => return Err(RoomWireError::Malformed),
        };
        view.map(Some).map_err(room_error)
    }
}

impl LobbyService {
    pub fn room_list(&self, cursor: Option<&RoomListCursor>) -> Result<RoomList, RoomWireError> {
        let mut inner = self.inner.lock().map_err(|_| RoomWireError::Unavailable)?;
        if inner.draining {
            return Err(RoomWireError::Unavailable);
        }
        let now = self.now_ms();
        inner
            .rooms
            .advance(&self.authentication, now)
            .map_err(online_error)?;
        inner.rooms.policy.list(cursor, now).map_err(room_error)
    }

    pub fn room_command(
        self: &Arc<Self>,
        authority: &ConnectionAuthority,
        nickname: &Nickname,
        intent: RoomIntent,
    ) -> Response {
        let mut inner = self.inner.lock().map_err(|_| RoomWireError::Unavailable)?;
        if inner.draining {
            return Err(RoomWireError::Unavailable);
        }
        let now = self.now_ms();
        self.advance(&mut inner, now)
            .map_err(|_| RoomWireError::Unavailable)?;
        let account = authority.account();
        let entry = if inner.policy.status(account).is_some() {
            Err(RoomWireError::Busy)
        } else if !inner.rooms.contains(account)
            && inner.policy.used() + inner.rooms.members() >= self.limits.capacity
        {
            Err(RoomWireError::Capacity)
        } else if !inner.rooms.contains(account)
            && self.registry.is_occupied(account).map_err(online_error)?
        {
            Err(RoomWireError::Busy)
        } else {
            Ok(())
        };
        let starting = match &intent {
            RoomIntent::Start { request_id, .. } => Some(id(request_id)?),
            _ => None,
        };
        let result = self
            .authentication
            .authorities
            .with_authority(authority, self.authentication.clock.now(), || {
                inner
                    .rooms
                    .change(authority, nickname, intent, entry, self.codes.as_ref(), now)
            })
            .map_err(online_error)?;
        inner.rooms.release_inactive();
        if let Some(request) = starting
            && result.is_ok()
            && let Some(prepared) = inner.rooms.policy.preparation(account)
            && !inner.rooms.jobs.contains_key(&prepared.key)
        {
            let key = prepared.key;
            if let Err(cause) = self.prepare_round(&mut inner.rooms, prepared) {
                let error = online_error(cause);
                let _ = inner.rooms.policy.fail_preparation(key, now);
                if let Some(reply) = inner
                    .rooms
                    .replies
                    .get_mut(&(authority.session_hash(), request))
                {
                    reply.response = Err(error);
                }
                return Err(error);
            }
        }
        result
    }
}

fn id(value: &str) -> Result<Uuid, RoomWireError> {
    let id = Uuid::parse_str(value).map_err(|_| RoomWireError::Malformed)?;
    if id.is_nil() || id.to_string() != value {
        return Err(RoomWireError::Malformed);
    }
    Ok(id)
}

fn settings(options: &liar_protocol::rooms::RoomOptions) -> Result<RoomSettings, RoomWireError> {
    RoomSettings::new(&options.title, options.visibility, options.capacity).map_err(room_error)
}

fn room_error(error: RoomError) -> RoomWireError {
    match error {
        RoomError::Invalid => RoomWireError::Malformed,
        RoomError::NotFound => RoomWireError::NotFound,
        RoomError::Forbidden => RoomWireError::Forbidden,
        RoomError::Full => RoomWireError::Full,
        RoomError::Busy => RoomWireError::Busy,
        RoomError::Capacity => RoomWireError::Capacity,
        RoomError::Conflict => RoomWireError::Conflict,
        RoomError::Stale => RoomWireError::Stale,
        RoomError::NotReady => RoomWireError::NotReady,
    }
}

pub(super) fn online_error(error: OnlineError) -> RoomWireError {
    match error {
        OnlineError::Unauthorized => RoomWireError::Unauthorized,
        OnlineError::Capacity => RoomWireError::Capacity,
        OnlineError::RateLimited => RoomWireError::RateLimited,
        _ => RoomWireError::Unavailable,
    }
}
