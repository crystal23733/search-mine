use super::*;
use liar_protocol::online::OnlineError;
#[derive(Clone, Copy)]
pub struct LobbyConfig {
    pub limits: LobbyLimits,
    pub authorities: usize,
    pub requests: usize,
    pub board_capacity: usize,
    pub board_workers: usize,
    pub bot_workers: usize,
}
pub fn load_lobby_config<F: Fn(&str) -> Option<String>>(
    read: F,
) -> Result<LobbyConfig, OnlineError> {
    let value = |key, default, min, max| {
        let n = match read(key) {
            None => default,
            Some(value) if !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()) => {
                value.parse::<usize>().map_err(|_| OnlineError::Capacity)?
            }
            Some(_) => return Err(OnlineError::Capacity),
        };
        if !(min..=max).contains(&n) {
            return Err(OnlineError::Capacity);
        }
        Ok(n)
    };
    let config = LobbyConfig {
        limits: LobbyLimits {
            capacity: value("LIAR_LOBBY_CAPACITY", 32, 1, 4096)?,
            workers: value("LIAR_LOBBY_WORKERS", 2, 1, 64)?,
        },
        authorities: value("LIAR_LOBBY_AUTHORITIES", 64, 1, 20000)?,
        requests: value("LIAR_LOBBY_REQUESTS", 16, 1, 256)?,
        board_capacity: value("LIAR_BOARD_CAPACITY", 4, 1, 256)?,
        board_workers: value("LIAR_BOARD_WORKERS", 1, 1, 64)?,
        bot_workers: value("LIAR_BOT_WORKERS", 2, 1, 64)?,
    };
    if config.limits.workers > config.limits.capacity
        || config.board_workers > config.board_capacity
    {
        return Err(OnlineError::Capacity);
    }
    Ok(config)
}
