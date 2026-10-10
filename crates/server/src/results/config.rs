use liar_protocol::online::OnlineError;
#[derive(Clone, Copy)]
pub struct ResultConfig {
    pub authorities: usize,
    pub requests: usize,
}
pub fn load_result_config(
    read: impl Fn(&str) -> Option<String>,
) -> Result<ResultConfig, OnlineError> {
    let value = |key, default, max| {
        let value = match read(key) {
            None => default,
            Some(v) if !v.is_empty() && v.bytes().all(|b| b.is_ascii_digit()) => {
                v.parse::<usize>().map_err(|_| OnlineError::Capacity)?
            }
            Some(_) => return Err(OnlineError::Capacity),
        };
        if !(1..=max).contains(&value) {
            return Err(OnlineError::Capacity);
        }
        Ok(value)
    };
    Ok(ResultConfig {
        authorities: value("LIAR_RESULT_AUTHORITIES", 64, 20000)?,
        requests: value("LIAR_RESULT_REQUESTS", 16, 256)?,
    })
}
#[cfg(test)]
#[path = "../../tests/unit/result_config.rs"]
mod tests;
