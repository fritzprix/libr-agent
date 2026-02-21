use log::warn;
use std::time::Duration;

/// Retry a fallible operation with exponential backoff
///
/// # Arguments
/// * `operation` - The operation to retry
/// * `max_attempts` - Maximum number of attempts
/// * `initial_delay_ms` - Initial delay in milliseconds
pub fn retry_with_backoff<F, T, E>(
    mut operation: F,
    max_attempts: u32,
    initial_delay_ms: u64,
) -> Result<T, E>
where
    F: FnMut() -> Result<T, E>,
    E: std::fmt::Display,
{
    for attempt in 1..=max_attempts {
        match operation() {
            Ok(result) => return Ok(result),
            Err(e) if attempt < max_attempts => {
                // Exponential backoff: 100ms, 200ms, 400ms, 800ms, 1600ms
                let delay = Duration::from_millis(initial_delay_ms * 2_u64.pow(attempt - 1));
                warn!(
                    "⚠️ Attempt {}/{} failed: {}. Retrying in {}ms...",
                    attempt,
                    max_attempts,
                    e,
                    delay.as_millis()
                );
                std::thread::sleep(delay);
            }
            Err(e) => {
                warn!("❌ All {} attempts failed. Last error: {}", max_attempts, e);
                return Err(e);
            }
        }
    }
    unreachable!()
}

/// Async version of retry_with_backoff
pub async fn retry_with_backoff_async<F, Fut, T, E>(
    mut operation: F,
    max_attempts: u32,
    initial_delay_ms: u64,
) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    for attempt in 1..=max_attempts {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) if attempt < max_attempts => {
                let delay = Duration::from_millis(initial_delay_ms * 2_u64.pow(attempt - 1));
                warn!(
                    "⚠️ Attempt {}/{} failed: {}. Retrying in {}ms...",
                    attempt,
                    max_attempts,
                    e,
                    delay.as_millis()
                );
                tokio::time::sleep(delay).await;
            }
            Err(e) => {
                warn!("❌ All {} attempts failed. Last error: {}", max_attempts, e);
                return Err(e);
            }
        }
    }
    unreachable!()
}


