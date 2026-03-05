//! Fault tolerance mechanisms for distributed execution.
//!
//! This module implements fault detection and recovery strategies
//! based on the Chandra-Toueg failure detector model.
//!
//! ## Features
//!
//! - Heartbeat-based failure detection
//! - Retry with exponential backoff
//! - Worker health tracking
//! - Task migration on failure
//!
//! ## Reference
//!
//! Chandra, T. D., & Toueg, S. (1996). "Unreliable Failure Detectors
//! for Reliable Distributed Systems." Journal of the ACM, 43(2).

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::RwLock;
use std::time::{Duration, Instant};

use crate::scheduler::WorkerId;

// ============================================================================
// RETRY POLICY
// ============================================================================

/// Retry policy for failed tasks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetryPolicy {
    /// Maximum retry attempts
    pub max_retries: u32,
    /// Initial delay between retries
    pub initial_delay: Duration,
    /// Maximum delay between retries
    pub max_delay: Duration,
    /// Backoff multiplier
    pub backoff_factor: u32,
    /// Whether to use jitter
    pub use_jitter: bool,
}

impl RetryPolicy {
    /// Create a new retry policy with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a policy with no retries.
    #[must_use]
    pub fn no_retry() -> Self {
        Self {
            max_retries: 0,
            ..Self::default()
        }
    }

    /// Create a policy for critical tasks (more retries, longer delays).
    #[must_use]
    pub fn critical() -> Self {
        Self {
            max_retries: 10,
            initial_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(60),
            backoff_factor: 2,
            use_jitter: true,
        }
    }

    /// Set maximum retries.
    #[must_use]
    pub const fn with_max_retries(mut self, retries: u32) -> Self {
        self.max_retries = retries;
        self
    }

    /// Set initial delay.
    #[must_use]
    pub const fn with_initial_delay(mut self, delay: Duration) -> Self {
        self.initial_delay = delay;
        self
    }

    /// Set maximum delay.
    #[must_use]
    pub const fn with_max_delay(mut self, delay: Duration) -> Self {
        self.max_delay = delay;
        self
    }

    /// Calculate delay for a given retry attempt.
    #[must_use]
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        if attempt == 0 {
            return Duration::ZERO;
        }

        let base_delay = self.initial_delay.as_millis() as u64;
        let factor = self.backoff_factor.pow(attempt.saturating_sub(1)) as u64;
        let delay_ms = base_delay.saturating_mul(factor);

        let capped_ms = delay_ms.min(self.max_delay.as_millis() as u64);

        Duration::from_millis(capped_ms)
    }

    /// Check if should retry given current attempt count.
    #[must_use]
    pub const fn should_retry(&self, current_attempts: u32) -> bool {
        current_attempts < self.max_retries
    }
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(30),
            backoff_factor: 2,
            use_jitter: true,
        }
    }
}

// ============================================================================
// RETRY STATE
// ============================================================================

/// State for tracking retry attempts.
#[derive(Debug, Clone)]
pub struct RetryState {
    /// Number of attempts made
    pub attempts: u32,
    /// Last failure time
    pub last_failure: Option<Instant>,
    /// Last error message
    pub last_error: Option<String>,
}

impl RetryState {
    /// Create a new retry state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            attempts: 0,
            last_failure: None,
            last_error: None,
        }
    }

    /// Record a failure.
    pub fn record_failure(&mut self, error: impl Into<String>) {
        self.attempts += 1;
        self.last_failure = Some(Instant::now());
        self.last_error = Some(error.into());
    }

    /// Reset the state (e.g., after success).
    pub fn reset(&mut self) {
        self.attempts = 0;
        self.last_failure = None;
        self.last_error = None;
    }

    /// Check if should retry given a policy.
    #[must_use]
    pub fn should_retry(&self, policy: &RetryPolicy) -> bool {
        policy.should_retry(self.attempts)
    }

    /// Get delay before next retry.
    #[must_use]
    pub fn next_delay(&self, policy: &RetryPolicy) -> Duration {
        policy.delay_for_attempt(self.attempts)
    }

    /// Check if currently in backoff period.
    #[must_use]
    pub fn in_backoff(&self, policy: &RetryPolicy) -> bool {
        if let Some(last) = self.last_failure {
            let delay = self.next_delay(policy);
            last.elapsed() < delay
        } else {
            false
        }
    }
}

