//! # Circuit Breaker Pattern for Resilient Systems
//!
//! This example demonstrates the circuit breaker pattern for protecting systems from
//! cascading failures when downstream services are unhealthy or overloaded.
//!
//! ## What You'll Learn
//! - How to implement a three-state circuit breaker (Closed, Open, Half-Open)
//! - How to share circuit state across multiple task instances
//! - How to implement failure thresholds and automatic recovery
//! - How to provide fallback responses when the circuit is open
//!
//! ## Circuit Breaker States
//!
//! **1. Closed (Normal Operation)**
//! ```text
//! Request → Circuit → Service
//!           (Tracking failures)
//! ```
//!
//! **2. Open (Failing Fast)**
//! ```text
//! Request → Circuit → ✗ Immediate Error
//!           (Not calling service)
//! ```
//!
//! **3. Half-Open (Testing Recovery)**
//! ```text
//! Request → Circuit → Service
//!           (Limited test requests)
//! ```
//!
//! ## State Transitions
//! ```text
//!           Success                 Test Success
//! ┌────────┐      ┌──────────┐     ┌──────────┐
//! │ CLOSED │─────►│   OPEN   │────►│ HALF-OPEN│──┐
//! │        │      │          │     │          │  │
//! └────────┘      └──────────┘     └──────────┘  │
//!     ▲              ▲                    │       │
//!     │              │ Failure            │       │
//!     └──────────────┴────────────────────┘       │
//!                    Test Failure                 │
//!                                                  │
//!     ┌─────────────────────────────────────────┘
//!     Test Success
//! ```
//!
//! ## Workflow Diagram
//! ```text
//! Time: 0ms
//! ┌─────────────┐
//! │Request 1    │──► Circuit: CLOSED ──► Service Call ──► Success
//! └─────────────┘    Failures: 0/3
//!
//! Time: 100ms
//! ┌─────────────┐
//! │Request 2    │──► Circuit: CLOSED ──► Service Call ──► Failure
//! └─────────────┘    Failures: 1/3
//!
//! Time: 200ms
//! ┌─────────────┐
//! │Request 3    │──► Circuit: CLOSED ──► Service Call ──► Failure
//! └─────────────┘    Failures: 2/3
//!
//! Time: 300ms
//! ┌─────────────┐
//! │Request 4    │──► Circuit: CLOSED ──► Service Call ──► Failure
//! └─────────────┘    Failures: 3/3 ──► Circuit OPENS!
//!
//! Time: 400ms
//! ┌─────────────┐
//! │Request 5    │──► Circuit: OPEN ──► ✗ Fail Fast (no call)
//! └─────────────┘                       Use Fallback
//!
//! Time: 5000ms (timeout expires)
//! ┌─────────────┐
//! │Request 6    │──► Circuit: HALF-OPEN ──► Service Call ──► Success
//! └─────────────┘    Circuit CLOSES!
//! ```
//!
//! ## Key Concepts
//! - **Fail fast**: When circuit is open, don't wait for timeout - fail immediately
//! - **Shared state**: Circuit state is shared across all task instances
//! - **Automatic recovery**: Circuit automatically tests recovery after timeout
//! - **Failure threshold**: Number of failures before opening circuit
//! - **Recovery timeout**: How long to wait before testing recovery
//! - **Fallback strategies**: Alternative responses when service is unavailable
//!
//! ## Use Cases
//! - Protecting against cascading failures in microservices
//! - API rate limiting and backpressure
//! - Database connection pool exhaustion
//! - Preventing resource exhaustion from slow/down services
//! - Graceful degradation with fallback responses
//!
//! ## Running This Example
//! ```bash
//! cargo run --example circuit_breaker
//! ```
//!
//! ## Expected Output
//! ```text
//! === Circuit Breaker Example ===
//!
//! Example 1: Circuit opening after failures
//!
//! [ServiceCall-1] Circuit state: Closed (failures: 0/3)
//! [ServiceCall-1] Calling external service...
//! [ServiceCall-1] ✓ Service responded: Success
//! Result 1: Ok("Success")
//!
//! [ServiceCall-2] Circuit state: Closed (failures: 0/3)
//! [ServiceCall-2] Calling external service...
//! [ServiceCall-2] ✗ Service failed: Service error
//! [ServiceCall-2] Failure recorded (1/3)
//! Result 2: Err("Service error")
//!
//! [ServiceCall-3] Circuit state: Closed (failures: 1/3)
//! [ServiceCall-3] Calling external service...
//! [ServiceCall-3] ✗ Service failed: Service error
//! [ServiceCall-3] Failure recorded (2/3)
//! Result 3: Err("Service error")
//!
//! [ServiceCall-4] Circuit state: Closed (failures: 2/3)
//! [ServiceCall-4] Calling external service...
//! [ServiceCall-4] ✗ Service failed: Service error
//! [ServiceCall-4] Failure threshold reached! Circuit opening...
//! [ServiceCall-4] Circuit state: OPEN
//! Result 4: Err("Service error")
//!
//! [ServiceCall-5] Circuit state: Open
//! [ServiceCall-5] ✗ Circuit is open - failing fast
//! [ServiceCall-5] Using fallback response
//! Result 5: Ok("Fallback: cached response")
//!
//! Example 2: Circuit recovery after timeout
//!
//! Waiting 3 seconds for circuit recovery timeout...
//!
//! [ServiceCall-6] Circuit state: Half-Open (testing recovery)
//! [ServiceCall-6] Calling external service...
//! [ServiceCall-6] ✓ Service responded: Success
//! [ServiceCall-6] Recovery test succeeded! Circuit closing...
//! [ServiceCall-6] Circuit state: CLOSED
//! Result 6: Ok("Success")
//!
//! [ServiceCall-7] Circuit state: Closed (failures: 0/3)
//! [ServiceCall-7] Calling external service...
//! [ServiceCall-7] ✓ Service responded: Success
//! Result 7: Ok("Success")
//! ```

