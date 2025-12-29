//! Generic rate limiting and retry mechanism for all providers
//!
//! This module provides a reusable rate limiter that:
//! - Respects `requests_per_minute` configuration
//! - Implements retry logic with exponential backoff
//! - Handles network errors and rate limit errors (429)
//! - Supports abort signals for user cancellation
//! - Provides status callbacks for user feedback
//! - Automatically adjusts delays based on error type

use std::time::Duration;
use tokio::time::sleep;

use crate::agent::AgentContext;
use crate::config::AgentConfig;

/// Generic rate limiter with retry support
pub struct RateLimiter {
    /// Maximum number of retry attempts
    max_retries: usize,
    /// Base delay in seconds (calculated from requests_per_minute)
    base_delay: u64,
    /// Whether to use exponential backoff
    exponential_backoff: bool,
}

impl RateLimiter {
    /// Create a new rate limiter from agent configuration
    ///
    /// # Arguments
    /// * `config` - Agent configuration containing requests_per_minute
    /// * `max_retries` - Maximum number of retry attempts (default: 3)
    ///
    /// # Returns
    /// A configured RateLimiter instance
    pub fn from_config(config: &AgentConfig, max_retries: usize) -> Self {
        // Calculate base delay from RPM setting
        // Default to 60 seconds (1 RPM) if not set to be more conservative
        let base_delay = if let Some(rpm) = config.requests_per_minute {
            if rpm > 0 {
                60 / rpm // Convert RPM to seconds between requests
            } else {
                60 // Fallback to 60 seconds for rate limiting
            }
        } else {
            60 // Default to 60 seconds (1 RPM) - more conservative
        }
        .max(1); // Ensure at least 1 second delay

        Self {
            max_retries,
            base_delay,
            exponential_backoff: true,
        }
    }

    /// Create a rate limiter with custom settings
    pub fn new(base_delay: u64, max_retries: usize, exponential_backoff: bool) -> Self {
        Self {
            max_retries,
            base_delay: base_delay.max(1),
            exponential_backoff,
        }
    }

    /// Execute an operation with retry logic
    ///
    /// # Arguments
    /// * `operation` - Async function to execute (can be a closure or async block)
    /// * `context` - Agent execution context for callbacks and abort signals
    /// * `provider_name` - Name of the provider for logging (e.g., "openai", "gemini")
    ///
    /// # Returns
    /// The operation result or an error after all retries are exhausted
    ///
    /// # Example
    /// ```ignore
    /// let result = rate_limiter.execute_with_retry(
    ///     || async {
    ///         // Your API call here
    ///         make_request().await
    ///     },
    ///     &context,
    ///     "openai"
    /// ).await?;
    /// ```
    pub async fn execute_with_retry<F, Fut, T>(
        &self,
        operation: F,
        context: &AgentContext,
        provider_name: &str,
    ) -> Result<T, String>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<T, String>>,
    {
        let mut last_error = String::new();

        for attempt in 1..=self.max_retries {
            // Execute the operation
            let result = operation().await;

            match result {
                Ok(value) => {
                    // Success! Return the value
                    if attempt > 1 {
                        self.log_success(provider_name, attempt, context);
                    }
                    return Ok(value);
                }
                Err(error) => {
                    last_error = error.clone();

                    // Check if this is a retryable error
                    let is_retryable = self.is_retryable_error(&error);

                    if !is_retryable {
                        // Non-retryable error, fail immediately
                        return Err(error);
                    }

                    // Calculate delay for this attempt with error context
                    let delay = self.calculate_delay(attempt, &error);

                    // Log the error and retry intent
                    self.log_error(
                        provider_name,
                        attempt,
                        self.max_retries,
                        &error,
                        delay,
                        context,
                    );

                    // Check if we should retry
                    if attempt < self.max_retries {
                        // Wait for delay or abort signal
                        let should_abort = self.wait_with_abort(delay, context).await;

                        if should_abort {
                            return Err("Cancelled by user".to_string());
                        }
                    }
                }
            }
        }

        // All retries exhausted
        Err(format!(
            "Failed after {} attempts: {}",
            self.max_retries, last_error
        ))
    }

    /// Calculate delay for a given attempt number and error type
    fn calculate_delay(&self, attempt: usize, error: &str) -> u64 {
        // Check if this is a rate limit error (429)
        let is_rate_limit = error.contains("429")
            || error.to_lowercase().contains("too many requests")
            || error.to_lowercase().contains("rate limit")
            || error.to_lowercase().contains("quota exceeded");

        if self.exponential_backoff {
            // For rate limit errors, use more aggressive backoff
            let multiplier = if is_rate_limit { 4 } else { 2 };

            // Exponential backoff: base_delay * multiplier^(attempt-1)
            // Capped at 10 minutes for rate limits, 5 minutes for other errors
            let exponential_delay = self.base_delay * u64::pow(multiplier, attempt as u32 - 1);
            let max_delay = if is_rate_limit { 600 } else { 300 }; // 10 min vs 5 min

            exponential_delay.min(max_delay)
        } else {
            // Linear delay, but longer for rate limits
            if is_rate_limit {
                self.base_delay * 2 // Double the delay for rate limits
            } else {
                self.base_delay
            }
        }
    }

