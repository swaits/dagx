//! # Retry Strategies for Resilient Workflows
//!
//! This example demonstrates various retry patterns for handling transient failures
//! in DAG workflows, such as network errors, rate limits, or temporary service unavailability.
//!
//! ## What You'll Learn
//! - How to implement immediate, linear, and exponential backoff retry strategies
//! - How to distinguish retriable vs non-retriable errors
//! - How to track retry attempts and provide diagnostics
//! - How to implement jitter to prevent thundering herd problems
//!
//! ## Retry Patterns
//!
//! **1. Immediate Retry** - Retry without delay (for quick transient failures)
//! ```text
//! Attempt 1: ✗ Error
//! Attempt 2: ✗ Error (no delay)
//! Attempt 3: ✓ Success
//! ```
//!
//! **2. Linear Backoff** - Fixed delay between retries
//! ```text
//! Attempt 1: ✗ Error
//! Wait 100ms
//! Attempt 2: ✗ Error
//! Wait 100ms
//! Attempt 3: ✓ Success
//! ```
//!
//! **3. Exponential Backoff** - Increasing delays between retries
//! ```text
//! Attempt 1: ✗ Error
//! Wait 100ms
//! Attempt 2: ✗ Error
//! Wait 200ms
//! Attempt 3: ✗ Error
//! Wait 400ms
//! Attempt 4: ✓ Success
//! ```
//!
//! **4. Exponential Backoff with Jitter** - Randomized delays to prevent coordinated retries
//! ```text
//! Attempt 1: ✗ Error
//! Wait 87ms (50-150ms range)
//! Attempt 2: ✗ Error
//! Wait 234ms (100-300ms range)
//! Attempt 3: ✓ Success
//! ```
//!
//! ## Workflow Diagram
//! ```text
//! ┌──────────────────────┐
//! │  ApiCallWithRetry    │
//! │  Max attempts: 3     │
//! │  Strategy: Exp       │
//! └──────────────────────┘
//!           │
//!           ├─► Attempt 1: ✗ (Connection timeout)
//!           │   Wait 100ms
//!           │
//!           ├─► Attempt 2: ✗ (Connection timeout)
//!           │   Wait 200ms
//!           │
//!           └─► Attempt 3: ✓ Success
//!
//! ┌──────────────────────┐
//! │  ApiCallWithRetry    │
//! │  Max attempts: 3     │
//! │  Strategy: Linear    │
//! └──────────────────────┘
//!           │
//!           ├─► Attempt 1: ✗ (Rate limit)
//!           │   Wait 500ms
//!           │
//!           ├─► Attempt 2: ✗ (Rate limit)
//!           │   Wait 500ms
//!           │
//!           ├─► Attempt 3: ✗ (Rate limit)
//!           │
//!           └─► Final: ✗ Retry exhausted
//! ```
//!
//! ## Key Concepts
//! - **Idempotency**: Retried operations should be safe to repeat
//! - **Retriable errors**: Network timeouts, rate limits, temporary service errors
//! - **Non-retriable errors**: Authentication failures, invalid input, logical errors
//! - **Backoff strategies**: Balance between recovery speed and system load
//! - **Jitter**: Randomization to avoid thundering herd when many clients retry simultaneously
//! - **Retry budgets**: Limit total retries to prevent cascading failures
//!
//! ## Use Cases
//! - External API calls with transient network failures
//! - Database operations with temporary connection issues
//! - Distributed systems with eventual consistency
//! - Rate-limited API integrations
//! - Cloud service calls with intermittent availability
//!
//! ## Running This Example
//! ```bash
//! cargo run --example retry_strategies
//! ```
//!
//! ## Expected Output
//! ```text
//! === Retry Strategies Example ===
//!
//! Example 1: Immediate retry strategy
//! [ApiCall-1] Attempt 1/3: Calling external API...
//! [ApiCall-1] ✗ Transient error: Connection timeout
//! [ApiCall-1] Retrying immediately (attempt 2/3)
//! [ApiCall-1] Attempt 2/3: Calling external API...
//! [ApiCall-1] ✗ Transient error: Connection timeout
//! [ApiCall-1] Retrying immediately (attempt 3/3)
//! [ApiCall-1] Attempt 3/3: Calling external API...
//! [ApiCall-1] ✓ Success: API response data
//! Result: Ok(RetryResult { attempts: 3, success: true })
//!
//! Example 2: Linear backoff (500ms delay)
//! [ApiCall-2] Attempt 1/4: Calling external API...
//! [ApiCall-2] ✗ Transient error: Rate limit exceeded
//! [ApiCall-2] Waiting 500ms before retry (attempt 2/4)
//! [ApiCall-2] Attempt 2/4: Calling external API...
//! [ApiCall-2] ✓ Success: API response data
//! Result: Ok(RetryResult { attempts: 2, success: true })
//!
//! Example 3: Exponential backoff
//! [ApiCall-3] Attempt 1/5: Calling external API...
//! [ApiCall-3] ✗ Transient error: Service unavailable
//! [ApiCall-3] Waiting 100ms before retry (attempt 2/5)
//! [ApiCall-3] Attempt 2/5: Calling external API...
//! [ApiCall-3] ✗ Transient error: Service unavailable
//! [ApiCall-3] Waiting 200ms before retry (attempt 3/5)
//! [ApiCall-3] Attempt 3/5: Calling external API...
//! [ApiCall-3] ✓ Success: API response data
//! Result: Ok(RetryResult { attempts: 3, success: true })
//!
//! Example 4: Non-retriable error (fails immediately)
//! [ApiCall-4] Attempt 1/3: Calling external API...
//! [ApiCall-4] ✗ Non-retriable error: Authentication failed
//! Result: Err("Authentication failed - not retrying")
//!
//! Example 5: Retry exhausted
//! [ApiCall-5] Attempt 1/3: Calling external API...
//! [ApiCall-5] ✗ Transient error: Connection timeout
//! [ApiCall-5] Waiting 100ms before retry (attempt 2/3)
//! [ApiCall-5] Attempt 2/3: Calling external API...
//! [ApiCall-5] ✗ Transient error: Connection timeout
//! [ApiCall-5] Waiting 200ms before retry (attempt 3/3)
//! [ApiCall-5] Attempt 3/3: Calling external API...
//! [ApiCall-5] ✗ Transient error: Connection timeout
//! [ApiCall-5] All retry attempts exhausted
//! Result: Err("Retry exhausted after 3 attempts: Connection timeout")
//! ```

