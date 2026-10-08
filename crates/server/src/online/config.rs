use super::*;
use liar_protocol::online::OnlineError;
#[derive(Clone, Copy)]
pub struct OnlineConfig {
    pub limits: MatchLimits,
    pub connections: usize,
}
pub fn load_online_config<F: Fn(&str) -> Option<String>>(
    read: F,
) -> Result<OnlineConfig, OnlineError> {
    let value = |key, default, min, max| {
        let n = match read(key) {
            None => default,
            Some(v) if !v.is_empty() && v.bytes().all(|b| b.is_ascii_digit()) => {
                v.parse::<usize>().map_err(|_| OnlineError::Capacity)?
            }
            Some(_) => return Err(OnlineError::Capacity),
        };
        if !(min..=max).contains(&n) {
            return Err(OnlineError::Capacity);
        }
        Ok(n)
    };
    Ok(OnlineConfig {
        limits: MatchLimits {
            matches: value("LIAR_ONLINE_MATCHES", 16, 1, 10000)?,
            mailbox: value("LIAR_ONLINE_MAILBOX", 64, 4, 1024)?,
            outgoing: value("LIAR_ONLINE_OUTGOING", 16, 4, 256)?,
            proof_workers: value("LIAR_ONLINE_PROOF_WORKERS", 2, 1, 64)?,
        },
        connections: value("LIAR_ONLINE_CONNECTIONS", 32, 1, 20000)?,
    })
}
