//! Multiplayer views and lifecycle over the shared rule transitions.
use super::{
    Ack, AnalysisJob, AnalysisResult, Board, Command, EndReason, MatchEnd, OpponentView, OwnView,
    PreparedRound, Rejection, Round, RoundMode, RulesSnapshot,
};
use crate::random::RandomSource;
use std::cmp::Reverse;

pub const MIN_RACE_PLAYERS: usize = 2;
pub const MAX_RACE_PLAYERS: usize = 4;

pub struct PreparedRace(PreparedRound);
pub struct RaceEngine(Round);

impl PreparedRace {
    pub fn prepare(board: Board, rules: RulesSnapshot, count: usize) -> Result<Self, Rejection> {
        if !(MIN_RACE_PLAYERS..=MAX_RACE_PLAYERS).contains(&count) {
            return Err(Rejection::InvalidPlayerCount);
        }
        Round::prepare(board, rules, count).map(Self)
    }

    pub fn rules_hash(&self) -> &str {
        &self.0.rules.hash
    }

    pub fn start(
        self,
        now: u64,
        random: impl RandomSource + Send + Sync + 'static,
    ) -> Result<RaceEngine, Rejection> {
        self.0
            .start(now, RoundMode::Race(Box::new(random)))
            .map(RaceEngine)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RaceOpponent {
    pub seat: usize,
    pub progress: OpponentView,
    pub forfeited: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RaceProjection {
    pub rules: RulesSnapshot,
    pub own: OwnView,
    pub forfeited: bool,
    pub opponents: Vec<RaceOpponent>,
    pub revision: u64,
    pub countdown_ms: u32,
    pub remaining_ms: u32,
    pub result: Option<RaceResult>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Standing {
    pub seat: usize,
    pub rank: Option<u8>,
    pub opened_safe: u16,
    pub forfeited: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RaceResult {
    pub reason: EndReason,
    pub winner: Option<usize>,
    pub at: u64,
    pub standings: Vec<Standing>,
}

impl RaceResult {
    pub(super) fn from_round(round: &Round, end: MatchEnd<usize>) -> Self {
        let mut standings: Vec<_> = round
            .players
            .iter()
            .enumerate()
            .map(|(seat, player)| Standing {
                seat,
                rank: None,
                opened_safe: player.view.opened_safe() as u16,
                forfeited: player.forfeited,
            })
            .collect();

        let ranked = matches!(
            end.reason,
            EndReason::Clear | EndReason::Timeout | EndReason::Forfeit
        );
        let mut winner = None;
        if ranked {
            standings.sort_by_key(|entry| {
                (
                    entry.forfeited,
                    Some(entry.seat) != end.winner,
                    Reverse(entry.opened_safe),
                    entry.seat,
                )
            });
            let mut prior = None;
            for (index, entry) in standings
                .iter_mut()
                .take_while(|entry| !entry.forfeited)
                .enumerate()
            {
                let key = (Some(entry.seat) == end.winner, entry.opened_safe);
                let rank = match prior {
                    Some((other_key, rank)) if key == other_key => rank,
                    _ => (index + 1) as u8,
                };
                entry.rank = Some(rank);
                prior = Some((key, rank));
            }

            let mut leaders = standings.iter().filter(|entry| entry.rank == Some(1));
            if let Some(first) = leaders.next()
                && leaders.next().is_none()
            {
                winner = Some(first.seat);
            }
        }

        Self {
            reason: end.reason,
            winner,
            at: end.at,
            standings,
        }
    }
}

impl RaceEngine {
    pub fn start_ms(&self) -> u64 {
        self.0.start
    }

    pub fn last_sequence(&self, seat: usize) -> Option<u64> {
        self.0.players.get(seat).map(|player| player.last_seq)
    }

    pub fn projection(&self, seat: usize) -> Option<RaceProjection> {
        let round = &self.0;
        let player = round.players.get(seat)?;
        let at = round.projected_at();
        Some(RaceProjection {
            rules: round.rules.clone(),
            own: round.own_view(seat, at),
            forfeited: player.forfeited,
            opponents: (0..round.players.len())
                .filter(|&index| index != seat)
                .map(|index| RaceOpponent {
                    seat: index,
                    progress: round.opponent_view(index, at),
                    forfeited: round.players[index].forfeited,
                })
                .collect(),
            revision: round.public_revision[seat],
            countdown_ms: round.countdown_ms(at),
            remaining_ms: round.remaining_ms(at),
            result: round.race_result.clone(),
        })
    }

    pub fn result(&self) -> Option<&RaceResult> {
        self.0.race_result.as_ref()
    }

    pub fn apply(&mut self, command: Command<usize>) -> Ack {
        self.0.apply(command)
    }

    pub fn apply_batch(&mut self, commands: Vec<Command<usize>>) -> Vec<Ack> {
        self.0.apply_batch(commands)
    }

    pub fn proof_work(&self, seat: usize) -> Result<AnalysisJob, Rejection> {
        self.0.validate_seat(seat)?;
        Ok(self.0.proof_work(seat))
    }

    pub fn commit_proof(&mut self, result: AnalysisResult) -> Result<(), Rejection> {
        if self.0.end.is_some() {
            return Err(Rejection::Finished);
        }
        self.0.commit_proof(result)
    }

    pub fn advance(&mut self, now: u64) -> Result<(), Rejection> {
        self.0.advance(now)
    }

    pub fn disconnect(&mut self, seat: usize, now: u64) -> Result<(), Rejection> {
        self.0.disconnect(seat, now)
    }

    pub fn resume(&mut self, seat: usize, epoch: u32, now: u64) -> Result<(), Rejection> {
        self.0.resume(seat, epoch, now)?;
        self.0.settle_departures(now);
        Ok(())
    }

    pub fn forfeit(&mut self, seat: usize, now: u64) -> Result<(), Rejection> {
        self.0.validate_seat(seat)?;
        self.0.advance(now)?;
        if self.0.end.is_some() {
            return Err(Rejection::Finished);
        }
        if now < self.0.start {
            return self.0.cancel_before_start(now);
        }
        self.0.mark_forfeited(seat);
        self.0.settle_departures(now);
        Ok(())
    }

    pub fn cancel_before_start(&mut self, now: u64) -> Result<(), Rejection> {
        self.0.cancel_before_start(now)
    }

    pub fn abort(&mut self, now: u64) -> Result<(), Rejection> {
        self.0.abort(now)
    }
}

pub(super) fn draw_target(
    eligible: impl Iterator<Item = usize>,
    random: &mut (impl RandomSource + ?Sized),
) -> Option<usize> {
    let mut seats = [0; MAX_RACE_PLAYERS - 1];
    let mut count = 0;
    for (slot, seat) in seats.iter_mut().zip(eligible) {
        *slot = seat;
        count += 1;
    }
    // Limit retries on out-of-range random samples. There is at most one selected opponent.
    crate::random::sample_index(count, random).map(|index| seats[index])
}

impl Round {
    pub(super) fn advance_race(&mut self, now: u64) {
        let through = now.min(self.deadline);
        while let Some(expiry) = (0..self.players.len())
            .filter_map(|seat| self.reconnect_deadline(seat))
            .filter(|&expiry| expiry <= through)
            .min()
        {
            for seat in 0..self.players.len() {
                if self.reconnect_deadline(seat) == Some(expiry) {
                    self.mark_forfeited(seat);
                }
            }
            // Before the match deadline, expire tied leases together before deciding the outcome.
            if expiry < self.deadline {
                self.settle_departures(expiry);
                if self.end.is_some() {
                    return;
                }
            }
        }

        if now >= self.deadline {
            let reason = if self.players.iter().all(|player| player.forfeited) {
                EndReason::Abandoned
            } else {
                EndReason::Timeout
            };
            self.finish(reason, None, self.deadline);
        }
    }

    fn mark_forfeited(&mut self, seat: usize) {
        let player = &mut self.players[seat];
        if !player.forfeited {
            player.forfeited = true;
            player.disconnected_at = None;
            self.bump_all();
        }
    }

    fn settle_departures(&mut self, at: u64) {
        let mut active = self
            .players
            .iter()
            .enumerate()
            .filter(|(_, player)| !player.forfeited);
        match (active.next(), active.next()) {
            (None, _) => self.finish(EndReason::Abandoned, None, at),
            (Some((seat, player)), None) if player.disconnected_at.is_none() => {
                self.finish(EndReason::Forfeit, Some(seat), at);
            }
            _ => {}
        }
    }
}
