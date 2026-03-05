//! Task types for distributed execution.
//!
//! This module provides the core task abstractions for the distributed
//! computing framework. Tasks represent units of work that can be
//! executed on CPU, GPU, or remote backends.
//!
//! ## Task Types
//!
//! - [`BinaryTask`]: Execute a compiled Rust binary
//! - [`ShaderTask`]: Execute a GPU compute shader (SPIR-V)
//! - [`PipelineTask`]: Chain multiple tasks with stdout → stdin
//!
//! ## Example
//!
//! ```rust
//! use pepita::task::{Task, TaskBuilder, TaskPriority};
//!
//! // Create a binary task
//! let task = Task::binary("./worker")
//!     .args(vec!["--input", "data.bin"])
//!     .priority(TaskPriority::High)
//!     .build();
//!
//! assert!(task.is_binary());
//! ```

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use crate::scheduler::TaskId;

// ============================================================================
// TASK PRIORITY
// ============================================================================

/// Task execution priority.
///
/// Higher priority tasks are scheduled before lower priority ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum TaskPriority {
    /// Lowest priority (background tasks)
    Low = 0,
    /// Normal priority (default)
    #[default]
    Normal = 1,
    /// High priority (user-interactive)
    High = 2,
    /// Critical priority (system tasks)
    Critical = 3,
}

impl TaskPriority {
    /// Convert to numeric value.
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    /// Create from numeric value.
    #[must_use]
    pub const fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Low),
            1 => Some(Self::Normal),
            2 => Some(Self::High),
            3 => Some(Self::Critical),
            _ => None,
        }
    }
}

// ============================================================================
// TASK AFFINITY
// ============================================================================

/// CPU affinity for task execution.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum CpuAffinity {
    /// No affinity - run on any CPU
    #[default]
    Any,
    /// Pin to specific core
    Core(usize),
    /// Pin to specific cores
    Cores(Vec<usize>),
    /// Pin to NUMA node
    NumaNode(i32),
}

impl CpuAffinity {
    /// Check if affinity allows running on a specific core.
    #[must_use]
    pub fn allows_core(&self, core: usize) -> bool {
        match self {
            Self::Any | Self::NumaNode(_) => true, // NumaNode simplified, would need topology info
            Self::Core(c) => *c == core,
            Self::Cores(cores) => cores.contains(&core),
        }
    }
}

// ============================================================================
// TASK STATE
// ============================================================================

/// Current state of a task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TaskState {
    /// Task is waiting to be scheduled
    #[default]
    Pending,
    /// Task is queued for execution
    Queued,
    /// Task is currently running
    Running,
    /// Task completed successfully
    Completed,
    /// Task failed with error
    Failed,
    /// Task was cancelled
    Cancelled,
    /// Task timed out
    TimedOut,
}

impl TaskState {
    /// Check if task is in a terminal state.
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Failed | Self::Cancelled | Self::TimedOut
        )
    }

    /// Check if task is still active.
    #[must_use]
    pub const fn is_active(self) -> bool {
        matches!(self, Self::Pending | Self::Queued | Self::Running)
    }
}

// ============================================================================
// BACKEND TYPE
// ============================================================================

/// Execution backend type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Backend {
    /// Execute on CPU (local)
    #[default]
    Cpu,
    /// Execute on GPU (via wgpu)
    Gpu,
    /// Execute on remote worker
    Remote,
    /// Any available backend
    Any,
}

impl Backend {
    /// Check if backend is local (CPU or GPU).
    #[must_use]
    pub const fn is_local(self) -> bool {
        matches!(self, Self::Cpu | Self::Gpu)
    }
}

// ============================================================================
// BINARY TASK
// ============================================================================

/// A task that executes a compiled binary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinaryTask {
    /// Path to the binary
    pub path: PathBuf,
    /// Command-line arguments
    pub args: Vec<String>,
    /// Environment variables
    pub env: HashMap<String, String>,
    /// Working directory
    pub working_dir: Option<PathBuf>,
    /// Stdin input (if any)
    pub stdin: Option<Vec<u8>>,
}

