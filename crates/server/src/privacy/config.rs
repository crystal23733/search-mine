use liar_protocol::online::OnlineError;
#[derive(Clone, Copy)]
pub struct PrivacyConfig {
    pub authorities: usize,
    pub requests: usize,
}
pub fn load_privacy_config(
    read: impl Fn(&str) -> Option<String>,
) -> Result<PrivacyConfig, OnlineError> {
    let value = |key, default, max| {
        let value = match read(key) {
            None => default,
            Some(v) if !v.is_empty() && v.bytes().all(|b| b.is_ascii_digit()) => {
                v.parse::<usize>().map_err(|_| OnlineError::Capacity)?
            }
            _ => return Err(OnlineError::Capacity),
        };
        if !(1..=max).contains(&value) {
            return Err(OnlineError::Capacity);
        }
        Ok(value)
    };
    Ok(PrivacyConfig {
        authorities: value("LIAR_PRIVACY_AUTHORITIES", 64, 20000)?,
        requests: value("LIAR_PRIVACY_REQUESTS", 4, 64)?,
    })
}