impl Default for RetryState {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// WORKER HEALTH
// ============================================================================

/// Health status of a worker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    /// Worker is healthy
    Healthy,
    /// Worker is suspected (missed some heartbeats)
    Suspected,
    /// Worker is marked as failed
    Failed,
    /// Worker is unknown/new
    Unknown,
}

impl HealthStatus {
    /// Check if worker is considered available.
    #[must_use]
    pub const fn is_available(self) -> bool {
        matches!(self, Self::Healthy | Self::Unknown)
    }
}

/// Health information for a worker.
#[derive(Debug, Clone)]
pub struct WorkerHealth {
    /// Worker ID
    pub worker_id: WorkerId,
    /// Current status
    pub status: HealthStatus,
    /// Last heartbeat time
    pub last_heartbeat: Option<Instant>,
    /// Consecutive failures
    pub consecutive_failures: u32,
    /// Total tasks completed
    pub tasks_completed: u64,
    /// Total tasks failed
    pub tasks_failed: u64,
}

impl WorkerHealth {
    /// Create a new health record.
    #[must_use]
    pub fn new(worker_id: WorkerId) -> Self {
        Self {
            worker_id,
            status: HealthStatus::Unknown,
            last_heartbeat: None,
            consecutive_failures: 0,
            tasks_completed: 0,
            tasks_failed: 0,
        }
    }

    /// Record a heartbeat.
    pub fn record_heartbeat(&mut self) {
        self.last_heartbeat = Some(Instant::now());
        self.status = HealthStatus::Healthy;
        self.consecutive_failures = 0;
    }

    /// Record a task completion.
    pub fn record_task_completion(&mut self, success: bool) {
        if success {
            self.tasks_completed += 1;
            self.consecutive_failures = 0;
        } else {
            self.tasks_failed += 1;
            self.consecutive_failures += 1;
        }
    }

    /// Update status based on heartbeat timeout.
    pub fn update_status(&mut self, heartbeat_timeout: Duration, failure_threshold: u32) {
        if let Some(last) = self.last_heartbeat {
            let elapsed = last.elapsed();
            if elapsed > heartbeat_timeout * 3 {
                self.status = HealthStatus::Failed;
            } else if elapsed > heartbeat_timeout {
                self.status = HealthStatus::Suspected;
            }
        }

        if self.consecutive_failures >= failure_threshold {
            self.status = HealthStatus::Failed;
        }
    }

    /// Get success rate.
    #[must_use]
    pub fn success_rate(&self) -> f64 {
        let total = self.tasks_completed + self.tasks_failed;
        if total == 0 {
            return 1.0;
        }
        self.tasks_completed as f64 / total as f64
    }
}

// ============================================================================
// FAILURE DETECTOR
// ============================================================================

/// Heartbeat-based failure detector.
///
/// Implements an unreliable failure detector following the
/// Chandra-Toueg model.
#[derive(Debug)]
pub struct FailureDetector {
    /// Worker health records
    workers: RwLock<HashMap<WorkerId, WorkerHealth>>,
    /// Heartbeat interval
    heartbeat_interval: Duration,
    /// Heartbeat timeout (consider suspect after this)
    heartbeat_timeout: Duration,
    /// Failure threshold (consecutive failures before marking failed)
    failure_threshold: u32,
    /// Whether detector is running
    running: AtomicBool,
}