impl BinaryTask {
    /// Create a new binary task.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            args: Vec::new(),
            env: HashMap::new(),
            working_dir: None,
            stdin: None,
        }
    }

    /// Add command-line arguments.
    #[must_use]
    pub fn with_args(mut self, args: Vec<impl Into<String>>) -> Self {
        self.args = args.into_iter().map(Into::into).collect();
        self
    }

    /// Add environment variables.
    #[must_use]
    pub fn with_env(mut self, env: HashMap<String, String>) -> Self {
        self.env = env;
        self
    }

    /// Set working directory.
    #[must_use]
    pub fn with_working_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.working_dir = Some(dir.into());
        self
    }

    /// Set stdin input.
    #[must_use]
    pub fn with_stdin(mut self, stdin: Vec<u8>) -> Self {
        self.stdin = Some(stdin);
        self
    }
}

// ============================================================================
// SHADER TASK
// ============================================================================

/// A task that executes a GPU compute shader.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShaderTask {
    /// SPIR-V shader binary
    pub shader_binary: Vec<u8>,
    /// Input buffer sizes
    pub input_sizes: Vec<usize>,
    /// Output buffer sizes
    pub output_sizes: Vec<usize>,
    /// Workgroup dimensions (x, y, z)
    pub workgroups: (u32, u32, u32),
    /// Push constants (if any)
    pub push_constants: Option<Vec<u8>>,
}

impl ShaderTask {
    /// Create a new shader task.
    #[must_use]
    pub fn new(shader_binary: Vec<u8>) -> Self {
        Self {
            shader_binary,
            input_sizes: Vec::new(),
            output_sizes: Vec::new(),
            workgroups: (1, 1, 1),
            push_constants: None,
        }
    }

    /// Set input buffer sizes.
    #[must_use]
    pub fn with_inputs(mut self, sizes: Vec<usize>) -> Self {
        self.input_sizes = sizes;
        self
    }

    /// Set output buffer sizes.
    #[must_use]
    pub fn with_outputs(mut self, sizes: Vec<usize>) -> Self {
        self.output_sizes = sizes;
        self
    }

    /// Set workgroup dimensions.
    #[must_use]
    pub const fn with_workgroups(mut self, x: u32, y: u32, z: u32) -> Self {
        self.workgroups = (x, y, z);
        self
    }

    /// Set push constants.
    #[must_use]
    pub fn with_push_constants(mut self, constants: Vec<u8>) -> Self {
        self.push_constants = Some(constants);
        self
    }

    /// Calculate total workgroup count.
    #[must_use]
    pub const fn total_workgroups(&self) -> u64 {
        self.workgroups.0 as u64 * self.workgroups.1 as u64 * self.workgroups.2 as u64
    }
}

// ============================================================================
// PIPELINE TASK
// ============================================================================

/// A task that chains multiple binary tasks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipelineTask {
    /// Pipeline stages (executed sequentially)
    pub stages: Vec<BinaryTask>,
    /// Whether to pipe stdout → stdin between stages
    pub pipe_output: bool,
}

impl PipelineTask {
    /// Create a new pipeline task.
    #[must_use]
    pub fn new() -> Self {
        Self {
            stages: Vec::new(),
            pipe_output: true,
        }
    }

    /// Add a stage to the pipeline.
    #[must_use]
    pub fn add_stage(mut self, task: BinaryTask) -> Self {
        self.stages.push(task);
        self
    }

    /// Set whether to pipe output between stages.
    #[must_use]
    pub const fn with_pipe_output(mut self, pipe: bool) -> Self {
        self.pipe_output = pipe;
        self
    }

    /// Get the number of stages.
    #[must_use]
    pub fn num_stages(&self) -> usize {
        self.stages.len()
    }

    /// Check if pipeline is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.stages.is_empty()
    }
}

impl Default for PipelineTask {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// TASK KIND
// ============================================================================

/// The specific type of task to execute.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskKind {
    /// Execute a binary
    Binary(BinaryTask),
    /// Execute a GPU shader
    Shader(ShaderTask),
    /// Execute a pipeline of tasks
    Pipeline(PipelineTask),
}

// ============================================================================
// TASK
// ============================================================================

/// A unit of work to be executed by the scheduler.
#[derive(Debug, Clone)]
pub struct Task {
    /// Unique task identifier (assigned by scheduler)
    pub id: Option<TaskId>,
    /// The specific task type
    pub kind: TaskKind,
    /// Execution backend
    pub backend: Backend,
    /// Priority level
    pub priority: TaskPriority,
    /// CPU affinity
    pub affinity: CpuAffinity,
    /// Current state
    pub state: TaskState,
    /// Timeout duration
    pub timeout: Option<Duration>,
    /// Maximum retry attempts
    pub max_retries: u32,
    /// Current retry count
    pub retry_count: u32,
    /// User-defined metadata
    pub metadata: HashMap<String, String>,
}