use dagx::{task, DagRunner};

use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};

// Circuit breaker states
#[derive(Clone, Debug, PartialEq)]
enum CircuitState {
    Closed { failure_count: u32 },
    Open { opened_at: Instant },
    HalfOpen,
}

// Shared circuit breaker
struct CircuitBreaker {
    state: Arc<Mutex<CircuitState>>,
    failure_threshold: u32,
    recovery_timeout: Duration,
}

impl CircuitBreaker {
    fn new(failure_threshold: u32, recovery_timeout: Duration) -> Self {
        Self {
            state: Arc::new(Mutex::new(CircuitState::Closed { failure_count: 0 })),
            failure_threshold,
            recovery_timeout,
        }
    }

    async fn check_state(&self) -> CircuitState {
        let mut state = self.state.lock().await;

        // Check if we should transition from Open to Half-Open
        if let CircuitState::Open { opened_at } = *state {
            if opened_at.elapsed() >= self.recovery_timeout {
                *state = CircuitState::HalfOpen;
                println!("Circuit transitioning: Open → Half-Open");
            }
        }

        state.clone()
    }

    async fn record_success(&self) {
        let mut state = self.state.lock().await;

        match *state {
            CircuitState::HalfOpen => {
                // Recovery test succeeded - close the circuit
                *state = CircuitState::Closed { failure_count: 0 };
                println!("[CircuitBreaker] Recovery test succeeded! Circuit closing...");
            }
            CircuitState::Closed { .. } => {
                // Reset failure count on success
                *state = CircuitState::Closed { failure_count: 0 };
            }
            CircuitState::Open { .. } => {
                // Shouldn't happen, but reset if it does
                *state = CircuitState::Closed { failure_count: 0 };
            }
        }
    }

    async fn record_failure(&self) {
        let mut state = self.state.lock().await;

        match *state {
            CircuitState::Closed { failure_count } => {
                let new_count = failure_count + 1;

                if new_count >= self.failure_threshold {
                    *state = CircuitState::Open {
                        opened_at: Instant::now(),
                    };
                    println!("[CircuitBreaker] Failure threshold reached! Circuit opening...");
                } else {
                    *state = CircuitState::Closed {
                        failure_count: new_count,
                    };
                    println!(
                        "[CircuitBreaker] Failure recorded ({}/{})",
                        new_count, self.failure_threshold
                    );
                }
            }
            CircuitState::HalfOpen => {
                // Recovery test failed - reopen circuit
                *state = CircuitState::Open {
                    opened_at: Instant::now(),
                };
                println!("[CircuitBreaker] Recovery test failed! Circuit reopening...");
            }
            CircuitState::Open { .. } => {
                // Already open, no action needed
            }
        }
    }
}

// Service call protected by circuit breaker
struct ProtectedServiceCall {
    task_id: String,
    circuit: Arc<CircuitBreaker>,
    should_fail: bool,
}

impl ProtectedServiceCall {
    fn new(task_id: impl Into<String>, circuit: Arc<CircuitBreaker>, should_fail: bool) -> Self {
        Self {
            task_id: task_id.into(),
            circuit,
            should_fail,
        }
    }

    async fn call_service(&self) -> Result<String, String> {
        println!("[{}] Calling external service...", self.task_id);

        // Simulate service call
        sleep(Duration::from_millis(100)).await;

        if self.should_fail {
            Err("Service error".to_string())
        } else {
            Ok("Success".to_string())
        }
    }
}