impl FailureDetector {
    /// Create a new failure detector.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(Duration::from_secs(1), Duration::from_secs(3), 3)
    }

    /// Create with custom configuration.
    #[must_use]
    pub fn with_config(
        heartbeat_interval: Duration,
        heartbeat_timeout: Duration,
        failure_threshold: u32,
    ) -> Self {
        Self {
            workers: RwLock::new(HashMap::new()),
            heartbeat_interval,
            heartbeat_timeout,
            failure_threshold,
            running: AtomicBool::new(true),
        }
    }

    /// Register a worker.
    pub fn register_worker(&self, worker_id: WorkerId) {
        let mut workers = self.workers.write().unwrap();
        workers.insert(worker_id, WorkerHealth::new(worker_id));
    }

    /// Deregister a worker.
    pub fn deregister_worker(&self, worker_id: WorkerId) {
        let mut workers = self.workers.write().unwrap();
        workers.remove(&worker_id);
    }

    /// Record a heartbeat from a worker.
    pub fn record_heartbeat(&self, worker_id: WorkerId) {
        let mut workers = self.workers.write().unwrap();
        if let Some(health) = workers.get_mut(&worker_id) {
            health.record_heartbeat();
        } else {
            // Auto-register on first heartbeat
            let mut health = WorkerHealth::new(worker_id);
            health.record_heartbeat();
            workers.insert(worker_id, health);
        }
    }

    /// Record task completion.
    pub fn record_task_result(&self, worker_id: WorkerId, success: bool) {
        let mut workers = self.workers.write().unwrap();
        if let Some(health) = workers.get_mut(&worker_id) {
            health.record_task_completion(success);
        }
    }

    /// Get worker health status.
    #[must_use]
    pub fn get_status(&self, worker_id: WorkerId) -> HealthStatus {
        let workers = self.workers.read().unwrap();
        workers
            .get(&worker_id)
            .map(|h| h.status)
            .unwrap_or(HealthStatus::Unknown)
    }

    /// Get worker health record.
    #[must_use]
    pub fn get_health(&self, worker_id: WorkerId) -> Option<WorkerHealth> {
        let workers = self.workers.read().unwrap();
        workers.get(&worker_id).cloned()
    }

    /// Get all healthy workers.
    #[must_use]
    pub fn healthy_workers(&self) -> Vec<WorkerId> {
        let workers = self.workers.read().unwrap();
        workers
            .iter()
            .filter(|(_, h)| h.status == HealthStatus::Healthy)
            .map(|(id, _)| *id)
            .collect()
    }

    /// Get all failed workers.
    #[must_use]
    pub fn failed_workers(&self) -> Vec<WorkerId> {
        let workers = self.workers.read().unwrap();
        workers
            .iter()
            .filter(|(_, h)| h.status == HealthStatus::Failed)
            .map(|(id, _)| *id)
            .collect()
    }

    /// Update all worker statuses based on heartbeat timeouts.
    pub fn update_all_statuses(&self) {
        let mut workers = self.workers.write().unwrap();
        for health in workers.values_mut() {
            health.update_status(self.heartbeat_timeout, self.failure_threshold);
        }
    }

    /// Check if a worker is available.
    #[must_use]
    pub fn is_available(&self, worker_id: WorkerId) -> bool {
        self.get_status(worker_id).is_available()
    }

    /// Get the heartbeat interval.
    #[must_use]
    pub const fn heartbeat_interval(&self) -> Duration {
        self.heartbeat_interval
    }

    /// Stop the detector.
    pub fn stop(&self) {
        self.running.store(false, Ordering::Release);
    }

    /// Check if running.
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }
}

impl Default for FailureDetector {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// CIRCUIT BREAKER
// ============================================================================

/// Circuit breaker state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// Circuit is closed (normal operation)
    Closed,
    /// Circuit is open (failing fast)
    Open,
    /// Circuit is half-open (testing)
    HalfOpen,
}

