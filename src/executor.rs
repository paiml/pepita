//! Executor backends for task execution.
//!
//! This module provides the executor abstraction and implementations
//! for different compute backends (CPU, GPU, Remote).
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │                    Executor Trait                       │
//! ├─────────────────────────────────────────────────────────┤
//! │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │
//! │  │ CpuExecutor  │  │ GpuExecutor  │  │RemoteExecutor│  │
//! │  │  (threads)   │  │   (wgpu)     │  │   (TCP)      │  │
//! │  └──────────────┘  └──────────────┘  └──────────────┘  │
//! └─────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Example
//!
//! ```rust,ignore
//! use pepita::executor::{CpuExecutor, Executor};
//! use pepita::task::Task;
//!
//! let executor = CpuExecutor::new(4);
//! let task = Task::binary("./worker").build();
//! let result = executor.execute(task).await?;
//! ```

use std::io;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::error::{KernelError, Result};
use crate::scheduler::TaskId;
use crate::task::{Backend, BinaryTask, ExecutionResult, Task, TaskKind, TaskState};

// ============================================================================
// EXECUTOR TRAIT
// ============================================================================

/// Trait for task executors.
///
/// Executors are responsible for actually running tasks on their
/// respective backends (CPU, GPU, or remote workers).
pub trait Executor: Send + Sync {
    /// Get the backend type.
    fn backend(&self) -> Backend;

    /// Check if executor can handle a task.
    fn can_execute(&self, task: &Task) -> bool;

    /// Execute a task synchronously.
    fn execute_sync(&self, task: &Task) -> Result<ExecutionResult>;

    /// Get the number of available workers.
    fn num_workers(&self) -> usize;

    /// Check if executor is healthy.
    fn is_healthy(&self) -> bool;

    /// Shutdown the executor.
    fn shutdown(&self);
}

// ============================================================================
// CPU EXECUTOR
// ============================================================================

/// CPU executor for running binary tasks locally.
///
/// Uses a thread pool for concurrent execution with configurable
/// worker count and resource limits.
#[derive(Debug)]
pub struct CpuExecutor {
    /// Number of worker threads
    num_workers: usize,
    /// Maximum memory per task (bytes)
    max_memory: Option<usize>,
    /// Maximum CPU time per task
    max_cpu_time: Option<Duration>,
    /// Running task count
    running_tasks: AtomicUsize,
    /// Whether executor is running
    running: AtomicBool,
}

impl CpuExecutor {
    /// Create a new CPU executor.
    ///
    /// # Arguments
    ///
    /// * `num_workers` - Number of concurrent workers
    #[must_use]
    pub fn new(num_workers: usize) -> Self {
        Self {
            num_workers: num_workers.max(1),
            max_memory: None,
            max_cpu_time: None,
            running_tasks: AtomicUsize::new(0),
            running: AtomicBool::new(true),
        }
    }

    /// Create executor with default worker count (CPU count).
    #[must_use]
    pub fn default_workers() -> Self {
        // Use available parallelism or default to 4
        let num_workers = std::thread::available_parallelism()
            .map(std::num::NonZero::get)
            .unwrap_or(4);
        Self::new(num_workers)
    }

    /// Set maximum memory per task.
    #[must_use]
    pub const fn with_max_memory(mut self, bytes: usize) -> Self {
        self.max_memory = Some(bytes);
        self
    }

    /// Set maximum CPU time per task.
    #[must_use]
    pub const fn with_max_cpu_time(mut self, duration: Duration) -> Self {
        self.max_cpu_time = Some(duration);
        self
    }

    /// Get the number of currently running tasks.
    #[must_use]
    pub fn running_tasks(&self) -> usize {
        self.running_tasks.load(Ordering::Acquire)
    }

    /// Execute a binary task.
    fn execute_binary(
        &self,
        task: &BinaryTask,
        timeout: Option<Duration>,
    ) -> Result<ExecutionResult> {
        let start = Instant::now();
        let task_id = TaskId::new(0); // Placeholder, real ID set by scheduler

        self.running_tasks.fetch_add(1, Ordering::Release);

        let result = self.run_binary_internal(task, timeout, task_id, start);

        self.running_tasks.fetch_sub(1, Ordering::Release);

        result
    }

