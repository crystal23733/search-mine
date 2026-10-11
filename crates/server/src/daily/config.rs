use super::ReplayError;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DailyConfig {
    pub authorities: usize,
    pub requests: usize,
}
pub fn load_daily_config(get: impl Fn(&str) -> Option<String>) -> Result<DailyConfig, ReplayError> {
    fn limit(value: Option<String>, default: usize, max: usize) -> Result<usize, ReplayError> {
        match value {
            None => Ok(default),
            Some(value) if !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()) => value
                .parse::<usize>()
                .ok()
                .filter(|n| (1..=max).contains(n))
                .ok_or(ReplayError::Unavailable),
            _ => Err(ReplayError::Unavailable),
        }
    }
    Ok(DailyConfig {
        authorities: limit(get("LIAR_DAILY_AUTHORITIES"), 64, 20000)?,
        requests: limit(get("LIAR_DAILY_REQUESTS"), 4, 64)?,
    })
}