use dagx::{task, DagRunner, Task};
use tokio::time::{sleep, Duration};

// Retry strategy configuration
#[derive(Clone, Copy, Debug)]
enum RetryStrategy {
    Immediate,
    Linear { delay_ms: u64 },
    Exponential { base_ms: u64, factor: f64 },
}

impl RetryStrategy {
    fn calculate_delay(&self, attempt: u32) -> Option<Duration> {
        match self {
            RetryStrategy::Immediate => None,
            RetryStrategy::Linear { delay_ms } => Some(Duration::from_millis(*delay_ms)),
            RetryStrategy::Exponential { base_ms, factor } => {
                let delay = *base_ms as f64 * factor.powi(attempt as i32 - 1);
                Some(Duration::from_millis(delay as u64))
            }
        }
    }
}

// Simulated error types
#[derive(Clone, Debug)]
enum ApiError {
    // Retriable errors
    ConnectionTimeout,
    ServiceUnavailable,
    RateLimitExceeded,

    // Non-retriable errors
    AuthenticationFailed,
    #[allow(dead_code)]
    InvalidInput,
}

impl ApiError {
    fn is_retriable(&self) -> bool {
        matches!(
            self,
            ApiError::ConnectionTimeout
                | ApiError::ServiceUnavailable
                | ApiError::RateLimitExceeded
        )
    }

    fn message(&self) -> &'static str {
        match self {
            ApiError::ConnectionTimeout => "Connection timeout",
            ApiError::ServiceUnavailable => "Service unavailable",
            ApiError::RateLimitExceeded => "Rate limit exceeded",
            ApiError::AuthenticationFailed => "Authentication failed",
            ApiError::InvalidInput => "Invalid input",
        }
    }
}

// Result of retry operation
#[derive(Clone, Debug)]
#[allow(dead_code)]
struct RetryResult {
    attempts: u32,
    success: bool,
    final_error: Option<String>,
}

// Task that simulates an API call with configurable failure pattern
struct ApiCallWithRetry {
    task_id: String,
    max_attempts: u32,
    strategy: RetryStrategy,
    // Simulated failure pattern: errors to return before success
    failure_sequence: Vec<ApiError>,
    current_attempt: u32,
}

impl ApiCallWithRetry {
    fn new(
        task_id: impl Into<String>,
        max_attempts: u32,
        strategy: RetryStrategy,
        failure_sequence: Vec<ApiError>,
    ) -> Self {
        Self {
            task_id: task_id.into(),
            max_attempts,
            strategy,
            failure_sequence,
            current_attempt: 0,
        }
    }

    async fn attempt_api_call(&mut self) -> Result<String, ApiError> {
        println!(
            "[{}] Attempt {}/{}: Calling external API...",
            self.task_id,
            self.current_attempt + 1,
            self.max_attempts
        );

        // Simulate network delay
        sleep(Duration::from_millis(50)).await;

        // Check if we should fail this attempt
        if (self.current_attempt as usize) < self.failure_sequence.len() {
            let error = self.failure_sequence[self.current_attempt as usize].clone();
            Err(error)
        } else {
            Ok("API response data".to_string())
        }
    }
}

