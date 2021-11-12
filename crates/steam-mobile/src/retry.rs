use std::time::Duration;

use backoff::ExponentialBackoff;

const INITIAL_LOGIN_RETRY_SEC: u64 = 1;
const MAX_LOGIN_RETRY_SEC: u64 = 10;

pub(crate) fn login_retry_strategy() -> ExponentialBackoff {
    let mut login_retry_strategy = ExponentialBackoff::default();
    login_retry_strategy.current_interval = Duration::from_secs(INITIAL_LOGIN_RETRY_SEC);
    login_retry_strategy.initial_interval = Duration::from_secs(INITIAL_LOGIN_RETRY_SEC);
    login_retry_strategy.max_interval = Duration::from_secs(MAX_LOGIN_RETRY_SEC);
    login_retry_strategy
}