impl Task {
    /// Create a binary task builder.
    #[must_use]
    pub fn binary(path: impl Into<PathBuf>) -> TaskBuilder {
        TaskBuilder::new(TaskKind::Binary(BinaryTask::new(path)))
    }

    /// Create a shader task builder.
    #[must_use]
    pub fn shader(shader_binary: Vec<u8>) -> TaskBuilder {
        TaskBuilder::new(TaskKind::Shader(ShaderTask::new(shader_binary)))
    }

    /// Create a pipeline task builder.
    #[must_use]
    pub fn pipeline() -> TaskBuilder {
        TaskBuilder::new(TaskKind::Pipeline(PipelineTask::new()))
    }

    /// Check if this is a binary task.
    #[must_use]
    pub const fn is_binary(&self) -> bool {
        matches!(self.kind, TaskKind::Binary(_))
    }

    /// Check if this is a shader task.
    #[must_use]
    pub const fn is_shader(&self) -> bool {
        matches!(self.kind, TaskKind::Shader(_))
    }

    /// Check if this is a pipeline task.
    #[must_use]
    pub const fn is_pipeline(&self) -> bool {
        matches!(self.kind, TaskKind::Pipeline(_))
    }

    /// Get the binary task (if applicable).
    #[must_use]
    pub const fn as_binary(&self) -> Option<&BinaryTask> {
        match &self.kind {
            TaskKind::Binary(t) => Some(t),
            _ => None,
        }
    }

    /// Get the shader task (if applicable).
    #[must_use]
    pub const fn as_shader(&self) -> Option<&ShaderTask> {
        match &self.kind {
            TaskKind::Shader(t) => Some(t),
            _ => None,
        }
    }

    /// Get the pipeline task (if applicable).
    #[must_use]
    pub const fn as_pipeline(&self) -> Option<&PipelineTask> {
        match &self.kind {
            TaskKind::Pipeline(t) => Some(t),
            _ => None,
        }
    }

    /// Check if task can be retried.
    #[must_use]
    pub const fn can_retry(&self) -> bool {
        self.retry_count < self.max_retries
    }

    /// Increment retry count.
    pub fn increment_retry(&mut self) {
        self.retry_count += 1;
    }
}

// ============================================================================
// TASK BUILDER
// ============================================================================

/// Builder for creating tasks with a fluent API.
#[derive(Debug)]
pub struct TaskBuilder {
    kind: TaskKind,
    backend: Backend,
    priority: TaskPriority,
    affinity: CpuAffinity,
    timeout: Option<Duration>,
    max_retries: u32,
    metadata: HashMap<String, String>,
}

impl TaskBuilder {
    /// Create a new task builder.
    fn new(kind: TaskKind) -> Self {
        Self {
            kind,
            backend: Backend::default(),
            priority: TaskPriority::default(),
            affinity: CpuAffinity::default(),
            timeout: None,
            max_retries: 3,
            metadata: HashMap::new(),
        }
    }

    /// Set command-line arguments (for binary tasks).
    #[must_use]
    pub fn args(mut self, args: Vec<impl Into<String>>) -> Self {
        if let TaskKind::Binary(ref mut task) = self.kind {
            task.args = args.into_iter().map(Into::into).collect();
        }
        self
    }

    /// Set environment variables (for binary tasks).
    #[must_use]
    pub fn env(mut self, env: HashMap<String, String>) -> Self {
        if let TaskKind::Binary(ref mut task) = self.kind {
            task.env = env;
        }
        self
    }

    /// Add a pipeline stage (for pipeline tasks).
    #[must_use]
    pub fn add_stage(mut self, stage: BinaryTask) -> Self {
        if let TaskKind::Pipeline(ref mut task) = self.kind {
            task.stages.push(stage);
        }
        self
    }

    /// Set the execution backend.
    #[must_use]
    pub const fn backend(mut self, backend: Backend) -> Self {
        self.backend = backend;
        self
    }

    /// Set the priority level.
    #[must_use]
    pub const fn priority(mut self, priority: TaskPriority) -> Self {
        self.priority = priority;
        self
    }

