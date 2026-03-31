// Time and Location Context Provider
// Injects current date, time, and timezone information into system prompts

use super::ContextProvider;
use crate::mcp::types::ContextVolatility;
use async_trait::async_trait;
use chrono::{Datelike, Local, Timelike};

/// Context provider for current time and location information
///
/// Provides current date, time, and timezone in a human-readable format.
/// This helps AI understand the user's temporal context.
pub struct TimeLocationContextProvider;

impl TimeLocationContextProvider {
    /// Create a new time location context provider
    pub fn new() -> Self {
        Self
    }

    /// Build time and location context string
    fn build_context(&self) -> String {
        let now = Local::now();

        // Format date as "Monday, December 30, 2025"
        let weekday = match now.weekday() {
            chrono::Weekday::Mon => "Monday",
            chrono::Weekday::Tue => "Tuesday",
            chrono::Weekday::Wed => "Wednesday",
            chrono::Weekday::Thu => "Thursday",
            chrono::Weekday::Fri => "Friday",
            chrono::Weekday::Sat => "Saturday",
            chrono::Weekday::Sun => "Sunday",
        };

        let month = match now.month() {
            1 => "January",
            2 => "February",
            3 => "March",
            4 => "April",
            5 => "May",
            6 => "June",
            7 => "July",
            8 => "August",
            9 => "September",
            10 => "October",
            11 => "November",
            12 => "December",
            _ => "Unknown",
        };

        let current_date = format!("{}, {} {}, {}", weekday, month, now.day(), now.year());

        // Format time with timezone (hour granularity - avoids cache invalidation on every minute/second)
        let current_time = format!("{:02}:00 {}", now.hour(), now.offset());

        // Get timezone name
        let timezone = format!("{}", now.offset());

        format!(
            "# Current Context Information\n\n\
            ## Date and Time\n\
            - **Current Date**: {}\n\
            - **Current Time**: {}\n\
            - **Timezone**: {}\n\n\
            *This information is automatically updated to help you understand the user's current temporal context.*",
            current_date,
            current_time,
            timezone
        )
    }
}

#[async_trait]
impl ContextProvider for TimeLocationContextProvider {
    fn provider_id(&self) -> &str {
        "time_location"
    }

    fn priority(&self) -> i32 {
        1000 // Low priority - volatile content placed last to maximize stable prefix for prompt caching
    }

    fn volatility(&self) -> ContextVolatility {
        ContextVolatility::Volatile
    }

    async fn get_context(&self, _assistant_id: Option<&str>) -> Result<String, String> {
        Ok(self.build_context())
    }

    async fn is_enabled(&self) -> bool {
        // Always enabled - time/location is always relevant
        true
    }
}

impl Default for TimeLocationContextProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_context_contains_sections() {
        let provider = TimeLocationContextProvider::new();
        let context = provider.build_context();

        assert!(context.contains("# Current Context Information"));
        assert!(context.contains("## Date and Time"));
        assert!(context.contains("Current Date"));
        assert!(context.contains("Current Time"));
        assert!(context.contains("Timezone"));
    }

    #[tokio::test]
    async fn test_provider_trait() {
        let provider = TimeLocationContextProvider::new();

        assert_eq!(provider.provider_id(), "time_location");
        assert_eq!(provider.priority(), 1000);
        assert_eq!(provider.volatility(), ContextVolatility::Volatile);
        assert!(provider.is_enabled().await);

        let context = provider.get_context(None).await;
        assert!(context.is_ok());
        assert!(!context.unwrap().is_empty());
    }
}
