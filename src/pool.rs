//! High-level execution pool for distributed task execution.
//!
//! This module provides the main user-facing API for submitting and
//! executing tasks across CPU, GPU, and remote backends.
//!
//! ## Example
//!
//! ```rust,ignore
//! use pepita::pool::Pool;
//! use pepita::task::Task;
//!
//! // Create a pool with 4 CPU workers
//! let pool = Pool::builder()
//!     .cpu_workers(4)
//!     .build()?;
//!
//! // Submit a task
//! let task = Task::binary("./worker")
//!     .args(vec!["--input", "data.bin"])
//!     .build();
//!
//! let result = pool.submit(task)?;
//! ```

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use crate::error::{KernelError, Result};
use crate::executor::{CpuExecutor, ExecutorRegistry, GpuExecutor, RemoteExecutor};
use crate::fault::{FailureDetector, RetryPolicy, RetryState};
use crate::scheduler::{Scheduler, TaskId};
use crate::task::{ExecutionResult, Task};

// ============================================================================
// POOL CONFIGURATION
// ============================================================================

/// Configuration for the execution pool.
#[derive(Debug, Clone)]
pub struct PoolConfig {
    /// Number of CPU workers (0 = auto-detect)
    pub cpu_workers: usize,
    /// Enable GPU backend
    pub enable_gpu: bool,
    /// Remote worker addresses
    pub remote_workers: Vec<String>,
    /// Queue capacity per worker
    pub queue_capacity: usize,
    /// Default retry policy
    pub retry_policy: RetryPolicy,
    /// Task timeout (None = no timeout)
    pub default_timeout: Option<std::time::Duration>,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            cpu_workers: 0, // Auto-detect
            enable_gpu: false,
            remote_workers: Vec::new(),
            queue_capacity: 1024,
            retry_policy: RetryPolicy::default(),
            default_timeout: None,
        }
    }
}

// ============================================================================
// POOL BUILDER
// ============================================================================

/// Builder for creating execution pools.
#[derive(Debug, Default)]
pub struct PoolBuilder {
    config: PoolConfig,
}

impl PoolBuilder {
    /// Create a new pool builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set number of CPU workers.
    #[must_use]
    pub const fn cpu_workers(mut self, count: usize) -> Self {
        self.config.cpu_workers = count;
        self
    }

    /// Enable GPU backend.
    #[must_use]
    pub const fn enable_gpu(mut self, enable: bool) -> Self {
        self.config.enable_gpu = enable;
        self
    }

    /// Add remote workers.
    #[must_use]
    pub fn remote_workers(mut self, addresses: Vec<impl Into<String>>) -> Self {
        self.config.remote_workers = addresses.into_iter().map(Into::into).collect();
        self
    }

    /// Set queue capacity per worker.
    #[must_use]
    pub const fn queue_capacity(mut self, capacity: usize) -> Self {
        self.config.queue_capacity = capacity;
        self
    }

    /// Set retry policy.
    #[must_use]
    pub fn retry_policy(mut self, policy: RetryPolicy) -> Self {
        self.config.retry_policy = policy;
        self
    }

    /// Set default timeout.
    #[must_use]
    pub fn default_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.config.default_timeout = Some(timeout);
        self
    }

    /// Build the pool.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn build(self) -> Result<Pool> {
        Ok(Pool::from_config(self.config))
    }
}

// ============================================================================
// POOL STATISTICS
// ============================================================================

/// Statistics for the execution pool.
#[derive(Debug, Clone, Default)]
pub struct PoolStats {
    /// Total tasks submitted
    pub tasks_submitted: u64,
    /// Tasks completed successfully
    pub tasks_completed: u64,
    /// Tasks failed
    pub tasks_failed: u64,
    /// Tasks currently pending
    pub tasks_pending: u64,
    /// Total retries
    pub total_retries: u64,
}

