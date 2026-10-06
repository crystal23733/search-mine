use crate::board::{BoardSpec, CellId};
use crate::{policy::ATTACK_STRATEGY, random::RNG_VERSION, solver::SOLVER_VERSION};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
#[serde(deny_unknown_fields)]
pub struct GameRules {
    pub version: u16,
    pub width: u8,
    pub height: u8,
    pub mines: u16,
    pub opening: u16,
    pub duration_ms: u32,
    pub countdown_ms: u32,
    pub mine_stun_ms: u32,
    pub wrong_accuse_stun_ms: u32,
    pub reflect_stun_ms: u32,
    pub gauge_capacity: u16,
    pub safe_gain: u16,
    pub max_lies: u8,
    pub reconnect_grace_ms: u32,
    pub room_expiry_ms: u32,
    pub max_commands_per_seat: u16,
    pub attack_strategy: String,
}
impl GameRules {
    pub fn board_spec(&self) -> BoardSpec {
        BoardSpec {
            width: self.width,
            height: self.height,
            mines: self.mines,
            opening: CellId(self.opening),
        }
    }
    pub fn validate(&self) -> Result<(), RulesError> {
        let spec = self
            .board_spec()
            .validate()
            .map_err(|_| RulesError::Invalid)?;
        if self.version != 1
            || self.width < 2
            || self.height < 2
            || self.mines == 0
            || usize::from(self.mines) > spec.area() - 1 - spec.neighbors(spec.opening).len()
            || !(1..=3_600_000).contains(&self.duration_ms)
            || self.countdown_ms > 10_000
            || [
                self.mine_stun_ms,
                self.wrong_accuse_stun_ms,
                self.reflect_stun_ms,
            ]
            .iter()
            .any(|v| !(1..=60_000).contains(v))
            || !(1..=100).contains(&self.gauge_capacity)
            || self.safe_gain == 0
            || self.safe_gain > self.gauge_capacity
            || self.max_lies != 2
            || self.reconnect_grace_ms == 0
            || self.reconnect_grace_ms > self.duration_ms
            || !(1..=86_400_000).contains(&self.room_expiry_ms)
            || !(1..=32768).contains(&self.max_commands_per_seat)
            || self.attack_strategy != ATTACK_STRATEGY
        {
            return Err(RulesError::Invalid);
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
#[serde(deny_unknown_fields)]
pub struct RulesSnapshot {
    pub rules: GameRules,
    pub hash: String,
    pub solver_version: u16,
    pub rng_version: u16,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RulesError {
    Malformed,
    Invalid,
    HashMismatch,
}
impl RulesSnapshot {
    pub fn load(source: &str) -> Result<Self, RulesError> {
        if source.len() > 16_384 {
            return Err(RulesError::Malformed);
        }
        Self::from_rules(toml::from_str(source).map_err(|_| RulesError::Malformed)?)
    }
    pub fn from_rules(rules: GameRules) -> Result<Self, RulesError> {
        rules.validate()?;
        let bytes = serde_json::to_vec(&(&rules, SOLVER_VERSION, RNG_VERSION))
            .map_err(|_| RulesError::Malformed)?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"liar-rules-v1\0");
        hasher.update(&bytes);
        Ok(Self {
            rules,
            hash: hasher.finalize().to_hex().to_string(),
            solver_version: SOLVER_VERSION,
            rng_version: RNG_VERSION,
        })
    }
    pub fn verify(&self) -> Result<(), RulesError> {
        if *self != Self::from_rules(self.rules.clone())? {
            return Err(RulesError::HashMismatch);
        }
        Ok(())
    }
    pub fn bundled() -> Self {
        static SNAPSHOT: std::sync::OnceLock<RulesSnapshot> = std::sync::OnceLock::new();
        SNAPSHOT
            .get_or_init(|| {
                Self::load(include_str!("../../../config/game-rules.toml"))
                    .expect("bundled rules are validated in CI")
            })
            .clone()
    }
}