    /// Set CPU affinity.
    #[must_use]
    pub fn affinity(mut self, affinity: CpuAffinity) -> Self {
        self.affinity = affinity;
        self
    }

    /// Set timeout duration.
    #[must_use]
    pub const fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Set maximum retry attempts.
    #[must_use]
    pub const fn max_retries(mut self, retries: u32) -> Self {
        self.max_retries = retries;
        self
    }

    /// Add metadata.
    #[must_use]
    pub fn metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Build the task.
    #[must_use]
    pub fn build(self) -> Task {
        Task {
            id: None,
            kind: self.kind,
            backend: self.backend,
            priority: self.priority,
            affinity: self.affinity,
            state: TaskState::Pending,
            timeout: self.timeout,
            max_retries: self.max_retries,
            retry_count: 0,
            metadata: self.metadata,
        }
    }
}

// ============================================================================
// EXECUTION RESULT
// ============================================================================

/// Result of task execution.
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    /// Task ID
    pub task_id: TaskId,
    /// Exit code (for binary tasks)
    pub exit_code: Option<i32>,
    /// Standard output
    pub stdout: Vec<u8>,
    /// Standard error
    pub stderr: Vec<u8>,
    /// Output buffers (for shader tasks)
    pub output_buffers: Vec<Vec<u8>>,
    /// Execution duration
    pub duration: Duration,
    /// Final state
    pub state: TaskState,
    /// Error message (if failed)
    pub error: Option<String>,
}

impl ExecutionResult {
    /// Create a successful result.
    #[must_use]
    pub fn success(task_id: TaskId, duration: Duration) -> Self {
        Self {
            task_id,
            exit_code: Some(0),
            stdout: Vec::new(),
            stderr: Vec::new(),
            output_buffers: Vec::new(),
            duration,
            state: TaskState::Completed,
            error: None,
        }
    }

    /// Create a failed result.
    #[must_use]
    pub fn failure(task_id: TaskId, error: impl Into<String>, duration: Duration) -> Self {
        Self {
            task_id,
            exit_code: None,
            stdout: Vec::new(),
            stderr: Vec::new(),
            output_buffers: Vec::new(),
            duration,
            state: TaskState::Failed,
            error: Some(error.into()),
        }
    }

    /// Check if execution succeeded.
    #[must_use]
    pub const fn is_success(&self) -> bool {
        matches!(self.state, TaskState::Completed)
    }

    /// Check if execution failed.
    #[must_use]
    pub const fn is_failure(&self) -> bool {
        matches!(self.state, TaskState::Failed)
    }

    /// Get stdout as string.
    #[must_use]
    pub fn stdout_string(&self) -> String {
        String::from_utf8_lossy(&self.stdout).into_owned()
    }

    /// Get stderr as string.
    #[must_use]
    pub fn stderr_string(&self) -> String {
        String::from_utf8_lossy(&self.stderr).into_owned()
    }
}