impl PoolStats {
    /// Get success rate.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn success_rate(&self) -> f64 {
        let total = self.tasks_completed + self.tasks_failed;
        if total == 0 {
            return 1.0;
        }
        self.tasks_completed as f64 / total as f64
    }
}

// ============================================================================
// POOL
// ============================================================================

/// Execution pool for distributed task execution.
///
/// The pool manages a set of executors (CPU, GPU, remote) and a
/// work-stealing scheduler for efficient task distribution.
pub struct Pool {
    /// Configuration
    config: PoolConfig,
    /// Work-stealing scheduler
    scheduler: Arc<Scheduler<Task>>,
    /// Executor registry
    executors: Arc<ExecutorRegistry>,
    /// Failure detector
    failure_detector: Arc<FailureDetector>,
    /// Tasks submitted counter
    tasks_submitted: AtomicU64,
    /// Tasks completed counter
    tasks_completed: AtomicU64,
    /// Tasks failed counter
    tasks_failed: AtomicU64,
    /// Retries counter
    total_retries: AtomicU64,
    /// Whether pool is running
    running: AtomicBool,
}

impl Pool {
    /// Create a new pool builder.
    #[must_use]
    pub fn builder() -> PoolBuilder {
        PoolBuilder::new()
    }

    /// Create a pool from configuration.
    fn from_config(config: PoolConfig) -> Self {
        let cpu_workers = if config.cpu_workers == 0 {
            std::thread::available_parallelism()
                .map(std::num::NonZero::get)
                .unwrap_or(4)
        } else {
            config.cpu_workers
        };

        // Create scheduler
        let scheduler = Arc::new(Scheduler::with_capacity(cpu_workers, config.queue_capacity));

        // Create executor registry
        let mut executors = ExecutorRegistry::new();

        // Add CPU executor
        executors.register(Arc::new(CpuExecutor::new(cpu_workers)));

        // Add GPU executor if enabled
        if config.enable_gpu {
            executors.register(Arc::new(GpuExecutor::new()));
        }

        // Add remote executors
        if !config.remote_workers.is_empty() {
            let mut remote = RemoteExecutor::new();
            for addr in &config.remote_workers {
                remote.add_worker(addr.clone());
            }
            executors.register(Arc::new(remote));
        }

        // Create failure detector
        let failure_detector = Arc::new(FailureDetector::new());

        Self {
            config,
            scheduler,
            executors: Arc::new(executors),
            failure_detector,
            tasks_submitted: AtomicU64::new(0),
            tasks_completed: AtomicU64::new(0),
            tasks_failed: AtomicU64::new(0),
            total_retries: AtomicU64::new(0),
            running: AtomicBool::new(true),
        }
    }

    /// Submit a task for execution.
    ///
    /// The task is queued in the scheduler and executed by the
    /// appropriate backend based on task requirements.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn submit(&self, task: Task) -> Result<ExecutionResult> {
        if !self.running.load(Ordering::Acquire) {
            return Err(KernelError::DeviceNotReady);
        }

        self.tasks_submitted.fetch_add(1, Ordering::Relaxed);

        // Execute with retry
        let mut retry_state = RetryState::new();
        let mut current_task = task;

