//! Local practice adapter. All game decisions stay in liar-core; views use the shared DTO.
use liar_core::{
    bot::{BotPolicy, Clock, Difficulty},
    game::{Action, Command, RuleEngine, Seat},
    generator::{BoardGenerator, GenerationBudget},
    random::SeededRng,
    rules::RulesSnapshot,
};
use liar_protocol::game::{GameView, LocalStep, PublicAction, PublicError, from_projection};
pub struct PracticeSession {
    engine: RuleEngine,
    bot: BotPolicy,
    random: SeededRng,
    bot_seq: u64,
    analysis_history: [usize; 2],
}
struct Tick(u64);
impl Clock for Tick {
    fn now_ms(&self) -> u64 {
        self.0
    }
}
impl PracticeSession {
    pub fn new(seed: &str, difficulty: &str) -> Result<Self, PublicError> {
        if seed.is_empty() || seed.len() > 20 || !seed.bytes().all(|c| c.is_ascii_digit()) {
            return Err(PublicError::Malformed);
        }
        let seed = seed.parse::<u64>().map_err(|_| PublicError::Malformed)?;
        let difficulty = match difficulty {
            "easy" => Difficulty::Easy,
            "normal" => Difficulty::Normal,
            "hard" => Difficulty::Hard,
            _ => return Err(PublicError::Malformed),
        };
        let rules = RulesSnapshot::bundled();
        let generated =
            BoardGenerator::generate(rules.rules.board_spec(), seed, GenerationBudget::default())
                .map_err(|_| PublicError::Unavailable)?;
        let engine = RuleEngine::new(generated.board, rules, 0).map_err(PublicError::from)?;
        let bot = BotPolicy::new(difficulty, &engine.projection(Seat::Two))
            .map_err(|_| PublicError::Unavailable)?;
        Ok(Self {
            engine,
            bot,
            random: SeededRng::new(0xb075eed),
            bot_seq: 0,
            analysis_history: [1, 1],
        })
    }
    pub fn view(&self) -> GameView {
        from_projection(&self.engine.projection(Seat::One), Seat::One)
    }
    pub fn step(&mut self, input: &str, time_ms: u32) -> Result<LocalStep, PublicError> {
        let input = liar_protocol::game::decode_local(input)?;
        self.engine
            .advance(u64::from(time_ms))
            .map_err(PublicError::from)?;
        let ack = self.engine.apply(Command {
            id: u128::from(input.command_id),
            seq: u64::from(input.client_seq),
            epoch: 1,
            seat: Seat::One,
            received_at: u64::from(time_ms),
            action: action_from_public(input.action),
        });
        self.run_bot(time_ms)?;
        Ok(LocalStep {
            ack: liar_protocol::game::from_ack(ack, input.command_id),
            view: self.view(),
        })
    }
    pub fn advance(&mut self, time_ms: u32) -> Result<GameView, PublicError> {
        self.engine
            .advance(u64::from(time_ms))
            .map_err(PublicError::from)?;
        self.run_bot(time_ms)?;
        Ok(self.view())
    }
    fn refresh(&mut self) {
        for seat in [Seat::One, Seat::Two] {
            let count = self.engine.projection(seat).own.history.len();
            if count != self.analysis_history[seat.index()] {
                if let Ok(result) = self.engine.proof_work(seat).run() {
                    let _ = self.engine.commit_proof(result);
                }
                self.analysis_history[seat.index()] = count;
            }
        }
    }
    fn run_bot(&mut self, time_ms: u32) -> Result<(), PublicError> {
        self.refresh();
        let Some(action) = self
            .bot
            .choose(
                &self.engine.projection(Seat::Two),
                &Tick(u64::from(time_ms)),
                &mut self.random,
            )
            .map_err(|_| PublicError::Unavailable)?
        else {
            return Ok(());
        };
        self.bot_seq += 1;
        self.engine.apply(Command {
            id: u128::from(self.bot_seq),
            seq: self.bot_seq,
            epoch: 1,
            seat: Seat::Two,
            received_at: u64::from(time_ms),
            action,
        });
        self.refresh();
        Ok(())
    }
}
pub fn action_from_public(action: PublicAction) -> Action {
    use liar_core::board::CellId;
    match action {
        PublicAction::Open { cell } => Action::Open(CellId(cell)),
        PublicAction::Flag { cell } => Action::ToggleFlag(CellId(cell)),
        PublicAction::Accuse { cell } => Action::Accuse(CellId(cell)),
        PublicAction::Attack => Action::Attack,
    }
}

pub fn checked_time_ms(value: f64) -> Result<u32, PublicError> {
    if !value.is_finite() || value.fract() != 0.0 || value < 0.0 || value > f64::from(u32::MAX) {
        return Err(PublicError::InvalidTime);
    }
    Ok(value as u32)
}