    fn run_binary_internal(
        &self,
        task: &BinaryTask,
        timeout: Option<Duration>,
        task_id: TaskId,
        start: Instant,
    ) -> Result<ExecutionResult> {
        let mut cmd = Command::new(&task.path);
        cmd.args(&task.args);

        // Set environment variables
        for (key, value) in &task.env {
            cmd.env(key, value);
        }

        // Set working directory if specified
        if let Some(ref dir) = task.working_dir {
            cmd.current_dir(dir);
        }

        // Configure I/O
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        if task.stdin.is_some() {
            cmd.stdin(Stdio::piped());
        }

        // Spawn process
        let mut child = cmd.spawn().map_err(|e| {
            if e.kind() == io::ErrorKind::NotFound {
                KernelError::UblkDeviceNotFound
            } else {
                KernelError::InvalidRequest
            }
        })?;

        // Write stdin if provided
        if let Some(ref stdin_data) = task.stdin {
            if let Some(mut stdin) = child.stdin.take() {
                use std::io::Write;
                let _ = stdin.write_all(stdin_data);
            }
        }

        // Wait for completion with optional timeout
        let output = if let Some(timeout_duration) = timeout.or(self.max_cpu_time) {
            // Simple timeout implementation
            let deadline = start + timeout_duration;
            loop {
                match child.try_wait() {
                    Ok(Some(status)) => {
                        let output = child
                            .wait_with_output()
                            .map_err(|_| KernelError::IoTimeout)?;
                        let duration = start.elapsed();
                        return Ok(ExecutionResult {
                            task_id,
                            exit_code: status.code(),
                            stdout: output.stdout,
                            stderr: output.stderr,
                            output_buffers: Vec::new(),
                            duration,
                            state: if status.success() {
                                TaskState::Completed
                            } else {
                                TaskState::Failed
                            },
                            error: if status.success() {
                                None
                            } else {
                                Some(format!("Exit code: {:?}", status.code()))
                            },
                        });
                    }
                    Ok(None) => {
                        if Instant::now() > deadline {
                            let _ = child.kill();
                            return Ok(ExecutionResult {
                                task_id,
                                exit_code: None,
                                stdout: Vec::new(),
                                stderr: Vec::new(),
                                output_buffers: Vec::new(),
                                duration: start.elapsed(),
                                state: TaskState::TimedOut,
                                error: Some("Task timed out".to_string()),
                            });
                        }
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    Err(_) => {
                        return Err(KernelError::IoTimeout);
                    }
                }
            }
        } else {
            child
                .wait_with_output()
                .map_err(|_| KernelError::IoTimeout)?
        };

        let duration = start.elapsed();
        let exit_code = output.status.code();
        let success = output.status.success();

        Ok(ExecutionResult {
            task_id,
            exit_code,
            stdout: output.stdout,
            stderr: output.stderr,
            output_buffers: Vec::new(),
            duration,
            state: if success {
                TaskState::Completed
            } else {
                TaskState::Failed
            },
            error: if success {
                None
            } else {
                Some(format!("Exit code: {exit_code:?}"))
            },
        })
    }
}

impl Default for CpuExecutor {
    fn default() -> Self {
        Self::default_workers()
    }
}

impl Executor for CpuExecutor {
    fn backend(&self) -> Backend {
        Backend::Cpu
    }

    fn can_execute(&self, task: &Task) -> bool {
        // CPU executor handles binary and pipeline tasks
        matches!(task.kind, TaskKind::Binary(_) | TaskKind::Pipeline(_))
            && (task.backend == Backend::Cpu || task.backend == Backend::Any)
            && self.running.load(Ordering::Acquire)
    }

    fn execute_sync(&self, task: &Task) -> Result<ExecutionResult> {
        if !self.running.load(Ordering::Acquire) {
            return Err(KernelError::DeviceNotReady);
        }

        match &task.kind {
            TaskKind::Binary(binary_task) => self.execute_binary(binary_task, task.timeout),
            TaskKind::Pipeline(pipeline_task) => {
                // Execute pipeline stages sequentially
                let start = Instant::now();
                let mut last_output: Option<Vec<u8>> = None;

                for (idx, stage) in pipeline_task.stages.iter().enumerate() {
                    let mut stage_task = stage.clone();

                    // Pipe previous output to stdin
                    if pipeline_task.pipe_output && idx > 0 {
                        stage_task.stdin = last_output.take();
                    }

                    let result = self.execute_binary(&stage_task, task.timeout)?;

                    if !result.is_success() {
                        return Ok(ExecutionResult {
                            task_id: TaskId::new(0),
                            exit_code: result.exit_code,
                            stdout: result.stdout,
                            stderr: result.stderr,
                            output_buffers: Vec::new(),
                            duration: start.elapsed(),
                            state: TaskState::Failed,
                            error: Some(format!("Pipeline stage {idx} failed")),
                        });
                    }

                    last_output = Some(result.stdout);
                }

                Ok(ExecutionResult {
                    task_id: TaskId::new(0),
                    exit_code: Some(0),
                    stdout: last_output.unwrap_or_default(),
                    stderr: Vec::new(),
                    output_buffers: Vec::new(),
                    duration: start.elapsed(),
                    state: TaskState::Completed,
                    error: None,
                })
            }
            TaskKind::Shader(_) => Err(KernelError::NotSupported),
        }
    }