    /// Determine if an error is retryable
    fn is_retryable_error(&self, error: &str) -> bool {
        let error_lower = error.to_lowercase();

        // Network errors
        if error_lower.contains("network")
            || error_lower.contains("connection")
            || error_lower.contains("timeout")
            || error_lower.contains("timed out")
        {
            return true;
        }

        // Rate limit errors (429) - always retryable
        if error_lower.contains("429")
            || error_lower.contains("too many requests")
            || error_lower.contains("rate limit")
            || error_lower.contains("quota exceeded")
            || error_lower.contains("quota")
        {
            return true;
        }

        // Server errors (5xx)
        if error_lower.contains("503")
            || error_lower.contains("502")
            || error_lower.contains("500")
            || error_lower.contains("internal server error")
            || error_lower.contains("service unavailable")
        {
            return true;
        }

        false
    }

    /// Log error and retry intent
    fn log_error(
        &self,
        provider: &str,
        attempt: usize,
        max_retries: usize,
        error: &str,
        delay: u64,
        context: &AgentContext,
    ) {
        eprintln!(
            "[{}] Request failed (Attempt {}/{}): {}",
            provider, attempt, max_retries, error
        );

        if let Some(callback) = &context.status_callback {
            let message = format!(
                "⚠️ {} error (Attempt {}/{}). Retrying in {}s... (Use .stop to cancel)",
                provider, attempt, max_retries, delay
            );
            callback(message);
        }
    }

    /// Log successful retry
    fn log_success(&self, provider: &str, attempt: usize, context: &AgentContext) {
        eprintln!("[{}] Request succeeded on attempt {}", provider, attempt);

        if let Some(callback) = &context.status_callback {
            let message = format!("✅ {} request succeeded on attempt {}", provider, attempt);
            callback(message);
        }
    }

    /// Wait for delay duration, respecting abort signal
    async fn wait_with_abort(&self, delay_secs: u64, context: &AgentContext) -> bool {
        let duration = Duration::from_secs(delay_secs);

        // If we have an abort signal, wait with cancellation support
        if let Some(mut rx) = context.abort_signal.clone() {
            tokio::select! {
                _ = sleep(duration) => false,  // Delay completed, don't abort
                _ = rx.changed() => {
                    // Abort signal received
                    if *rx.borrow() {
                        if let Some(callback) = &context.status_callback {
                            callback("🛑 Retry cancelled by user.".to_string());
                        }
                        true  // Abort requested
                    } else {
                        false  // Signal changed but not set to true
                    }
                }
            }
        } else {
            // No abort signal, just sleep
            sleep(duration).await;
            false
        }
    }

    /// Get the current base delay in seconds
    pub fn base_delay(&self) -> u64 {
        self.base_delay
    }

    /// Get the maximum number of retries
    pub fn max_retries(&self) -> usize {
        self.max_retries
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delay_calculation() {
        // Test linear delay
        let limiter = RateLimiter::new(10, 3, false);
        assert_eq!(limiter.calculate_delay(1, "some error"), 10);
        assert_eq!(limiter.calculate_delay(2, "some error"), 10);
        assert_eq!(limiter.calculate_delay(3, "some error"), 10);

        // Test exponential delay for normal errors
        let limiter = RateLimiter::new(10, 3, true);
        assert_eq!(limiter.calculate_delay(1, "some error"), 10); // 10 * 2^0
        assert_eq!(limiter.calculate_delay(2, "some error"), 20); // 10 * 2^1
        assert_eq!(limiter.calculate_delay(3, "some error"), 40); // 10 * 2^2
    }

    #[test]
    fn test_rate_limit_delay() {
        // Test that rate limit errors get more aggressive backoff
        let limiter = RateLimiter::new(10, 3, true);
        assert_eq!(limiter.calculate_delay(1, "429 Too Many Requests"), 10); // 10 * 4^0
        assert_eq!(limiter.calculate_delay(2, "429 Too Many Requests"), 40); // 10 * 4^1
        assert_eq!(limiter.calculate_delay(3, "429 Too Many Requests"), 160); // 10 * 4^2
    }

    #[test]
    fn test_exponential_cap() {
        // Test that exponential delay is capped at 5 minutes for normal errors
        let limiter = RateLimiter::new(100, 10, true);
        assert_eq!(limiter.calculate_delay(10, "some error"), 300); // Should be capped at 300

        // Test that rate limit errors are capped at 10 minutes
        assert_eq!(limiter.calculate_delay(10, "429 error"), 600); // Should be capped at 600
    }

    #[test]
    fn test_minimum_delay() {
        // Test that delay is at least 1 second
        let limiter = RateLimiter::new(0, 3, true);
        assert_eq!(limiter.base_delay(), 1);
    }

    #[test]
    fn test_retryable_errors() {
        let limiter = RateLimiter::new(10, 3, true);

        // Network errors
        assert!(limiter.is_retryable_error("Network error"));
        assert!(limiter.is_retryable_error("Connection refused"));
        assert!(limiter.is_retryable_error("Request timed out"));

        // Rate limit errors
        assert!(limiter.is_retryable_error("429 Too Many Requests"));
        assert!(limiter.is_retryable_error("Rate limit exceeded"));
        assert!(limiter.is_retryable_error("Quota exceeded"));

        // Server errors
        assert!(limiter.is_retryable_error("503 Service Unavailable"));
        assert!(limiter.is_retryable_error("500 Internal Server Error"));

        // Non-retryable errors
        assert!(!limiter.is_retryable_error("Invalid API key"));
        assert!(!limiter.is_retryable_error("404 Not Found"));
        assert!(!limiter.is_retryable_error("400 Bad Request"));
    }
}