// ============================================================================
// TESTS (EXTREME TDD)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------------------
    // TaskPriority Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_priority_ordering() {
        assert!(TaskPriority::Low < TaskPriority::Normal);
        assert!(TaskPriority::Normal < TaskPriority::High);
        assert!(TaskPriority::High < TaskPriority::Critical);
    }

    #[test]
    fn test_priority_default() {
        assert_eq!(TaskPriority::default(), TaskPriority::Normal);
    }

    #[test]
    fn test_priority_as_u8() {
        assert_eq!(TaskPriority::Low.as_u8(), 0);
        assert_eq!(TaskPriority::Normal.as_u8(), 1);
        assert_eq!(TaskPriority::High.as_u8(), 2);
        assert_eq!(TaskPriority::Critical.as_u8(), 3);
    }

    #[test]
    fn test_priority_from_u8() {
        assert_eq!(TaskPriority::from_u8(0), Some(TaskPriority::Low));
        assert_eq!(TaskPriority::from_u8(1), Some(TaskPriority::Normal));
        assert_eq!(TaskPriority::from_u8(2), Some(TaskPriority::High));
        assert_eq!(TaskPriority::from_u8(3), Some(TaskPriority::Critical));
        assert_eq!(TaskPriority::from_u8(4), None);
    }

    // ------------------------------------------------------------------------
    // CpuAffinity Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_affinity_any() {
        let affinity = CpuAffinity::Any;
        assert!(affinity.allows_core(0));
        assert!(affinity.allows_core(100));
    }

    #[test]
    fn test_affinity_core() {
        let affinity = CpuAffinity::Core(2);
        assert!(!affinity.allows_core(0));
        assert!(affinity.allows_core(2));
        assert!(!affinity.allows_core(3));
    }

    #[test]
    fn test_affinity_cores() {
        let affinity = CpuAffinity::Cores(vec![0, 2, 4]);
        assert!(affinity.allows_core(0));
        assert!(!affinity.allows_core(1));
        assert!(affinity.allows_core(2));
        assert!(!affinity.allows_core(3));
        assert!(affinity.allows_core(4));
    }

    #[test]
    fn test_affinity_default() {
        assert_eq!(CpuAffinity::default(), CpuAffinity::Any);
    }

    // ------------------------------------------------------------------------
    // TaskState Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_state_is_terminal() {
        assert!(!TaskState::Pending.is_terminal());
        assert!(!TaskState::Queued.is_terminal());
        assert!(!TaskState::Running.is_terminal());
        assert!(TaskState::Completed.is_terminal());
        assert!(TaskState::Failed.is_terminal());
        assert!(TaskState::Cancelled.is_terminal());
        assert!(TaskState::TimedOut.is_terminal());
    }

    #[test]
    fn test_state_is_active() {
        assert!(TaskState::Pending.is_active());
        assert!(TaskState::Queued.is_active());
        assert!(TaskState::Running.is_active());
        assert!(!TaskState::Completed.is_active());
        assert!(!TaskState::Failed.is_active());
    }

    #[test]
    fn test_state_default() {
        assert_eq!(TaskState::default(), TaskState::Pending);
    }

    // ------------------------------------------------------------------------
    // Backend Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_backend_is_local() {
        assert!(Backend::Cpu.is_local());
        assert!(Backend::Gpu.is_local());
        assert!(!Backend::Remote.is_local());
        assert!(!Backend::Any.is_local());
    }

    #[test]
    fn test_backend_default() {
        assert_eq!(Backend::default(), Backend::Cpu);
    }

    // ------------------------------------------------------------------------
    // BinaryTask Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_binary_task_new() {
        let task = BinaryTask::new("./worker");
        assert_eq!(task.path, PathBuf::from("./worker"));
        assert!(task.args.is_empty());
        assert!(task.env.is_empty());
    }

    #[test]
    fn test_binary_task_with_args() {
        let task = BinaryTask::new("./worker").with_args(vec!["--input", "data.bin"]);
        assert_eq!(task.args, vec!["--input", "data.bin"]);
    }

    #[test]
    fn test_binary_task_with_env() {
        let mut env = HashMap::new();
        env.insert("KEY".to_string(), "VALUE".to_string());
        let task = BinaryTask::new("./worker").with_env(env);
        assert_eq!(task.env.get("KEY"), Some(&"VALUE".to_string()));
    }

    #[test]
    fn test_binary_task_with_working_dir() {
        let task = BinaryTask::new("./worker").with_working_dir("/tmp");
        assert_eq!(task.working_dir, Some(PathBuf::from("/tmp")));
    }

    #[test]
    fn test_binary_task_with_stdin() {
        let task = BinaryTask::new("./worker").with_stdin(vec![1, 2, 3]);
        assert_eq!(task.stdin, Some(vec![1, 2, 3]));
    }

    // ------------------------------------------------------------------------
    // ShaderTask Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_shader_task_new() {
        let task = ShaderTask::new(vec![0x03, 0x02, 0x23, 0x07]);
        assert_eq!(task.shader_binary, vec![0x03, 0x02, 0x23, 0x07]);
        assert_eq!(task.workgroups, (1, 1, 1));
    }

    #[test]
    fn test_shader_task_with_workgroups() {
        let task = ShaderTask::new(vec![]).with_workgroups(64, 64, 1);
        assert_eq!(task.workgroups, (64, 64, 1));
    }

    #[test]
    fn test_shader_task_total_workgroups() {
        let task = ShaderTask::new(vec![]).with_workgroups(8, 8, 4);
        assert_eq!(task.total_workgroups(), 256);
    }

    #[test]
    fn test_shader_task_with_inputs_outputs() {
        let task = ShaderTask::new(vec![])
            .with_inputs(vec![1024, 2048])
            .with_outputs(vec![4096]);
        assert_eq!(task.input_sizes, vec![1024, 2048]);
        assert_eq!(task.output_sizes, vec![4096]);
    }

    // ------------------------------------------------------------------------
    // PipelineTask Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_pipeline_task_new() {
        let task = PipelineTask::new();
        assert!(task.is_empty());
        assert_eq!(task.num_stages(), 0);
        assert!(task.pipe_output);
    }

    #[test]
    fn test_pipeline_task_add_stage() {
        let task = PipelineTask::new()
            .add_stage(BinaryTask::new("./stage1"))
            .add_stage(BinaryTask::new("./stage2"));
        assert_eq!(task.num_stages(), 2);
        assert!(!task.is_empty());
    }

    #[test]
    fn test_pipeline_task_with_pipe_output() {
        let task = PipelineTask::new().with_pipe_output(false);
        assert!(!task.pipe_output);
    }

    // ------------------------------------------------------------------------
    // Task Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_task_binary_builder() {
        let task = Task::binary("./worker")
            .args(vec!["--input", "data.bin"])
            .backend(Backend::Cpu)
            .priority(TaskPriority::High)
            .build();

        assert!(task.is_binary());
        assert!(!task.is_shader());
        assert!(!task.is_pipeline());
        assert_eq!(task.backend, Backend::Cpu);
        assert_eq!(task.priority, TaskPriority::High);
    }

    #[test]
    fn test_task_shader_builder() {
        let task = Task::shader(vec![0x03, 0x02, 0x23, 0x07])
            .backend(Backend::Gpu)
            .build();

        assert!(task.is_shader());
        assert_eq!(task.backend, Backend::Gpu);
    }

    #[test]
    fn test_task_pipeline_builder() {
        let task = Task::pipeline()
            .add_stage(BinaryTask::new("./stage1"))
            .add_stage(BinaryTask::new("./stage2"))
            .build();

        assert!(task.is_pipeline());
        assert_eq!(task.as_pipeline().unwrap().num_stages(), 2);
    }

    #[test]
    fn test_task_as_binary() {
        let task = Task::binary("./worker").build();
        let binary = task.as_binary();
        assert!(binary.is_some());
        assert_eq!(binary.unwrap().path, PathBuf::from("./worker"));
    }

    #[test]
    fn test_task_retry() {
        let mut task = Task::binary("./worker").max_retries(3).build();

        assert!(task.can_retry());
        assert_eq!(task.retry_count, 0);

        task.increment_retry();
        assert!(task.can_retry());
        assert_eq!(task.retry_count, 1);

        task.increment_retry();
        task.increment_retry();
        assert!(!task.can_retry());
        assert_eq!(task.retry_count, 3);
    }

    #[test]
    fn test_task_timeout() {
        let task = Task::binary("./worker")
            .timeout(Duration::from_secs(30))
            .build();

        assert_eq!(task.timeout, Some(Duration::from_secs(30)));
    }

    #[test]
    fn test_task_metadata() {
        let task = Task::binary("./worker")
            .metadata("key1", "value1")
            .metadata("key2", "value2")
            .build();

        assert_eq!(task.metadata.get("key1"), Some(&"value1".to_string()));
        assert_eq!(task.metadata.get("key2"), Some(&"value2".to_string()));
    }

    // ------------------------------------------------------------------------
    // ExecutionResult Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_execution_result_success() {
        let result = ExecutionResult::success(TaskId(1), Duration::from_secs(5));
        assert!(result.is_success());
        assert!(!result.is_failure());
        assert_eq!(result.exit_code, Some(0));
        assert!(result.error.is_none());
    }

    #[test]
    fn test_execution_result_failure() {
        let result = ExecutionResult::failure(TaskId(1), "Task failed", Duration::from_secs(1));
        assert!(!result.is_success());
        assert!(result.is_failure());
        assert_eq!(result.error, Some("Task failed".to_string()));
    }

    #[test]
    fn test_execution_result_stdout_string() {
        let mut result = ExecutionResult::success(TaskId(1), Duration::from_secs(1));
        result.stdout = b"Hello, World!".to_vec();
        assert_eq!(result.stdout_string(), "Hello, World!");
    }

    #[test]
    fn test_execution_result_stderr_string() {
        let mut result = ExecutionResult::success(TaskId(1), Duration::from_secs(1));
        result.stderr = b"Error message".to_vec();
        assert_eq!(result.stderr_string(), "Error message");
    }
}
