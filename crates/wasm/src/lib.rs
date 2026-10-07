//! Browser adapter for the shared local game domain.
pub mod daily;
pub mod session;
pub mod training;
use liar_protocol::game::PublicError;
use wasm_bindgen::prelude::*;
fn js_error(error: PublicError) -> JsError {
    JsError::new(
        serde_json::to_string(&error)
            .unwrap_or_else(|_| "\"unavailable\"".into())
            .trim_matches('"'),
    )
}

#[wasm_bindgen]
pub struct DailySession {
    session: daily::DailySession,
}
#[wasm_bindgen]
impl DailySession {
    #[wasm_bindgen(constructor)]
    pub fn new(date: &str, seed_version: f64) -> Result<DailySession, JsError> {
        let version = daily::checked_seed_version(seed_version).map_err(js_error)?;
        Ok(Self {
            session: daily::DailySession::new(date, version).map_err(js_error)?,
        })
    }
    pub fn snapshot(&self) -> Result<String, JsError> {
        serde_json::to_string(&self.session.view()).map_err(|_| js_error(PublicError::Unavailable))
    }
    pub fn step(&mut self, input: &str, time_ms: f64) -> Result<String, JsError> {
        let time = session::checked_time_ms(time_ms).map_err(js_error)?;
        serde_json::to_string(&self.session.step(input, time).map_err(js_error)?)
            .map_err(|_| js_error(PublicError::Unavailable))
    }
    pub fn advance(&mut self, time_ms: f64) -> Result<String, JsError> {
        let time = session::checked_time_ms(time_ms).map_err(js_error)?;
        serde_json::to_string(&self.session.advance(time).map_err(js_error)?)
            .map_err(|_| js_error(PublicError::Unavailable))
    }
    pub fn replay(&self) -> Result<String, JsError> {
        serde_json::to_string(&self.session.export_replay())
            .map_err(|_| js_error(PublicError::Unavailable))
    }
}
#[wasm_bindgen]
pub struct LocalSession {
    session: session::PracticeSession,
}
#[wasm_bindgen]
impl LocalSession {
    #[wasm_bindgen(constructor)]
    pub fn new(seed: &str, difficulty: &str) -> Result<LocalSession, JsError> {
        Ok(Self {
            session: session::PracticeSession::new(seed, difficulty).map_err(js_error)?,
        })
    }
    pub fn snapshot(&self) -> Result<String, JsError> {
        serde_json::to_string(&self.session.view()).map_err(|_| js_error(PublicError::Unavailable))
    }
    pub fn step(&mut self, input: &str, time_ms: f64) -> Result<String, JsError> {
        let time_ms = session::checked_time_ms(time_ms).map_err(js_error)?;
        serde_json::to_string(&self.session.step(input, time_ms).map_err(js_error)?)
            .map_err(|_| js_error(PublicError::Unavailable))
    }
    pub fn advance(&mut self, time_ms: f64) -> Result<String, JsError> {
        let time_ms = session::checked_time_ms(time_ms).map_err(js_error)?;
        serde_json::to_string(&self.session.advance(time_ms).map_err(js_error)?)
            .map_err(|_| js_error(PublicError::Unavailable))
    }
}

#[wasm_bindgen]
pub struct TrainingSession {
    session: training::TrainingSession,
}
#[wasm_bindgen]
impl TrainingSession {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<TrainingSession, JsError> {
        Ok(Self {
            session: training::TrainingSession::new().map_err(js_error)?,
        })
    }
    pub fn snapshot(&self) -> Result<String, JsError> {
        serde_json::to_string(&self.session.view()).map_err(|_| js_error(PublicError::Unavailable))
    }
    pub fn step(&mut self, input: &str, time_ms: f64) -> Result<String, JsError> {
        let time_ms = session::checked_time_ms(time_ms).map_err(js_error)?;
        serde_json::to_string(&self.session.step(input, time_ms).map_err(js_error)?)
            .map_err(|_| js_error(PublicError::Unavailable))
    }
    pub fn advance(&mut self, time_ms: f64) -> Result<String, JsError> {
        let time_ms = session::checked_time_ms(time_ms).map_err(js_error)?;
        serde_json::to_string(&self.session.advance(time_ms).map_err(js_error)?)
            .map_err(|_| js_error(PublicError::Unavailable))
    }
}