    fn num_workers(&self) -> usize {
        self.num_workers
    }

    fn is_healthy(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }

    fn shutdown(&self) {
        self.running.store(false, Ordering::Release);
    }
}

// ============================================================================
// GPU EXECUTOR (STUB)
// ============================================================================

/// GPU executor for running compute shaders.
///
/// Uses wgpu for cross-platform GPU compute (Vulkan, Metal, DX12, WebGPU).
/// This is a stub implementation - full GPU support requires wgpu dependency.
#[derive(Debug, Default)]
pub struct GpuExecutor {
    /// Number of GPU devices
    num_devices: usize,
    /// Whether executor is running
    running: AtomicBool,
}

impl GpuExecutor {
    /// Create a new GPU executor.
    #[must_use]
    pub fn new() -> Self {
        Self {
            num_devices: 0, // Would enumerate via wgpu
            running: AtomicBool::new(true),
        }
    }

    /// Check if GPU is available.
    #[must_use]
    pub fn is_available() -> bool {
        // Would check wgpu adapter availability
        false
    }
}

impl Executor for GpuExecutor {
    fn backend(&self) -> Backend {
        Backend::Gpu
    }

    fn can_execute(&self, task: &Task) -> bool {
        matches!(task.kind, TaskKind::Shader(_))
            && (task.backend == Backend::Gpu || task.backend == Backend::Any)
            && self.running.load(Ordering::Acquire)
            && self.num_devices > 0
    }

    fn execute_sync(&self, _task: &Task) -> Result<ExecutionResult> {
        // GPU execution requires wgpu - return not supported for now
        Err(KernelError::NotSupported)
    }

    fn num_workers(&self) -> usize {
        self.num_devices
    }

    fn is_healthy(&self) -> bool {
        self.running.load(Ordering::Acquire) && self.num_devices > 0
    }

    fn shutdown(&self) {
        self.running.store(false, Ordering::Release);
    }
}

// ============================================================================
// REMOTE EXECUTOR (STUB)
// ============================================================================

/// Remote executor for running tasks on remote workers.
///
/// Uses TCP transport with bincode serialization for efficient
/// task distribution. This is a stub implementation.
#[derive(Debug, Default)]
pub struct RemoteExecutor {
    /// Remote worker addresses
    workers: Vec<String>,
    /// Whether executor is connected
    connected: AtomicBool,
}

impl RemoteExecutor {
    /// Create a new remote executor.
    #[must_use]
    pub fn new() -> Self {
        Self {
            workers: Vec::new(),
            connected: AtomicBool::new(false),
        }
    }

    /// Add a remote worker.
    pub fn add_worker(&mut self, address: impl Into<String>) {
        self.workers.push(address.into());
    }

    /// Connect to remote workers.
    pub fn connect(&self) -> Result<()> {
        // Would establish TCP connections
        self.connected.store(true, Ordering::Release);
        Ok(())
    }
}

impl Executor for RemoteExecutor {
    fn backend(&self) -> Backend {
        Backend::Remote
    }

    fn can_execute(&self, task: &Task) -> bool {
        matches!(task.kind, TaskKind::Binary(_))
            && (task.backend == Backend::Remote || task.backend == Backend::Any)
            && self.connected.load(Ordering::Acquire)
            && !self.workers.is_empty()
    }

    fn execute_sync(&self, _task: &Task) -> Result<ExecutionResult> {
        // Remote execution requires network transport
        Err(KernelError::NotSupported)
    }

    fn num_workers(&self) -> usize {
        self.workers.len()
    }

    fn is_healthy(&self) -> bool {
        self.connected.load(Ordering::Acquire) && !self.workers.is_empty()
    }