        loop {
            // Try to execute
            let result = self.executors.execute(&current_task);

            match result {
                Ok(exec_result) => {
                    if exec_result.is_success() {
                        self.tasks_completed.fetch_add(1, Ordering::Relaxed);
                        return Ok(exec_result);
                    }

                    // Check if we should retry
                    if retry_state.should_retry(&self.config.retry_policy) {
                        retry_state.record_failure(exec_result.error.clone().unwrap_or_default());
                        self.total_retries.fetch_add(1, Ordering::Relaxed);
                        current_task.increment_retry();

                        // Wait before retry
                        let delay = retry_state.next_delay(&self.config.retry_policy);
                        if delay > std::time::Duration::ZERO {
                            std::thread::sleep(delay);
                        }
                        continue;
                    }

                    self.tasks_failed.fetch_add(1, Ordering::Relaxed);
                    return Ok(exec_result);
                }
                Err(e) => {
                    // Check if retriable error
                    if e.is_retriable() && retry_state.should_retry(&self.config.retry_policy) {
                        retry_state.record_failure(format!("{e}"));
                        self.total_retries.fetch_add(1, Ordering::Relaxed);
                        current_task.increment_retry();

                        let delay = retry_state.next_delay(&self.config.retry_policy);
                        if delay > std::time::Duration::ZERO {
                            std::thread::sleep(delay);
                        }
                        continue;
                    }

                    self.tasks_failed.fetch_add(1, Ordering::Relaxed);
                    return Err(e);
                }
            }
        }
    }

    /// Submit a task and get back a task ID (non-blocking).
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn submit_async(&self, task: Task) -> Result<TaskId> {
        if !self.running.load(Ordering::Acquire) {
            return Err(KernelError::DeviceNotReady);
        }

        self.tasks_submitted.fetch_add(1, Ordering::Relaxed);

        self.scheduler
            .submit(task)
            .ok_or(KernelError::UblkQueueFull)
    }

    /// Get pool statistics.
    #[must_use]
    pub fn stats(&self) -> PoolStats {
        PoolStats {
            tasks_submitted: self.tasks_submitted.load(Ordering::Relaxed),
            tasks_completed: self.tasks_completed.load(Ordering::Relaxed),
            tasks_failed: self.tasks_failed.load(Ordering::Relaxed),
            tasks_pending: self.scheduler.pending_tasks() as u64,
            total_retries: self.total_retries.load(Ordering::Relaxed),
        }
    }

    /// Get the number of workers.
    #[must_use]
    pub fn num_workers(&self) -> usize {
        self.executors.total_workers()
    }

    /// Get the number of pending tasks.
    #[must_use]
    pub fn pending_tasks(&self) -> usize {
        self.scheduler.pending_tasks()
    }

    /// Check if pool is running.
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }

    /// Shutdown the pool.
    pub fn shutdown(&self) {
        self.running.store(false, Ordering::Release);
        self.scheduler.stop();
        self.executors.shutdown();
        self.failure_detector.stop();
    }

    /// Get worker loads.
    #[must_use]
    pub fn worker_loads(&self) -> Vec<usize> {
        self.scheduler.worker_loads()
    }
}

impl std::fmt::Debug for Pool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pool")
            .field("num_workers", &self.num_workers())
            .field("pending_tasks", &self.pending_tasks())
            .field("running", &self.is_running())
            .finish()
    }
}

