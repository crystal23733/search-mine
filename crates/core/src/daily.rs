//! Deterministic public solo daily challenge; no accounts, clocks or storage.
use crate::generator::{BoardGenerator, GenerationBudget};
use crate::{
    game::{Ack, Command, Projection, Rejection, RuleEngine, Seat},
    rules::RulesSnapshot,
};
use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};
pub const DAILY_SEED_VERSION: u16 = 1;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub struct DailyMetadata {
    pub date: String,
    pub seed_version: u16,
    pub mode: String,
    pub seed: String,
    pub rules_hash: String,
    pub solver_version: u16,
    pub rng_version: u16,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DailyError {
    InvalidDate,
    UnsupportedVersion,
    InvalidRules,
    Unavailable,
}
impl DailyMetadata {
    pub fn derive(date: &str, version: u16, rules: &RulesSnapshot) -> Result<Self, DailyError> {
        if version != DAILY_SEED_VERSION {
            return Err(DailyError::UnsupportedVersion);
        }
        rules.verify().map_err(|_| DailyError::InvalidRules)?;
        let bytes = date.as_bytes();
        if bytes.len() != 10
            || bytes.iter().enumerate().any(|(i, &byte)| {
                if i == 4 || i == 7 {
                    byte != b'-'
                } else {
                    !byte.is_ascii_digit()
                }
            })
        {
            return Err(DailyError::InvalidDate);
        }
        let parsed =
            NaiveDate::parse_from_str(date, "%Y-%m-%d").map_err(|_| DailyError::InvalidDate)?;
        if parsed.year() < 1 {
            return Err(DailyError::InvalidDate);
        }
        let canonical = format!(
            "liar-daily:solo-v1\n{date}\n{version}\n{}\n{}\n{}",
            rules.hash, rules.solver_version, rules.rng_version
        );
        let hash = blake3::hash(canonical.as_bytes());
        let mut seed_bytes = [0; 8];
        seed_bytes.copy_from_slice(&hash.as_bytes()[..8]);
        Ok(Self {
            date: date.into(),
            seed_version: version,
            mode: "solo-v1".into(),
            seed: u64::from_le_bytes(seed_bytes).to_string(),
            rules_hash: rules.hash.clone(),
            solver_version: rules.solver_version,
            rng_version: rules.rng_version,
        })
    }
    pub fn id(&self) -> String {
        format!("{}:v{}:{}", self.date, self.seed_version, self.rules_hash)
    }
}
pub struct DailyPuzzle {
    engine: RuleEngine,
    metadata: DailyMetadata,
}
impl DailyPuzzle {
    pub fn new(date: &str, version: u16) -> Result<Self, DailyError> {
        Self::with_rules(date, version, RulesSnapshot::bundled())
    }
    pub fn with_rules(date: &str, version: u16, rules: RulesSnapshot) -> Result<Self, DailyError> {
        let metadata = DailyMetadata::derive(date, version, &rules)?;
        let seed = metadata.seed.parse().map_err(|_| DailyError::Unavailable)?;
        let generated =
            BoardGenerator::generate(rules.rules.board_spec(), seed, GenerationBudget::default())
                .map_err(|_| DailyError::Unavailable)?;
        let engine =
            RuleEngine::new_solo(generated.board, rules, 0).map_err(|_| DailyError::Unavailable)?;
        Ok(Self { engine, metadata })
    }
    pub fn metadata(&self) -> &DailyMetadata {
        &self.metadata
    }
    pub fn projection(&self) -> Projection {
        self.engine.projection(Seat::One)
    }
    pub fn apply(&mut self, command: Command) -> Ack {
        self.engine.apply(command)
    }
    pub fn advance(&mut self, time: u64) -> Result<(), Rejection> {
        self.engine.advance(time)
    }
    pub fn complete(&self) -> bool {
        self.projection().end.is_some_and(|end| {
            end.reason == crate::game::EndReason::Clear && end.winner == Some(Seat::One)
        })
    }
    pub fn elapsed_ms(&self) -> u32 {
        let view = self.projection();
        view.rules
            .rules
            .duration_ms
            .saturating_sub(view.remaining_ms)
    }
}