#[task]
impl ProtectedServiceCall {
    async fn run(&mut self) -> Result<String, String> {
        let state = self.circuit.check_state().await;

        // Print current circuit state
        match &state {
            CircuitState::Closed { failure_count } => {
                println!(
                    "[{}] Circuit state: Closed (failures: {}/{})",
                    self.task_id, failure_count, self.circuit.failure_threshold
                );
            }
            CircuitState::Open { .. } => {
                println!("[{}] Circuit state: Open", self.task_id);
            }
            CircuitState::HalfOpen => {
                println!(
                    "[{}] Circuit state: Half-Open (testing recovery)",
                    self.task_id
                );
            }
        }

        // Check if circuit is open
        if matches!(state, CircuitState::Open { .. }) {
            println!("[{}] ✗ Circuit is open - failing fast", self.task_id);
            return Err("Circuit breaker is open".to_string());
        }

        // Attempt the service call
        match self.call_service().await {
            Ok(response) => {
                println!("[{}] ✓ Service responded: {}", self.task_id, response);
                self.circuit.record_success().await;

                // Print new state if it changed
                let new_state = self.circuit.check_state().await;
                if !matches!(new_state, CircuitState::Closed { failure_count: 0 }) {
                    println!("[{}] Circuit state: {:?}", self.task_id, new_state);
                }

                Ok(response)
            }
            Err(error) => {
                println!("[{}] ✗ Service failed: {}", self.task_id, error);
                self.circuit.record_failure().await;

                // Print new state if it changed
                let new_state = self.circuit.check_state().await;
                if matches!(new_state, CircuitState::Open { .. }) {
                    println!("[{}] Circuit state: OPEN", self.task_id);
                }

                Err(error)
            }
        }
    }
}

// Fallback task that provides cached/default response when circuit is open
struct FallbackService {
    task_id: String,
}

impl FallbackService {
    fn new(task_id: impl Into<String>) -> Self {
        Self {
            task_id: task_id.into(),
        }
    }
}

#[task]
impl FallbackService {
    async fn run(&mut self, result: &Result<String, String>) -> Result<String, String> {
        match result {
            Ok(response) => Ok(response.clone()),
            Err(error) => {
                if error.contains("Circuit breaker is open") {
                    println!("[{}] Using fallback response", self.task_id);
                    Ok("Fallback: cached response".to_string())
                } else {
                    Err(error.clone())
                }
            }
        }
    }
}

#[tokio::main]
async fn main() {
    println!("=== Circuit Breaker Example ===\n");

    // Example 1: Circuit opening after threshold failures
    println!("Example 1: Circuit opening after failures\n");
    {
        // Create shared circuit breaker: 3 failures trigger open, 3 second recovery timeout
        let circuit = Arc::new(CircuitBreaker::new(3, Duration::from_secs(3)));

        let dag = DagRunner::new();

        // Request 1: Success (circuit closed)
        let call1 = dag.add_task(ProtectedServiceCall::new(
            "ServiceCall-1",
            circuit.clone(),
            false, // success
        ));

        dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() })
            .await
            .unwrap();

        println!("Result 1: {:?}\n", dag.get(call1).unwrap());

        // Request 2: Failure (circuit closed, count = 1)
        let dag = DagRunner::new();
        let call2 = dag.add_task(ProtectedServiceCall::new(
            "ServiceCall-2",
            circuit.clone(),
            true, // fail
        ));

        dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() })
            .await
            .unwrap();

        println!("Result 2: {:?}\n", dag.get(call2).unwrap());

        // Request 3: Failure (circuit closed, count = 2)
        let dag = DagRunner::new();
        let call3 = dag.add_task(ProtectedServiceCall::new(
            "ServiceCall-3",
            circuit.clone(),
            true, // fail
        ));

        dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() })
            .await
            .unwrap();

        println!("Result 3: {:?}\n", dag.get(call3).unwrap());

        // Request 4: Failure (circuit closed, count = 3, opens!)
        let dag = DagRunner::new();
        let call4 = dag.add_task(ProtectedServiceCall::new(
            "ServiceCall-4",
            circuit.clone(),
            true, // fail
        ));

        dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() })
            .await
            .unwrap();

        println!("Result 4: {:?}\n", dag.get(call4).unwrap());

        // Request 5: Circuit is now open, fails fast with fallback
        let dag = DagRunner::new();
        let call5 = dag.add_task(ProtectedServiceCall::new(
            "ServiceCall-5",
            circuit.clone(),
            false,
        ));

        let fallback5 = dag
            .add_task(FallbackService::new("ServiceCall-5"))
            .depends_on(call5);

        dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() })
            .await
            .unwrap();

        println!("Result 5: {:?}\n", dag.get(fallback5).unwrap());

        // Example 2: Circuit recovery after timeout
        println!("Example 2: Circuit recovery after timeout\n");
        println!("Waiting 3 seconds for circuit recovery timeout...\n");
        sleep(Duration::from_secs(3)).await;

        // Request 6: Circuit should be half-open, test succeeds, closes circuit
        let dag = DagRunner::new();
        let call6 = dag.add_task(ProtectedServiceCall::new(
            "ServiceCall-6",
            circuit.clone(),
            false, // success
        ));

        dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() })
            .await
            .unwrap();

        println!("Result 6: {:?}\n", dag.get(call6).unwrap());

        // Request 7: Circuit is now closed again, normal operation
        let dag = DagRunner::new();
        let call7 = dag.add_task(ProtectedServiceCall::new(
            "ServiceCall-7",
            circuit.clone(),
            false, // success
        ));

        dag.run(|fut| async move { tokio::spawn(fut).await.unwrap() })
            .await
            .unwrap();

        println!("Result 7: {:?}\n", dag.get(call7).unwrap());
    }

    println!("=== Circuit Breaker Example Complete ===");
}