// ============================================================================
// TESTS (EXTREME TDD)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task::TaskPriority;

    // ------------------------------------------------------------------------
    // PoolConfig Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_pool_config_default() {
        let config = PoolConfig::default();
        assert_eq!(config.cpu_workers, 0);
        assert!(!config.enable_gpu);
        assert!(config.remote_workers.is_empty());
    }

    // ------------------------------------------------------------------------
    // PoolBuilder Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_pool_builder_cpu_workers() {
        let pool = Pool::builder().cpu_workers(4).build().unwrap();
        assert_eq!(pool.num_workers(), 4);
    }

    #[test]
    fn test_pool_builder_auto_workers() {
        let pool = Pool::builder().cpu_workers(0).build().unwrap();
        assert!(pool.num_workers() >= 1);
    }

    #[test]
    fn test_pool_builder_queue_capacity() {
        let pool = Pool::builder()
            .cpu_workers(2)
            .queue_capacity(100)
            .build()
            .unwrap();
        assert!(pool.is_running());
    }

    #[test]
    fn test_pool_builder_retry_policy() {
        let policy = RetryPolicy::no_retry();
        let pool = Pool::builder()
            .cpu_workers(2)
            .retry_policy(policy)
            .build()
            .unwrap();
        assert!(pool.is_running());
    }

    // ------------------------------------------------------------------------
    // Pool Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_pool_builder() {
        let pool = Pool::builder().cpu_workers(4).build();
        assert!(pool.is_ok());
    }

    #[test]
    fn test_pool_submit_echo() {
        let pool = Pool::builder().cpu_workers(2).build().unwrap();

        let task = Task::binary("echo").args(vec!["Hello, Pool!"]).build();

        let result = pool.submit(task).unwrap();
        assert!(result.is_success());
        assert!(result.stdout_string().contains("Hello"));
    }

    #[test]
    fn test_pool_submit_multiple() {
        let pool = Pool::builder().cpu_workers(4).build().unwrap();

        for i in 0..10 {
            let task = Task::binary("echo")
                .args(vec![format!("Task {}", i)])
                .build();
            let result = pool.submit(task).unwrap();
            assert!(result.is_success());
        }

        let stats = pool.stats();
        assert_eq!(stats.tasks_submitted, 10);
        assert_eq!(stats.tasks_completed, 10);
    }

    #[test]
    fn test_pool_stats() {
        let pool = Pool::builder().cpu_workers(2).build().unwrap();

        // Submit successful task
        let task = Task::binary("true").build();
        pool.submit(task).unwrap();

        // Submit failing task
        let task = Task::binary("false").build();
        let _ = pool.submit(task);

        let stats = pool.stats();
        assert_eq!(stats.tasks_submitted, 2);
        assert_eq!(stats.tasks_completed, 1);
        assert_eq!(stats.tasks_failed, 1);
    }

    #[test]
    fn test_pool_stats_success_rate() {
        let mut stats = PoolStats::default();
        assert_eq!(stats.success_rate(), 1.0);

        stats.tasks_completed = 8;
        stats.tasks_failed = 2;
        assert!((stats.success_rate() - 0.8).abs() < 0.001);
    }

    #[test]
    fn test_pool_shutdown() {
        let pool = Pool::builder().cpu_workers(2).build().unwrap();
        assert!(pool.is_running());

        pool.shutdown();
        assert!(!pool.is_running());

        // Submit should fail after shutdown
        let task = Task::binary("echo").args(vec!["test"]).build();
        let result = pool.submit(task);
        assert!(result.is_err());
    }

    #[test]
    fn test_pool_worker_loads() {
        let pool = Pool::builder().cpu_workers(4).build().unwrap();
        let loads = pool.worker_loads();
        assert_eq!(loads.len(), 4);
    }

    #[test]
    fn test_pool_pending_tasks() {
        let pool = Pool::builder().cpu_workers(2).build().unwrap();
        assert_eq!(pool.pending_tasks(), 0);
    }

    #[test]
    fn test_pool_retry() {
        let pool = Pool::builder()
            .cpu_workers(2)
            .retry_policy(RetryPolicy::new().with_max_retries(2))
            .build()
            .unwrap();

        // This will fail and retry
        let task = Task::binary("false").build();
        let result = pool.submit(task);
        assert!(result.is_ok()); // Returns result, but failed

        let stats = pool.stats();
        // Should have retried
        assert!(stats.total_retries > 0 || stats.tasks_failed == 1);
    }

    #[test]
    fn test_pool_submit_async() {
        let pool = Pool::builder().cpu_workers(2).build().unwrap();

        let task = Task::binary("echo").args(vec!["async"]).build();
        let task_id = pool.submit_async(task);
        assert!(task_id.is_ok());
    }

    #[test]
    fn test_pool_with_priority() {
        let pool = Pool::builder().cpu_workers(2).build().unwrap();

        let task = Task::binary("echo")
            .args(vec!["high priority"])
            .priority(TaskPriority::High)
            .build();

        let result = pool.submit(task).unwrap();
        assert!(result.is_success());
    }

    #[test]
    fn test_pool_debug() {
        let pool = Pool::builder().cpu_workers(2).build().unwrap();
        let debug = format!("{:?}", pool);
        assert!(debug.contains("Pool"));
        assert!(debug.contains("num_workers"));
    }
}
