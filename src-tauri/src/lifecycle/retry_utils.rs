use log::warn;
use std::time::Duration;

/// Retry a fallible operation with exponential backoff
///
/// # Arguments
/// * `operation` - The operation to retry
/// * `max_attempts` - Maximum number of attempts (default: 5)
/// * `initial_delay_ms` - Initial delay in milliseconds (default: 100)
///
/// # Example
/// ```
/// use tauri_mcp_agent_lib::lifecycle::retry_utils::retry_with_backoff;
///
/// let result = retry_with_backoff(
///     || Ok::<(), String>(()),
///     3,
///     10
/// );
/// assert!(result.is_ok());
/// ```
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    #[test]
    fn test_retry_success_on_third_attempt() {
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let result = retry_with_backoff(
            || {
                let count = counter_clone.fetch_add(1, Ordering::SeqCst) + 1;
                if count < 3 {
                    Err(format!("Attempt {}", count))
                } else {
                    Ok(42)
                }
            },
            5,
            10, // Short delay for testing
        );

        assert_eq!(result, Ok(42));
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn test_retry_all_attempts_fail() {
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let result = retry_with_backoff(
            || {
                counter_clone.fetch_add(1, Ordering::SeqCst);
                Err::<(), _>("Always fail")
            },
            3,
            10,
        );

        assert!(result.is_err());
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_retry_async() {
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let result = retry_with_backoff_async(
            || {
                let counter = counter_clone.clone();
                async move {
                    let count = counter.fetch_add(1, Ordering::SeqCst) + 1;
                    if count < 2 {
                        Err(format!("Attempt {}", count))
                    } else {
                        Ok("success")
                    }
                }
            },
            5,
            10,
        )
        .await;

        assert_eq!(result, Ok("success"));
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }
}