    fn shutdown(&self) {
        self.connected.store(false, Ordering::Release);
    }
}

// ============================================================================
// EXECUTOR REGISTRY
// ============================================================================

/// Registry of available executors.
///
/// Manages multiple executors and routes tasks to the appropriate
/// backend based on task requirements.
#[derive(Default)]
pub struct ExecutorRegistry {
    /// Registered executors
    executors: Vec<Arc<dyn Executor>>,
}

impl ExecutorRegistry {
    /// Create a new empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            executors: Vec::new(),
        }
    }

    /// Register an executor.
    pub fn register(&mut self, executor: Arc<dyn Executor>) {
        self.executors.push(executor);
    }

    /// Find an executor for a task.
    #[must_use]
    pub fn find_executor(&self, task: &Task) -> Option<Arc<dyn Executor>> {
        self.executors.iter().find(|e| e.can_execute(task)).cloned()
    }

    /// Execute a task on the appropriate backend.
    pub fn execute(&self, task: &Task) -> Result<ExecutionResult> {
        let executor = self.find_executor(task).ok_or(KernelError::NotSupported)?;
        executor.execute_sync(task)
    }

    /// Get total number of workers across all executors.
    #[must_use]
    pub fn total_workers(&self) -> usize {
        self.executors.iter().map(|e| e.num_workers()).sum()
    }

    /// Shutdown all executors.
    pub fn shutdown(&self) {
        for executor in &self.executors {
            executor.shutdown();
        }
    }
}

impl std::fmt::Debug for ExecutorRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExecutorRegistry")
            .field("num_executors", &self.executors.len())
            .finish()
    }
}