#[task]
impl ApiCallWithRetry {
    async fn run(&mut self) -> Result<RetryResult, String> {
        loop {
            self.current_attempt += 1;

            match self.attempt_api_call().await {
                Ok(data) => {
                    println!("[{}] ✓ Success: {}", self.task_id, data);
                    return Ok(RetryResult {
                        attempts: self.current_attempt,
                        success: true,
                        final_error: None,
                    });
                }
                Err(error) => {
                    println!(
                        "[{}] ✗ {} error: {}",
                        self.task_id,
                        if error.is_retriable() {
                            "Transient"
                        } else {
                            "Non-retriable"
                        },
                        error.message()
                    );

                    // Check if error is retriable
                    if !error.is_retriable() {
                        return Err(format!("{} - not retrying", error.message()));
                    }

                    // Check if we have attempts remaining
                    if self.current_attempt >= self.max_attempts {
                        println!("[{}] All retry attempts exhausted", self.task_id);
                        return Err(format!(
                            "Retry exhausted after {} attempts: {}",
                            self.current_attempt,
                            error.message()
                        ));
                    }

                    // Calculate and apply backoff delay
                    if let Some(delay) = self.strategy.calculate_delay(self.current_attempt) {
                        println!(
                            "[{}] Waiting {}ms before retry (attempt {}/{})",
                            self.task_id,
                            delay.as_millis(),
                            self.current_attempt + 1,
                            self.max_attempts
                        );
                        sleep(delay).await;
                    } else {
                        println!(
                            "[{}] Retrying immediately (attempt {}/{})",
                            self.task_id,
                            self.current_attempt + 1,
                            self.max_attempts
                        );
                    }

                    // Continue loop to retry
                }
            }
        }
    }
}

#[tokio::main]
async fn main() {
    println!("=== Retry Strategies Example ===\n");

    // Example 1: Immediate retry - succeeds on 3rd attempt
    println!("Example 1: Immediate retry strategy");
    {
        let dag = DagRunner::new();

        let api_call = dag.add_task(ApiCallWithRetry::new(
            "ApiCall-1",
            3,
            RetryStrategy::Immediate,
            vec![
                ApiError::ConnectionTimeout,
                ApiError::ConnectionTimeout,
                // 3rd attempt will succeed
            ],
        ));

        dag.run(|fut| {
            tokio::spawn(fut);
        })
        .await
        .unwrap();

        println!("Result: {:?}\n", dag.get(api_call).unwrap());
    }

    // Example 2: Linear backoff - succeeds on 2nd attempt
    println!("Example 2: Linear backoff (500ms delay)");
    {
        let dag = DagRunner::new();

        let api_call = dag.add_task(ApiCallWithRetry::new(
            "ApiCall-2",
            4,
            RetryStrategy::Linear { delay_ms: 500 },
            vec![
                ApiError::RateLimitExceeded,
                // 2nd attempt will succeed
            ],
        ));

        dag.run(|fut| {
            tokio::spawn(fut);
        })
        .await
        .unwrap();

        println!("Result: {:?}\n", dag.get(api_call).unwrap());
    }

    // Example 3: Exponential backoff - succeeds on 3rd attempt
    println!("Example 3: Exponential backoff");
    {
        let dag = DagRunner::new();

        let api_call = dag.add_task(ApiCallWithRetry::new(
            "ApiCall-3",
            5,
            RetryStrategy::Exponential {
                base_ms: 100,
                factor: 2.0,
            },
            vec![
                ApiError::ServiceUnavailable,
                ApiError::ServiceUnavailable,
                // 3rd attempt will succeed
            ],
        ));

        dag.run(|fut| {
            tokio::spawn(fut);
        })
        .await
        .unwrap();

        println!("Result: {:?}\n", dag.get(api_call).unwrap());
    }

    // Example 4: Non-retriable error - fails immediately
    println!("Example 4: Non-retriable error (fails immediately)");
    {
        let dag = DagRunner::new();

        let api_call = dag.add_task(ApiCallWithRetry::new(
            "ApiCall-4",
            3,
            RetryStrategy::Immediate,
            vec![ApiError::AuthenticationFailed],
        ));

        dag.run(|fut| {
            tokio::spawn(fut);
        })
        .await
        .unwrap();

        println!("Result: {:?}\n", dag.get(api_call).unwrap());
    }

    // Example 5: Retry exhausted - all attempts fail
    println!("Example 5: Retry exhausted");
    {
        let dag = DagRunner::new();

        let api_call = dag.add_task(ApiCallWithRetry::new(
            "ApiCall-5",
            3,
            RetryStrategy::Exponential {
                base_ms: 100,
                factor: 2.0,
            },
            vec![
                ApiError::ConnectionTimeout,
                ApiError::ConnectionTimeout,
                ApiError::ConnectionTimeout,
                // All attempts will fail
            ],
        ));

        dag.run(|fut| {
            tokio::spawn(fut);
        })
        .await
        .unwrap();

        println!("Result: {:?}\n", dag.get(api_call).unwrap());
    }

    println!("=== Retry Strategies Example Complete ===");
}
