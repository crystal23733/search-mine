//! Pure authoritative transitions. Tick and serial ingress order are supplied by adapters.
use crate::{
    board::{Board, Cell, CellId, Observation, ObservedCell},
    policy::{PolicyKnowledge, PublicCellUpdate},
    random::RandomSource,
    rules::RulesSnapshot,
    solver::SolverBudget,
};
use std::collections::BTreeMap;

pub mod multiplayer;
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Seat {
    One,
    Two,
}
impl Seat {
    pub fn index(self) -> usize {
        match self {
            Self::One => 0,
            Self::Two => 1,
        }
    }
    pub fn other(self) -> Self {
        match self {
            Self::One => Self::Two,
            Self::Two => Self::One,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Open(CellId),
    ToggleFlag(CellId),
    Attack,
    Accuse(CellId),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Command<S = Seat> {
    pub id: u128,
    pub seq: u64,
    pub epoch: u32,
    pub seat: S,
    pub received_at: u64,
    pub action: Action,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rejection {
    InvalidBoard,
    InvalidRules,
    InvalidSeat,
    InvalidPlayerCount,
    InvalidTime,
    NotStarted,
    Finished,
    Disconnected,
    InvalidEpoch,
    InvalidSequence,
    CommandConflict,
    CommandLimit,
    InvalidCell,
    AlreadyOpen,
    Flagged,
    Stunned,
    AttackUnavailable,
    AnalysisStale,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionStatus {
    Applied,
    Rejected(Rejection),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ack {
    pub command_id: u128,
    pub revision: u64,
    pub status: ActionStatus,
    pub duplicate: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndReason {
    Clear,
    Timeout,
    Forfeit,
    Abandoned,
    ServerFailure,
    Cancelled,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MatchEnd<S = Seat> {
    pub reason: EndReason,
    pub winner: Option<S>,
    pub at: u64,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OwnView {
    pub cells: Observation,
    pub flags: Vec<bool>,
    pub gauge: u16,
    pub stun_ms: u32,
    pub history: Vec<Vec<PublicCellUpdate>>,
    pub stats: GameStats,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GameStats {
    pub mistakes: u16,
    pub accusation_attempts: u16,
    pub correct_accusations: u16,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpponentView {
    pub opened_safe: u16,
    pub stun_ms: u32,
    pub reconnect_ms: Option<u32>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Projection {
    pub rules: RulesSnapshot,
    pub own: OwnView,
    pub opponent: OpponentView,
    pub revision: u64,
    pub countdown_ms: u32,
    pub remaining_ms: u32,
    pub end: Option<MatchEnd>,
}
struct Overlay {
    target: CellId,
    truth: u8,
    shown: u8,
    source: usize,
}
struct Cached {
    seq: u64,
    action: Action,
    ack: Ack,
}
struct Player {
    view: Observation,
    flags: Vec<bool>,
    gauge: u16,
    stun_until: u64,
    overlays: Vec<Overlay>,
    knowledge: PolicyKnowledge,
    observation_revision: u64,
    epoch: u32,
    last_seq: u64,
    disconnected_at: Option<u64>,
    forfeited: bool,
    commands: BTreeMap<u128, Cached>,
    stats: GameStats,
}
enum RoundMode {
    Duel,
    Solo,
    Race(Box<dyn RandomSource + Send + Sync>),
}

// Keep private board, random source and overlays inside the shared rules engine.
struct Round {
    mode: RoundMode,
    board: Board,
    rules: RulesSnapshot,
    players: Vec<Player>,
    start: u64,
    deadline: u64,
    now: u64,
    public_revision: Vec<u64>,
    end: Option<MatchEnd<usize>>,
    race_result: Option<multiplayer::RaceResult>,
}

pub struct RuleEngine(Round);
pub struct PreparedEngine(PreparedRound);

pub struct AnalysisJob {
    seat: usize,
    revision: u64,
    knowledge: PolicyKnowledge,
}
pub struct AnalysisResult {
    seat: usize,
    revision: u64,
    knowledge: PolicyKnowledge,
}

struct PreparedRound {
    board: Board,
    rules: RulesSnapshot,
    players: Vec<Player>,
}

impl PreparedRound {
    fn start(self, now: u64, mode: RoundMode) -> Result<Round, Rejection> {
        let start = now
            .checked_add(u64::from(self.rules.rules.countdown_ms))
            .ok_or(Rejection::InvalidTime)?;
        let deadline = start
            .checked_add(u64::from(self.rules.rules.duration_ms))
            .ok_or(Rejection::InvalidTime)?;
        let public_revision = vec![0; self.players.len()];
        Ok(Round {
            mode,
            board: self.board,
            rules: self.rules,
            players: self.players,
            start,
            deadline,
            now,
            public_revision,
            end: None,
            race_result: None,
        })
    }
}
impl AnalysisJob {
    pub fn run(mut self) -> Result<AnalysisResult, Rejection> {
        self.knowledge
            .refresh()
            .map_err(|_| Rejection::InvalidBoard)?;
        Ok(AnalysisResult {
            seat: self.seat,
            revision: self.revision,
            knowledge: self.knowledge,
        })
    }
}
impl PreparedEngine {
    pub fn rules_hash(&self) -> &str {
        &self.0.rules.hash
    }

    pub fn start(self, now: u64) -> Result<RuleEngine, Rejection> {
        self.0.start(now, RoundMode::Duel).map(RuleEngine)
    }
}

impl RuleEngine {
    pub fn new(board: Board, rules: RulesSnapshot, now: u64) -> Result<Self, Rejection> {
        Self::prepare(board, rules)?.start(now)
    }

    pub fn prepare(board: Board, rules: RulesSnapshot) -> Result<PreparedEngine, Rejection> {
        Round::prepare(board, rules, 2).map(PreparedEngine)
    }

    pub fn new_solo(board: Board, rules: RulesSnapshot, now: u64) -> Result<Self, Rejection> {
        Round::prepare(board, rules, 2)?
            .start(now, RoundMode::Solo)
            .map(Self)
    }

    pub fn projection(&self, seat: Seat) -> Projection {
        let round = &self.0;
        let at = round.projected_at();
        Projection {
            rules: round.rules.clone(),
            own: round.own_view(seat.index(), at),
            opponent: round.opponent_view(seat.other().index(), at),
            revision: round.public_revision[seat.index()],
            countdown_ms: round.countdown_ms(at),
            remaining_ms: round.remaining_ms(at),
            end: round.end.map(|end| MatchEnd {
                reason: end.reason,
                winner: end.winner.map(|seat| [Seat::One, Seat::Two][seat]),
                at: end.at,
            }),
        }
    }

    pub fn last_sequence(&self, seat: Seat) -> u64 {
        self.0.players[seat.index()].last_seq
    }

    pub fn apply(&mut self, command: Command) -> Ack {
        self.0.apply(indexed_command(command))
    }

    pub fn apply_batch(&mut self, commands: Vec<Command>) -> Vec<Ack> {
        self.0
            .apply_batch(commands.into_iter().map(indexed_command).collect())
    }

    pub fn proof_work(&self, seat: Seat) -> AnalysisJob {
        self.0.proof_work(seat.index())
    }

    pub fn commit_proof(&mut self, result: AnalysisResult) -> Result<(), Rejection> {
        self.0.commit_proof(result)
    }

    pub fn start_ms(&self) -> u64 {
        self.0.start
    }

    pub fn advance(&mut self, now: u64) -> Result<(), Rejection> {
        self.0.advance(now)
    }

    pub fn cancel_before_start(&mut self, at: u64) -> Result<(), Rejection> {
        self.0.cancel_before_start(at)
    }

    pub fn disconnect(&mut self, seat: Seat, now: u64) -> Result<(), Rejection> {
        self.0.disconnect(seat.index(), now)
    }

    pub fn resume(&mut self, seat: Seat, epoch: u32, now: u64) -> Result<(), Rejection> {
        self.0.resume(seat.index(), epoch, now)
    }

    pub fn abort(&mut self, now: u64) -> Result<(), Rejection> {
        self.0.abort(now)
    }
}

fn indexed_command(command: Command) -> Command<usize> {
    Command {
        id: command.id,
        seq: command.seq,
        epoch: command.epoch,
        seat: command.seat.index(),
        received_at: command.received_at,
        action: command.action,
    }
}

impl Round {
    fn prepare(
        board: Board,
        rules: RulesSnapshot,
        count: usize,
    ) -> Result<PreparedRound, Rejection> {
        rules.verify().map_err(|_| Rejection::InvalidRules)?;
        let spec = rules.rules.board_spec();
        if board.spec() != spec || board.cell(spec.opening) != Some(Cell::Number(0)) {
            return Err(Rejection::InvalidBoard);
        }
        crate::generator::certify(&board, SolverBudget::default())
            .map_err(|_| Rejection::InvalidBoard)?;
        let mut view = Observation::closed(spec).map_err(|_| Rejection::InvalidBoard)?;
        let flags = vec![false; spec.area()];
        board
            .reveal(&mut view, spec.opening, &flags)
            .map_err(|_| Rejection::InvalidBoard)?;
        let knowledge = PolicyKnowledge::start(&view, SolverBudget::default())
            .map_err(|_| Rejection::InvalidBoard)?;
        let player = || Player {
            view: view.clone(),
            flags: flags.clone(),
            gauge: 0,
            stun_until: 0,
            overlays: Vec::new(),
            knowledge: knowledge.clone(),
            observation_revision: 0,
            epoch: 1,
            last_seq: 0,
            disconnected_at: None,
            forfeited: false,
            commands: BTreeMap::new(),
            stats: GameStats::default(),
        };
        Ok(PreparedRound {
            board,
            rules,
            players: (0..count).map(|_| player()).collect(),
        })
    }
    fn projected_at(&self) -> u64 {
        self.end.map_or(self.now, |end| end.at)
    }

    fn countdown_ms(&self, at: u64) -> u32 {
        self.start.saturating_sub(at) as u32
    }

    fn remaining_ms(&self, at: u64) -> u32 {
        self.deadline.saturating_sub(at.max(self.start)) as u32
    }

    fn own_view(&self, seat: usize, at: u64) -> OwnView {
        let player = &self.players[seat];
        OwnView {
            cells: player.view.clone(),
            flags: player.flags.clone(),
            gauge: player.gauge,
            stun_ms: player.stun_until.saturating_sub(at) as u32,
            history: player.knowledge.public_history().to_vec(),
            stats: player.stats,
        }
    }

    fn reconnect_deadline(&self, seat: usize) -> Option<u64> {
        self.players[seat]
            .disconnected_at
            .map(|at| at.saturating_add(u64::from(self.rules.rules.reconnect_grace_ms)))
    }

    fn opponent_view(&self, seat: usize, at: u64) -> OpponentView {
        let player = &self.players[seat];
        OpponentView {
            opened_safe: player.view.opened_safe() as u16,
            stun_ms: player.stun_until.saturating_sub(at) as u32,
            reconnect_ms: if self.end.is_some() || player.forfeited {
                None
            } else {
                self.reconnect_deadline(seat)
                    .map(|until| until.saturating_sub(at) as u32)
            },
        }
    }

    fn validate_seat(&self, seat: usize) -> Result<(), Rejection> {
        if seat < self.players.len() {
            Ok(())
        } else {
            Err(Rejection::InvalidSeat)
        }
    }

    fn apply(&mut self, command: Command<usize>) -> Ack {
        let index = command.seat;
        let reject = |reason, revision| Ack {
            command_id: command.id,
            revision,
            status: ActionStatus::Rejected(reason),
            duplicate: false,
        };
        if let Err(error) = self.validate_seat(index) {
            return reject(error, 0);
        }
        if let Err(error) = self.advance(command.received_at) {
            return reject(error, self.public_revision[index]);
        }
        if command.epoch != self.players[index].epoch {
            return reject(Rejection::InvalidEpoch, self.public_revision[index]);
        }
        if let Some(cached) = self.players[index].commands.get(&command.id) {
            return if cached.seq == command.seq && cached.action == command.action {
                Ack {
                    duplicate: true,
                    ..cached.ack
                }
            } else {
                reject(Rejection::CommandConflict, self.public_revision[index])
            };
        }
        if command.seq <= self.players[index].last_seq {
            return reject(Rejection::InvalidSequence, self.public_revision[index]);
        }
        if self.players[index].commands.len() >= usize::from(self.rules.rules.max_commands_per_seat)
        {
            return reject(Rejection::CommandLimit, self.public_revision[index]);
        }
        self.players[index].last_seq = command.seq;
        let status = match self.act(command.seat, command.action) {
            Ok(()) => {
                match command.action {
                    Action::Attack | Action::ToggleFlag(_) => self.public_revision[index] += 1,
                    _ => self.bump_all(),
                }
                if self.players[index].view.opened_safe() == self.board.safe_total() {
                    self.finish(EndReason::Clear, Some(command.seat), self.now);
                }
                ActionStatus::Applied
            }
            Err(reason) => ActionStatus::Rejected(reason),
        };
        let ack = Ack {
            command_id: command.id,
            revision: self.public_revision[index],
            status,
            duplicate: false,
        };
        self.players[index].commands.insert(
            command.id,
            Cached {
                seq: command.seq,
                action: command.action,
                ack,
            },
        );
        ack
    }
    fn apply_batch(&mut self, mut commands: Vec<Command<usize>>) -> Vec<Ack> {
        commands.sort_by_key(|c| (c.received_at, c.seat, c.seq));
        commands
            .into_iter()
            .map(|command| self.apply(command))
            .collect()
    }
    fn act(&mut self, index: usize, action: Action) -> Result<(), Rejection> {
        if self.end.is_some() {
            return Err(Rejection::Finished);
        }
        if self.now < self.start {
            return Err(Rejection::NotStarted);
        }
        if self.players[index].forfeited || self.players[index].disconnected_at.is_some() {
            return Err(Rejection::Disconnected);
        }
        if self.now < self.players[index].stun_until {
            return Err(Rejection::Stunned);
        }
        if matches!(self.mode, RoundMode::Solo)
            && (index != 0 || !matches!(action, Action::Open(_) | Action::ToggleFlag(_)))
        {
            return Err(Rejection::AttackUnavailable);
        }
        match action {
            Action::Open(cell) => {
                if !self.board.spec().contains(cell) {
                    return Err(Rejection::InvalidCell);
                }
                if self.players[index].view.cell(cell) != Some(ObservedCell::Unknown) {
                    return Err(Rejection::AlreadyOpen);
                }
                if self.players[index].flags[usize::from(cell.0)] {
                    return Err(Rejection::Flagged);
                }
                let mut view = self.players[index].view.clone();
                let opened = self
                    .board
                    .reveal(&mut view, cell, &self.players[index].flags)
                    .map_err(|_| Rejection::InvalidBoard)?;
                for lie in &self.players[index].overlays {
                    if matches!(view.cell(lie.target), Some(ObservedCell::Number(_))) {
                        view.set(lie.target, ObservedCell::Number(lie.shown))
                            .map_err(|_| Rejection::InvalidBoard)?;
                    }
                }
                let hit_mine = view.cell(cell) == Some(ObservedCell::Mine);
                self.commit_view(index, view)?;
                let player = &mut self.players[index];
                let gain = opened.len() as u32 * u32::from(self.rules.rules.safe_gain);
                player.gauge = (u32::from(player.gauge) + gain)
                    .min(u32::from(self.rules.rules.gauge_capacity))
                    as u16;
                if hit_mine {
                    player.stats.mistakes += 1;
                    player.stun_until = player.stun_until.max(
                        self.now
                            .saturating_add(u64::from(self.rules.rules.mine_stun_ms)),
                    );
                }
            }
            Action::ToggleFlag(cell) => {
                if self.players[index].view.cell(cell) != Some(ObservedCell::Unknown) {
                    return Err(Rejection::InvalidCell);
                }
                let flag = &mut self.players[index].flags[usize::from(cell.0)];
                *flag = !*flag;
            }
            Action::Attack => self.attack(index)?,
            Action::Accuse(cell) => {
                if !matches!(
                    self.players[index].view.cell(cell),
                    Some(ObservedCell::Number(_))
                ) {
                    return Err(Rejection::InvalidCell);
                }
                if let Some(lie_index) = self.players[index]
                    .overlays
                    .iter()
                    .position(|lie| lie.target == cell)
                {
                    let lie = &self.players[index].overlays[lie_index];
                    let source = lie.source;
                    let mut view = self.players[index].view.clone();
                    view.set(cell, ObservedCell::Number(lie.truth))
                        .map_err(|_| Rejection::InvalidBoard)?;
                    self.commit_view(index, view)?;
                    self.players[index].overlays.remove(lie_index);
                    self.players[index].stats.accusation_attempts += 1;
                    self.players[index].stats.correct_accusations += 1;
                    let attacker = &mut self.players[source];
                    attacker.gauge = 0;
                    attacker.stun_until = attacker.stun_until.max(
                        self.now
                            .saturating_add(u64::from(self.rules.rules.reflect_stun_ms)),
                    );
                } else {
                    self.players[index].stats.accusation_attempts += 1;
                    self.players[index].stats.mistakes += 1;
                    self.players[index].stun_until = self.players[index].stun_until.max(
                        self.now
                            .saturating_add(u64::from(self.rules.rules.wrong_accuse_stun_ms)),
                    );
                }
            }
        }
        Ok(())
    }
    fn attack(&mut self, seat: usize) -> Result<(), Rejection> {
        if self.players[seat].gauge < self.rules.rules.gauge_capacity {
            return Err(Rejection::AttackUnavailable);
        }
        let defender = match &mut self.mode {
            RoundMode::Race(random) => {
                let eligible = (0..self.players.len())
                    .filter(|&index| index != seat && !self.players[index].forfeited);
                multiplayer::draw_target(eligible, random.as_mut())
                    .ok_or(Rejection::AttackUnavailable)?
            }
            RoundMode::Duel | RoundMode::Solo => 1 - seat,
        };
        if self.players[defender].overlays.len() >= usize::from(self.rules.rules.max_lies) {
            return Err(Rejection::AttackUnavailable);
        }
        for i in 0..self.board.spec().area() {
            let cell = CellId(i as u16);
            if self.players[defender]
                .overlays
                .iter()
                .any(|lie| lie.target == cell)
            {
                continue;
            }
            for delta in [-1, 1] {
                let knowledge = &self.players[defender].knowledge;
                let Ok(proof) =
                    knowledge.certificate(cell, delta, self.players[defender].overlays.len())
                else {
                    continue;
                };
                if !proof.matches(knowledge)
                    || self.board.cell(cell) != Some(Cell::Number(proof.expected_truth()))
                {
                    return Err(Rejection::AttackUnavailable);
                }
                self.players[defender].overlays.push(Overlay {
                    target: cell,
                    truth: proof.expected_truth(),
                    shown: proof.displayed(),
                    source: seat,
                });
                self.players[seat].gauge = 0;
                return Ok(());
            }
        }
        Err(Rejection::AttackUnavailable)
    }
    fn commit_view(&mut self, seat: usize, view: Observation) -> Result<(), Rejection> {
        let player = &mut self.players[seat];
        let mut knowledge = player.knowledge.clone();
        knowledge
            .observe(&view)
            .map_err(|_| Rejection::InvalidBoard)?;
        player.view = view;
        player.knowledge = knowledge;
        player.observation_revision += 1;
        Ok(())
    }
    fn proof_work(&self, seat: usize) -> AnalysisJob {
        let player = &self.players[seat];
        AnalysisJob {
            seat,
            revision: player.observation_revision,
            knowledge: player.knowledge.clone(),
        }
    }
    fn commit_proof(&mut self, result: AnalysisResult) -> Result<(), Rejection> {
        self.validate_seat(result.seat)?;
        let player = &mut self.players[result.seat];
        if player.observation_revision != result.revision
            || player.view != *result.knowledge.public_view()
            || player.knowledge.public_history() != result.knowledge.public_history()
        {
            return Err(Rejection::AnalysisStale);
        }
        player.knowledge = result.knowledge;
        Ok(())
    }
    fn advance(&mut self, now: u64) -> Result<(), Rejection> {
        if now < self.now {
            return Err(Rejection::InvalidTime);
        }
        self.now = now;
        if self.end.is_some() {
            return Ok(());
        }
        if now >= self.start
            && self
                .players
                .iter()
                .all(|p| p.view.opened_safe() == self.board.safe_total())
        {
            let winner = matches!(self.mode, RoundMode::Solo).then_some(0);
            self.finish(EndReason::Clear, winner, self.start);
            return Ok(());
        }
        if matches!(self.mode, RoundMode::Race(_)) {
            self.advance_race(now);
            return Ok(());
        }
        let mut terminal = (
            self.deadline,
            EndReason::Timeout,
            self.duel_progress_winner(),
        );
        let grace = u64::from(self.rules.rules.reconnect_grace_ms);
        let disconnect = match (
            self.players[0].disconnected_at,
            self.players[1].disconnected_at,
        ) {
            (Some(a), Some(b)) => {
                Some((a.max(b).saturating_add(grace), EndReason::Abandoned, None))
            }
            (Some(a), None) => Some((a.saturating_add(grace), EndReason::Forfeit, Some(1))),
            (None, Some(b)) => Some((b.saturating_add(grace), EndReason::Forfeit, Some(0))),
            (None, None) => None,
        };
        if let Some(candidate) = disconnect
            && candidate.0 < terminal.0
        {
            terminal = candidate;
        }
        if now >= terminal.0 {
            self.finish(terminal.1, terminal.2, terminal.0);
        }
        Ok(())
    }
    fn duel_progress_winner(&self) -> Option<usize> {
        match self.players[0]
            .view
            .opened_safe()
            .cmp(&self.players[1].view.opened_safe())
        {
            std::cmp::Ordering::Greater => Some(0),
            std::cmp::Ordering::Less => Some(1),
            std::cmp::Ordering::Equal => None,
        }
    }
    fn finish(&mut self, reason: EndReason, winner: Option<usize>, at: u64) {
        if self.end.is_none() {
            let end = MatchEnd { reason, winner, at };
            self.end = Some(end);
            if matches!(self.mode, RoundMode::Race(_)) {
                self.race_result = Some(multiplayer::RaceResult::from_round(self, end));
            }
            self.bump_all();
        }
    }
    fn bump_all(&mut self) {
        for revision in &mut self.public_revision {
            *revision += 1;
        }
    }
    fn cancel_before_start(&mut self, at: u64) -> Result<(), Rejection> {
        if self.end.is_some() {
            return Err(Rejection::Finished);
        }
        if at < self.now || at > self.start || self.now >= self.start {
            return Err(Rejection::InvalidTime);
        }
        self.now = at;
        self.finish(EndReason::Cancelled, None, at);
        Ok(())
    }
    fn disconnect(&mut self, seat: usize, now: u64) -> Result<(), Rejection> {
        self.validate_seat(seat)?;
        self.advance(now)?;
        if self.end.is_some() {
            return Err(Rejection::Finished);
        }
        if now < self.start {
            self.finish(EndReason::Cancelled, None, now);
            return Ok(());
        }
        let player = &mut self.players[seat];
        if player.forfeited {
            return Err(Rejection::Disconnected);
        }
        if player.disconnected_at.is_none() {
            player.disconnected_at = Some(now);
            if matches!(self.mode, RoundMode::Race(_)) {
                self.bump_all();
            }
        }
        Ok(())
    }
    fn resume(&mut self, seat: usize, epoch: u32, now: u64) -> Result<(), Rejection> {
        self.validate_seat(seat)?;
        self.advance(now)?;
        if self.end.is_some() {
            return Err(Rejection::Finished);
        }
        let player = &mut self.players[seat];
        if player.forfeited {
            return Err(Rejection::Disconnected);
        }
        if epoch <= player.epoch {
            return Err(Rejection::InvalidEpoch);
        }
        if let Some(at) = player.disconnected_at
            && now >= at.saturating_add(u64::from(self.rules.rules.reconnect_grace_ms))
        {
            return Err(Rejection::Disconnected);
        }
        player.disconnected_at = None;
        player.epoch = epoch;
        if matches!(self.mode, RoundMode::Race(_)) {
            self.bump_all();
        } else {
            self.public_revision[seat] += 1;
        }
        self.advance(now)
    }
    fn abort(&mut self, now: u64) -> Result<(), Rejection> {
        self.advance(now)?;
        if self.end.is_none() {
            self.finish(EndReason::ServerFailure, None, now);
        }
        Ok(())
    }
}
