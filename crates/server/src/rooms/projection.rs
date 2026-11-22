use super::{Phase, Room, RoomError};
use liar_protocol::rooms::{
    ROOM_PAGE_SIZE, RoomCard, RoomList, RoomListCursor, RoomMember, RoomRound, RoomView,
    RoomVisibility,
};
use uuid::Uuid;

impl Room {
    pub(super) fn view(&self, caller: Uuid) -> RoomView {
        let own = self
            .members
            .iter()
            .find(|member| member.account == caller)
            .expect("Member view is only available to a member");
        let roster = match &self.phase {
            Phase::Countdown(round) | Phase::Playing(round) | Phase::Results(round) => Some(round),
            Phase::Waiting | Phase::Preparing(_) => None,
        };
        RoomView {
            room_id: self.id.to_string(),
            revision: self.revision.to_string(),
            code: self.code.as_str().into(),
            options: self.settings.0.clone(),
            phase: self.phase(),
            own_seat: own.seat,
            host_seat: self.members[0].seat,
            members: self
                .members
                .iter()
                .map(|member| RoomMember {
                    seat: member.seat,
                    nickname: member.nickname.as_str().into(),
                    ready: member.ready,
                    in_round: roster
                        .is_some_and(|round| member.round_generation == Some(round.key.generation)),
                })
                .collect(),
            round: roster
                .filter(|round| own.round_generation == Some(round.key.generation))
                .and_then(|round| {
                    round
                        .accounts
                        .iter()
                        .position(|&account| account == caller)
                        .map(|seat| RoomRound {
                            round_id: round.key.round_id.to_string(),
                            seat: seat as u8,
                        })
                }),
            expires_at_ms: self.expires_at(),
            start_error: None,
        }
    }

    fn card(&self) -> RoomCard {
        RoomCard {
            room_id: self.id.to_string(),
            title: self.settings.0.title.clone(),
            capacity: self.settings.0.capacity,
            members: self.members.len() as u8,
            phase: self.phase(),
            accepting: self.accepting(),
        }
    }

    fn sort_key(&self) -> (bool, Uuid) {
        (!self.accepting(), self.id)
    }
}

pub(super) fn list<'a>(
    rooms: impl Iterator<Item = &'a Room>,
    cursor: Option<&RoomListCursor>,
) -> Result<RoomList, RoomError> {
    let after = cursor
        .map(|cursor| {
            let id = Uuid::parse_str(&cursor.room_id).map_err(|_| RoomError::Invalid)?;
            if id.is_nil() || id.to_string() != cursor.room_id {
                return Err(RoomError::Invalid);
            }
            Ok((!cursor.accepting, id))
        })
        .transpose()?;
    let mut matching: Vec<_> = rooms
        .filter(|room| {
            room.settings.0.visibility == RoomVisibility::Public
                && after.is_none_or(|key| room.sort_key() > key)
        })
        .collect();
    matching.sort_unstable_by_key(|room| room.sort_key());
    let rooms: Vec<_> = matching
        .iter()
        .take(ROOM_PAGE_SIZE)
        .map(|room| room.card())
        .collect();
    let next = rooms
        .last()
        .filter(|_| matching.len() > ROOM_PAGE_SIZE)
        .map(|card| RoomListCursor {
            accepting: card.accepting,
            room_id: card.room_id.clone(),
        });
    Ok(RoomList { rooms, next })
}