// ============================================================================
// TESTS (EXTREME TDD)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------------------
    // CpuExecutor Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_cpu_executor_new() {
        let executor = CpuExecutor::new(4);
        assert_eq!(executor.num_workers(), 4);
        assert!(executor.is_healthy());
    }

    #[test]
    fn test_cpu_executor_default() {
        let executor = CpuExecutor::default();
        assert!(executor.num_workers() >= 1);
    }

    #[test]
    fn test_cpu_executor_min_workers() {
        let executor = CpuExecutor::new(0);
        assert_eq!(executor.num_workers(), 1); // Minimum is 1
    }

    #[test]
    fn test_cpu_executor_with_limits() {
        let executor = CpuExecutor::new(4)
            .with_max_memory(1024 * 1024 * 100)
            .with_max_cpu_time(Duration::from_secs(60));

        assert_eq!(executor.max_memory, Some(1024 * 1024 * 100));
        assert_eq!(executor.max_cpu_time, Some(Duration::from_secs(60)));
    }

    #[test]
    fn test_cpu_executor_backend() {
        let executor = CpuExecutor::new(4);
        assert_eq!(executor.backend(), Backend::Cpu);
    }

    #[test]
    fn test_cpu_executor_can_execute_binary() {
        let executor = CpuExecutor::new(4);
        let task = Task::binary("./worker").backend(Backend::Cpu).build();
        assert!(executor.can_execute(&task));
    }

    #[test]
    fn test_cpu_executor_cannot_execute_shader() {
        let executor = CpuExecutor::new(4);
        let task = Task::shader(vec![]).backend(Backend::Gpu).build();
        assert!(!executor.can_execute(&task));
    }

    #[test]
    fn test_cpu_executor_shutdown() {
        let executor = CpuExecutor::new(4);
        assert!(executor.is_healthy());

        executor.shutdown();
        assert!(!executor.is_healthy());

        // Can't execute after shutdown
        let task = Task::binary("./worker").build();
        assert!(!executor.can_execute(&task));
    }

    #[test]
    fn test_cpu_executor_running_tasks() {
        let executor = CpuExecutor::new(4);
        assert_eq!(executor.running_tasks(), 0);
    }

    // ------------------------------------------------------------------------
    // Execute Real Binary Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_cpu_executor_execute_echo() {
        let executor = CpuExecutor::new(4);
        let task = Task::binary("echo")
            .args(vec!["Hello, World!"])
            .backend(Backend::Cpu)
            .build();

        let result = executor.execute_sync(&task).unwrap();
        assert!(result.is_success());
        assert!(result.stdout_string().contains("Hello"));
    }

    #[test]
    fn test_cpu_executor_execute_not_found() {
        let executor = CpuExecutor::new(4);
        let task = Task::binary("/nonexistent/binary")
            .backend(Backend::Cpu)
            .build();

        let result = executor.execute_sync(&task);
        assert!(result.is_err());
    }

    #[test]
    fn test_cpu_executor_execute_with_args() {
        let executor = CpuExecutor::new(4);
        let task = Task::binary("printf")
            .args(vec!["%s %s", "foo", "bar"])
            .backend(Backend::Cpu)
            .build();

        let result = executor.execute_sync(&task).unwrap();
        assert!(result.is_success());
        assert_eq!(result.stdout_string().trim(), "foo bar");
    }

    #[test]
    fn test_cpu_executor_execute_with_env() {
        let executor = CpuExecutor::new(4);
        let mut env = std::collections::HashMap::new();
        env.insert("TEST_VAR".to_string(), "test_value".to_string());

        let task = Task::binary("sh")
            .args(vec!["-c", "echo $TEST_VAR"])
            .env(env)
            .backend(Backend::Cpu)
            .build();

        let result = executor.execute_sync(&task).unwrap();
        assert!(result.is_success());
        assert!(result.stdout_string().contains("test_value"));
    }

    #[test]
    fn test_cpu_executor_execute_false() {
        let executor = CpuExecutor::new(4);
        let task = Task::binary("false").backend(Backend::Cpu).build();

        let result = executor.execute_sync(&task).unwrap();
        assert!(result.is_failure());
        assert_ne!(result.exit_code, Some(0));
    }

    #[test]
    fn test_cpu_executor_execute_timeout() {
        let executor = CpuExecutor::new(4);
        let task = Task::binary("sleep")
            .args(vec!["10"])
            .timeout(Duration::from_millis(100))
            .backend(Backend::Cpu)
            .build();

        let result = executor.execute_sync(&task).unwrap();
        assert_eq!(result.state, TaskState::TimedOut);
    }

    // ------------------------------------------------------------------------
    // GpuExecutor Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_gpu_executor_new() {
        let executor = GpuExecutor::new();
        assert_eq!(executor.backend(), Backend::Gpu);
        assert_eq!(executor.num_workers(), 0);
    }

    #[test]
    fn test_gpu_executor_not_available() {
        assert!(!GpuExecutor::is_available());
    }

    #[test]
    fn test_gpu_executor_cannot_execute() {
        let executor = GpuExecutor::new();
        let task = Task::shader(vec![]).build();
        // No devices, so can't execute
        assert!(!executor.can_execute(&task));
    }

    // ------------------------------------------------------------------------
    // RemoteExecutor Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_remote_executor_new() {
        let executor = RemoteExecutor::new();
        assert_eq!(executor.backend(), Backend::Remote);
        assert_eq!(executor.num_workers(), 0);
    }

    #[test]
    fn test_remote_executor_add_worker() {
        let mut executor = RemoteExecutor::new();
        executor.add_worker("localhost:9000");
        executor.add_worker("localhost:9001");
        assert_eq!(executor.num_workers(), 2);
    }

    #[test]
    fn test_remote_executor_not_connected() {
        let executor = RemoteExecutor::new();
        let task = Task::binary("./worker").backend(Backend::Remote).build();
        assert!(!executor.can_execute(&task));
    }

    // ------------------------------------------------------------------------
    // ExecutorRegistry Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_registry_new() {
        let registry = ExecutorRegistry::new();
        assert_eq!(registry.total_workers(), 0);
    }

    #[test]
    fn test_registry_register() {
        let mut registry = ExecutorRegistry::new();
        registry.register(Arc::new(CpuExecutor::new(4)));
        assert_eq!(registry.total_workers(), 4);
    }

    #[test]
    fn test_registry_find_executor() {
        let mut registry = ExecutorRegistry::new();
        registry.register(Arc::new(CpuExecutor::new(4)));

        let task = Task::binary("./worker").backend(Backend::Cpu).build();
        let executor = registry.find_executor(&task);
        assert!(executor.is_some());
        assert_eq!(executor.unwrap().backend(), Backend::Cpu);
    }

    #[test]
    fn test_registry_execute() {
        let mut registry = ExecutorRegistry::new();
        registry.register(Arc::new(CpuExecutor::new(4)));

        let task = Task::binary("echo")
            .args(vec!["test"])
            .backend(Backend::Cpu)
            .build();

        let result = registry.execute(&task).unwrap();
        assert!(result.is_success());
    }

    #[test]
    fn test_registry_no_executor() {
        let registry = ExecutorRegistry::new();
        let task = Task::binary("./worker").backend(Backend::Cpu).build();

        let result = registry.execute(&task);
        assert!(result.is_err());
    }

    #[test]
    fn test_registry_shutdown() {
        let mut registry = ExecutorRegistry::new();
        let executor = Arc::new(CpuExecutor::new(4));
        registry.register(executor.clone());

        assert!(executor.is_healthy());
        registry.shutdown();
        assert!(!executor.is_healthy());
    }

    #[test]
    fn test_registry_multiple_backends() {
        let mut registry = ExecutorRegistry::new();
        registry.register(Arc::new(CpuExecutor::new(4)));
        registry.register(Arc::new(GpuExecutor::new()));
        registry.register(Arc::new(RemoteExecutor::new()));

        // Total workers = 4 CPU + 0 GPU + 0 Remote
        assert_eq!(registry.total_workers(), 4);
    }
}