/// Circuit breaker for protecting against cascading failures.
#[derive(Debug)]
pub struct CircuitBreaker {
    /// Current state
    state: RwLock<CircuitState>,
    /// Failure count
    failure_count: AtomicU64,
    /// Success count (for half-open)
    success_count: AtomicU64,
    /// Failure threshold to open
    failure_threshold: u64,
    /// Success threshold to close (from half-open)
    success_threshold: u64,
    /// Time to wait in open state before half-open
    reset_timeout: Duration,
    /// Time circuit was opened
    opened_at: RwLock<Option<Instant>>,
}

impl CircuitBreaker {
    /// Create a new circuit breaker.
    #[must_use]
    pub fn new(failure_threshold: u64, success_threshold: u64, reset_timeout: Duration) -> Self {
        Self {
            state: RwLock::new(CircuitState::Closed),
            failure_count: AtomicU64::new(0),
            success_count: AtomicU64::new(0),
            failure_threshold,
            success_threshold,
            reset_timeout,
            opened_at: RwLock::new(None),
        }
    }

    /// Get current state.
    #[must_use]
    pub fn state(&self) -> CircuitState {
        *self.state.read().unwrap()
    }

    /// Check if circuit allows execution.
    #[must_use]
    pub fn allows_execution(&self) -> bool {
        let state = *self.state.read().unwrap();
        match state {
            CircuitState::Closed => true,
            CircuitState::HalfOpen => true,
            CircuitState::Open => {
                // Check if we should transition to half-open
                let opened_at = self.opened_at.read().unwrap();
                if let Some(opened) = *opened_at {
                    if opened.elapsed() >= self.reset_timeout {
                        drop(opened_at);
                        self.transition_to_half_open();
                        return true;
                    }
                }
                false
            }
        }
    }

    /// Record a success.
    pub fn record_success(&self) {
        let state = *self.state.read().unwrap();
        match state {
            CircuitState::Closed => {
                self.failure_count.store(0, Ordering::Release);
            }
            CircuitState::HalfOpen => {
                let count = self.success_count.fetch_add(1, Ordering::AcqRel) + 1;
                if count >= self.success_threshold {
                    self.transition_to_closed();
                }
            }
            CircuitState::Open => {}
        }
    }

    /// Record a failure.
    pub fn record_failure(&self) {
        let state = *self.state.read().unwrap();
        match state {
            CircuitState::Closed => {
                let count = self.failure_count.fetch_add(1, Ordering::AcqRel) + 1;
                if count >= self.failure_threshold {
                    self.transition_to_open();
                }
            }
            CircuitState::HalfOpen => {
                self.transition_to_open();
            }
            CircuitState::Open => {}
        }
    }

    fn transition_to_open(&self) {
        let mut state = self.state.write().unwrap();
        *state = CircuitState::Open;
        let mut opened_at = self.opened_at.write().unwrap();
        *opened_at = Some(Instant::now());
        self.success_count.store(0, Ordering::Release);
    }

    fn transition_to_half_open(&self) {
        let mut state = self.state.write().unwrap();
        *state = CircuitState::HalfOpen;
        self.success_count.store(0, Ordering::Release);
        self.failure_count.store(0, Ordering::Release);
    }

    fn transition_to_closed(&self) {
        let mut state = self.state.write().unwrap();
        *state = CircuitState::Closed;
        let mut opened_at = self.opened_at.write().unwrap();
        *opened_at = None;
        self.failure_count.store(0, Ordering::Release);
        self.success_count.store(0, Ordering::Release);
    }

    /// Reset the circuit breaker.
    pub fn reset(&self) {
        self.transition_to_closed();
    }
}

impl Default for CircuitBreaker {
    fn default() -> Self {
        Self::new(5, 3, Duration::from_secs(30))
    }
}

