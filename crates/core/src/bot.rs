//! Bot inputs are public projections only; inference and timing are separate concerns.
use crate::{
    board::{CellId, Observation, ObservedCell},
    game::{Action, Projection},
    policy::PolicyKnowledge,
    random::{RandomSource, shuffle},
    rules::{BotSettings, RulesSnapshot},
    solver::{SolverBudget, SolverError},
};
use std::collections::BTreeMap;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Difficulty {
    Easy,
    Normal,
    Hard,
}
pub trait Clock {
    fn now_ms(&self) -> u64;
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BotError {
    InvalidSnapshot,
    InvalidTime,
}
pub struct BotPolicy {
    rules: RulesSnapshot,
    profile: BotSettings,
    knowledge: PolicyKnowledge,
    last_revision: u64,
    last_tick: Option<u64>,
    next_decision: u64,
    seen_lies: BTreeMap<CellId, u64>,
    last_attack: Option<(usize, u16)>,
    planned_safe: Vec<CellId>,
}
impl BotPolicy {
    pub fn new(difficulty: Difficulty, projection: &Projection) -> Result<Self, BotError> {
        projection
            .rules
            .verify()
            .map_err(|_| BotError::InvalidSnapshot)?;
        let spec = projection.rules.rules.board_spec();
        if projection.own.flags.len() != spec.area() {
            return Err(BotError::InvalidSnapshot);
        }
        let batch = projection
            .own
            .history
            .first()
            .ok_or(BotError::InvalidSnapshot)?;
        let mut initial = Observation::closed(spec).map_err(|_| BotError::InvalidSnapshot)?;
        let mut seen = vec![false; spec.area()];
        for update in batch {
            let i = usize::from(update.cell.0);
            if i >= seen.len() || seen[i] || update.observed == ObservedCell::Unknown {
                return Err(BotError::InvalidSnapshot);
            }
            seen[i] = true;
            initial
                .set(update.cell, update.observed)
                .map_err(|_| BotError::InvalidSnapshot)?;
        }
        let knowledge =
            PolicyKnowledge::replay(&initial, &projection.own.history, SolverBudget::default())
                .map_err(|_| BotError::InvalidSnapshot)?;
        if knowledge.public_view() != &projection.own.cells {
            return Err(BotError::InvalidSnapshot);
        }
        Ok(Self {
            rules: projection.rules.clone(),
            profile: bot_settings(&projection.rules, difficulty),
            knowledge,
            last_revision: projection.revision,
            last_tick: None,
            next_decision: 0,
            seen_lies: BTreeMap::new(),
            last_attack: None,
            planned_safe: Vec::new(),
        })
    }
    fn sync(&mut self, projection: &Projection) -> Result<(), BotError> {
        let history = self.knowledge.public_history();
        if projection.rules != self.rules
            || projection.revision < self.last_revision
            || projection.own.flags.len() != self.rules.rules.board_spec().area()
            || projection.own.history.len() > 512
            || !projection.own.history.starts_with(history)
        {
            return Err(BotError::InvalidSnapshot);
        }
        let mut next = self.knowledge.clone();
        for batch in projection.own.history.iter().skip(history.len()) {
            next.replay_batch(batch)
                .map_err(|_| BotError::InvalidSnapshot)?;
        }
        if next.public_view() != &projection.own.cells {
            return Err(BotError::InvalidSnapshot);
        }
        self.knowledge = next;
        self.last_revision = projection.revision;
        Ok(())
    }
    pub fn choose(
        &mut self,
        projection: &Projection,
        clock: &dyn Clock,
        random: &mut dyn RandomSource,
    ) -> Result<Option<Action>, BotError> {
        let now = clock.now_ms();
        if self.last_tick.is_some_and(|last| now < last) {
            return Err(BotError::InvalidTime);
        }
        self.sync(projection)?;
        self.last_tick = Some(now);
        let lies = self.knowledge.known_lies();
        self.seen_lies
            .retain(|cell, _| lies.binary_search(cell).is_ok());
        for &cell in &lies {
            self.seen_lies.entry(cell).or_insert(now);
        }
        if projection.end.is_some()
            || projection.countdown_ms > 0
            || projection.own.stun_ms > 0
            || now < self.next_decision
        {
            return Ok(None);
        }
        self.next_decision = now.saturating_add(u64::from(self.profile.decision_ms));
        if !lies.is_empty() {
            let due = |cell: &CellId| {
                self.seen_lies[cell].saturating_add(u64::from(self.profile.accusation_ms))
            };
            if let Some(&cell) = lies.iter().find(|&cell| now >= due(cell)) {
                return Ok(Some(Action::Accuse(cell)));
            }
            self.next_decision = self
                .next_decision
                .min(lies.iter().map(due).min().unwrap_or(self.next_decision));
            return Ok(None);
        }
        let signature = (
            self.knowledge.public_history().len(),
            projection.opponent.opened_safe,
        );
        if projection.own.gauge >= self.rules.rules.gauge_capacity
            && self.last_attack != Some(signature)
        {
            self.last_attack = Some(signature);
            return Ok(Some(Action::Attack));
        }
        // Preserve this profile's completed safe proofs across later, more costly frontiers.
        self.planned_safe
            .retain(|&cell| projection.own.cells.cell(cell) == Some(ObservedCell::Unknown));
        if self.planned_safe.is_empty() {
            self.planned_safe = match self.knowledge.limited_safe(self.profile.budget()) {
                Ok(safe) => safe,
                Err(SolverError::BudgetExceeded) => return Ok(None),
                Err(SolverError::Inconsistent) => return Err(BotError::InvalidSnapshot),
            };
        }
        shuffle(&mut self.planned_safe, random);
        Ok(self.planned_safe.first().map(|&cell| {
            if projection.own.flags[usize::from(cell.0)] {
                Action::ToggleFlag(cell)
            } else {
                Action::Open(cell)
            }
        }))
    }
}
pub fn bot_settings(rules: &RulesSnapshot, difficulty: Difficulty) -> crate::rules::BotSettings {
    match difficulty {
        Difficulty::Easy => rules.rules.bots.easy.clone(),
        Difficulty::Normal => rules.rules.bots.normal.clone(),
        Difficulty::Hard => rules.rules.bots.hard.clone(),
    }
}