// ============================================================================
// TESTS (EXTREME TDD)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------------------
    // RetryPolicy Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_retry_policy_default() {
        let policy = RetryPolicy::default();
        assert_eq!(policy.max_retries, 3);
        assert!(policy.should_retry(0));
        assert!(policy.should_retry(2));
        assert!(!policy.should_retry(3));
    }

    #[test]
    fn test_retry_policy_no_retry() {
        let policy = RetryPolicy::no_retry();
        assert_eq!(policy.max_retries, 0);
        assert!(!policy.should_retry(0));
    }

    #[test]
    fn test_retry_policy_critical() {
        let policy = RetryPolicy::critical();
        assert_eq!(policy.max_retries, 10);
        assert!(policy.should_retry(9));
        assert!(!policy.should_retry(10));
    }

    #[test]
    fn test_retry_policy_delay() {
        let policy = RetryPolicy::new()
            .with_initial_delay(Duration::from_millis(100))
            .with_backoff_factor(2);

        assert_eq!(policy.delay_for_attempt(0), Duration::ZERO);
        assert_eq!(policy.delay_for_attempt(1), Duration::from_millis(100));
        assert_eq!(policy.delay_for_attempt(2), Duration::from_millis(200));
        assert_eq!(policy.delay_for_attempt(3), Duration::from_millis(400));
    }

    #[test]
    fn test_retry_policy_delay_capped() {
        let policy = RetryPolicy::new()
            .with_initial_delay(Duration::from_secs(1))
            .with_max_delay(Duration::from_secs(5))
            .with_backoff_factor(2);

        // At attempt 4: 1 * 2^3 = 8 seconds, but capped at 5
        assert_eq!(policy.delay_for_attempt(4), Duration::from_secs(5));
    }

    impl RetryPolicy {
        fn with_backoff_factor(mut self, factor: u32) -> Self {
            self.backoff_factor = factor;
            self
        }
    }

    // ------------------------------------------------------------------------
    // RetryState Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_retry_state_new() {
        let state = RetryState::new();
        assert_eq!(state.attempts, 0);
        assert!(state.last_failure.is_none());
        assert!(state.last_error.is_none());
    }

    #[test]
    fn test_retry_state_record_failure() {
        let mut state = RetryState::new();
        state.record_failure("Error 1");
        assert_eq!(state.attempts, 1);
        assert!(state.last_failure.is_some());
        assert_eq!(state.last_error, Some("Error 1".to_string()));

        state.record_failure("Error 2");
        assert_eq!(state.attempts, 2);
        assert_eq!(state.last_error, Some("Error 2".to_string()));
    }

    #[test]
    fn test_retry_state_reset() {
        let mut state = RetryState::new();
        state.record_failure("Error");
        state.reset();
        assert_eq!(state.attempts, 0);
        assert!(state.last_failure.is_none());
    }

    #[test]
    fn test_retry_state_should_retry() {
        let policy = RetryPolicy::new().with_max_retries(2);
        let mut state = RetryState::new();

        assert!(state.should_retry(&policy));
        state.record_failure("Error");
        assert!(state.should_retry(&policy));
        state.record_failure("Error");
        assert!(!state.should_retry(&policy));
    }

    // ------------------------------------------------------------------------
    // WorkerHealth Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_worker_health_new() {
        let health = WorkerHealth::new(WorkerId(1));
        assert_eq!(health.worker_id, WorkerId(1));
        assert_eq!(health.status, HealthStatus::Unknown);
        assert!(health.last_heartbeat.is_none());
    }

    #[test]
    fn test_worker_health_record_heartbeat() {
        let mut health = WorkerHealth::new(WorkerId(1));
        health.record_heartbeat();
        assert_eq!(health.status, HealthStatus::Healthy);
        assert!(health.last_heartbeat.is_some());
    }

    #[test]
    fn test_worker_health_task_completion() {
        let mut health = WorkerHealth::new(WorkerId(1));

        health.record_task_completion(true);
        assert_eq!(health.tasks_completed, 1);
        assert_eq!(health.tasks_failed, 0);

        health.record_task_completion(false);
        assert_eq!(health.tasks_completed, 1);
        assert_eq!(health.tasks_failed, 1);
    }

    #[test]
    fn test_worker_health_success_rate() {
        let mut health = WorkerHealth::new(WorkerId(1));
        assert_eq!(health.success_rate(), 1.0);

        health.record_task_completion(true);
        health.record_task_completion(true);
        health.record_task_completion(false);
        assert!((health.success_rate() - 0.666).abs() < 0.01);
    }

    #[test]
    fn test_health_status_is_available() {
        assert!(HealthStatus::Healthy.is_available());
        assert!(HealthStatus::Unknown.is_available());
        assert!(!HealthStatus::Suspected.is_available());
        assert!(!HealthStatus::Failed.is_available());
    }

    // ------------------------------------------------------------------------
    // FailureDetector Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_failure_detector_new() {
        let detector = FailureDetector::new();
        assert!(detector.is_running());
    }

    #[test]
    fn test_failure_detector_register() {
        let detector = FailureDetector::new();
        detector.register_worker(WorkerId(1));

        assert_eq!(detector.get_status(WorkerId(1)), HealthStatus::Unknown);
    }

    #[test]
    fn test_failure_detector_heartbeat() {
        let detector = FailureDetector::new();
        detector.register_worker(WorkerId(1));
        detector.record_heartbeat(WorkerId(1));

        assert_eq!(detector.get_status(WorkerId(1)), HealthStatus::Healthy);
    }

    #[test]
    fn test_failure_detector_healthy_workers() {
        let detector = FailureDetector::new();
        detector.register_worker(WorkerId(1));
        detector.register_worker(WorkerId(2));

        detector.record_heartbeat(WorkerId(1));

        let healthy = detector.healthy_workers();
        assert_eq!(healthy.len(), 1);
        assert!(healthy.contains(&WorkerId(1)));
    }

    #[test]
    fn test_failure_detector_auto_register() {
        let detector = FailureDetector::new();
        detector.record_heartbeat(WorkerId(99));

        assert_eq!(detector.get_status(WorkerId(99)), HealthStatus::Healthy);
    }

    #[test]
    fn test_failure_detector_deregister() {
        let detector = FailureDetector::new();
        detector.register_worker(WorkerId(1));
        detector.deregister_worker(WorkerId(1));

        assert_eq!(detector.get_status(WorkerId(1)), HealthStatus::Unknown);
    }

    // ------------------------------------------------------------------------
    // CircuitBreaker Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_circuit_breaker_initial_state() {
        let cb = CircuitBreaker::default();
        assert_eq!(cb.state(), CircuitState::Closed);
        assert!(cb.allows_execution());
    }

    #[test]
    fn test_circuit_breaker_opens_on_failures() {
        let cb = CircuitBreaker::new(3, 2, Duration::from_secs(1));

        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Closed);

        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
        assert!(!cb.allows_execution());
    }

    #[test]
    fn test_circuit_breaker_success_resets_count() {
        let cb = CircuitBreaker::new(3, 2, Duration::from_secs(1));

        cb.record_failure();
        cb.record_failure();
        cb.record_success();
        cb.record_failure();
        cb.record_failure();

        // Should still be closed (success reset the count)
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[test]
    fn test_circuit_breaker_half_open_closes() {
        let cb = CircuitBreaker::new(1, 2, Duration::from_millis(10));

        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);

        std::thread::sleep(Duration::from_millis(20));
        assert!(cb.allows_execution()); // Should transition to half-open

        cb.record_success();
        cb.record_success();
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[test]
    fn test_circuit_breaker_half_open_reopens_on_failure() {
        let cb = CircuitBreaker::new(1, 2, Duration::from_millis(10));

        cb.record_failure();
        std::thread::sleep(Duration::from_millis(20));
        let _ = cb.allows_execution(); // Trigger half-open

        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
    }

    #[test]
    fn test_circuit_breaker_reset() {
        let cb = CircuitBreaker::new(1, 2, Duration::from_secs(100));

        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);

        cb.reset();
        assert_eq!(cb.state(), CircuitState::Closed);
        assert!(cb.allows_execution());
    }
}
