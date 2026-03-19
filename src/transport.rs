//! Message transport for distributed task execution.
//!
//! This module provides message types and transport abstractions
//! inspired by `ZeroMQ` patterns (REQ/REP, PUSH/PULL, PUB/SUB).
//!
//! ## Protocol
//!
//! Messages are serialized using a simple binary protocol:
//! - 4-byte length prefix (big-endian)
//! - Message type byte
//! - Payload
//!
//! ## Example
//!
//! ```rust
//! use pepita::transport::{Message, MessageType};
//!
//! // Create a heartbeat message
//! let msg = Message::heartbeat(42);
//! let bytes = msg.to_bytes();
//!
//! // Parse message back
//! let parsed = Message::from_bytes(&bytes).unwrap();
//! assert_eq!(parsed.message_type(), MessageType::Heartbeat);
//! ```

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::error::{KernelError, Result};
use crate::scheduler::TaskId;
use crate::task::{Backend, TaskState};

// ============================================================================
// MESSAGE TYPE
// ============================================================================

/// Type of message in the transport protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MessageType {
    /// Task submission request
    TaskSubmit = 0,
    /// Task execution result
    TaskResult = 1,
    /// Worker heartbeat
    Heartbeat = 2,
    /// Cancel task request
    TaskCancel = 3,
    /// Acknowledge message
    Ack = 4,
    /// Error response
    Error = 5,
    /// Shutdown signal
    Shutdown = 6,
    /// Worker registration
    Register = 7,
    /// Worker deregistration
    Deregister = 8,
    /// Status query
    Status = 9,
}

impl MessageType {
    /// Convert from byte.
    #[must_use]
    pub const fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::TaskSubmit),
            1 => Some(Self::TaskResult),
            2 => Some(Self::Heartbeat),
            3 => Some(Self::TaskCancel),
            4 => Some(Self::Ack),
            5 => Some(Self::Error),
            6 => Some(Self::Shutdown),
            7 => Some(Self::Register),
            8 => Some(Self::Deregister),
            9 => Some(Self::Status),
            _ => None,
        }
    }

    /// Convert to byte.
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

// ============================================================================
// MESSAGE PAYLOAD
// ============================================================================

/// Heartbeat payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeartbeatPayload {
    /// Worker ID
    pub worker_id: u32,
    /// Timestamp (unix millis)
    pub timestamp: u64,
    /// Number of pending tasks
    pub pending_tasks: u32,
    /// CPU load (0-100)
    pub cpu_load: u8,
    /// Memory usage (0-100)
    pub memory_usage: u8,
}

impl HeartbeatPayload {
    /// Create a new heartbeat payload.
    #[must_use]
    pub fn new(worker_id: u32) -> Self {
        let timestamp =
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64;

        Self { worker_id, timestamp, pending_tasks: 0, cpu_load: 0, memory_usage: 0 }
    }

    /// Set pending tasks count.
    #[must_use]
    pub const fn with_pending_tasks(mut self, count: u32) -> Self {
        self.pending_tasks = count;
        self
    }

    /// Set CPU load.
    #[must_use]
    pub fn with_cpu_load(mut self, load: u8) -> Self {
        self.cpu_load = if load > 100 { 100 } else { load };
        self
    }

    /// Set memory usage.
    #[must_use]
    pub fn with_memory_usage(mut self, usage: u8) -> Self {
        self.memory_usage = if usage > 100 { 100 } else { usage };
        self
    }

    /// Serialize to bytes.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(18);
        bytes.extend_from_slice(&self.worker_id.to_be_bytes());
        bytes.extend_from_slice(&self.timestamp.to_be_bytes());
        bytes.extend_from_slice(&self.pending_tasks.to_be_bytes());
        bytes.push(self.cpu_load);
        bytes.push(self.memory_usage);
        bytes
    }

    /// Deserialize from bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < 18 {
            return Err(KernelError::InvalidRequest);
        }

        let worker_id = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let timestamp = u64::from_be_bytes([
            bytes[4], bytes[5], bytes[6], bytes[7], bytes[8], bytes[9], bytes[10], bytes[11],
        ]);
        let pending_tasks = u32::from_be_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);
        let cpu_load = bytes[16];
        let memory_usage = bytes[17];

        Ok(Self { worker_id, timestamp, pending_tasks, cpu_load, memory_usage })
    }
}

/// Task result payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskResultPayload {
    /// Task ID
    pub task_id: u64,
    /// Final state
    pub state: TaskState,
    /// Exit code (if applicable)
    pub exit_code: Option<i32>,
    /// Execution duration (millis)
    pub duration_ms: u64,
    /// Error message (if any)
    pub error: Option<String>,
}

impl TaskResultPayload {
    /// Create a new result payload.
    #[must_use]
    pub fn new(task_id: TaskId, state: TaskState, duration: Duration) -> Self {
        Self {
            task_id: task_id.as_u64(),
            state,
            exit_code: None,
            duration_ms: duration.as_millis() as u64,
            error: None,
        }
    }

    /// Set exit code.
    #[must_use]
    pub const fn with_exit_code(mut self, code: i32) -> Self {
        self.exit_code = Some(code);
        self
    }

    /// Set error message.
    #[must_use]
    pub fn with_error(mut self, error: impl Into<String>) -> Self {
        self.error = Some(error.into());
        self
    }

    /// Serialize to bytes.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();

        // Task ID (8 bytes)
        bytes.extend_from_slice(&self.task_id.to_be_bytes());

        // State (1 byte)
        bytes.push(match self.state {
            TaskState::Pending => 0,
            TaskState::Queued => 1,
            TaskState::Running => 2,
            TaskState::Completed => 3,
            TaskState::Failed => 4,
            TaskState::Cancelled => 5,
            TaskState::TimedOut => 6,
        });

        // Exit code (5 bytes: 1 present flag + 4 value)
        if let Some(code) = self.exit_code {
            bytes.push(1);
            bytes.extend_from_slice(&code.to_be_bytes());
        } else {
            bytes.push(0);
            bytes.extend_from_slice(&[0u8; 4]);
        }

        // Duration (8 bytes)
        bytes.extend_from_slice(&self.duration_ms.to_be_bytes());

        // Error (variable: 4-byte length + string)
        match &self.error {
            Some(err) => {
                let err_bytes = err.as_bytes();
                bytes.extend_from_slice(&(err_bytes.len() as u32).to_be_bytes());
                bytes.extend_from_slice(err_bytes);
            }
            None => {
                bytes.extend_from_slice(&0u32.to_be_bytes());
            }
        }

        bytes
    }

    /// Deserialize from bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < 22 {
            return Err(KernelError::InvalidRequest);
        }

        let task_id = u64::from_be_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]);

        let state = match bytes[8] {
            0 => TaskState::Pending,
            1 => TaskState::Queued,
            2 => TaskState::Running,
            3 => TaskState::Completed,
            4 => TaskState::Failed,
            5 => TaskState::Cancelled,
            6 => TaskState::TimedOut,
            _ => return Err(KernelError::InvalidRequest),
        };

        let exit_code = if bytes[9] == 1 {
            Some(i32::from_be_bytes([bytes[10], bytes[11], bytes[12], bytes[13]]))
        } else {
            None
        };

        let duration_ms = u64::from_be_bytes([
            bytes[14], bytes[15], bytes[16], bytes[17], bytes[18], bytes[19], bytes[20], bytes[21],
        ]);

        let error = if bytes.len() > 22 {
            let err_len = u32::from_be_bytes([bytes[22], bytes[23], bytes[24], bytes[25]]) as usize;
            if err_len > 0 && bytes.len() >= 26 + err_len {
                Some(
                    String::from_utf8(bytes[26..26 + err_len].to_vec())
                        .map_err(|_| KernelError::InvalidRequest)?,
                )
            } else {
                None
            }
        } else {
            None
        };

        Ok(Self { task_id, state, exit_code, duration_ms, error })
    }
}

/// Worker registration payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterPayload {
    /// Worker ID
    pub worker_id: u32,
    /// Backend type
    pub backend: Backend,
    /// Number of workers/cores
    pub num_workers: u16,
    /// Worker name/hostname
    pub name: String,
}

impl RegisterPayload {
    /// Create a new registration payload.
    #[must_use]
    pub fn new(worker_id: u32, backend: Backend, num_workers: u16) -> Self {
        Self { worker_id, backend, num_workers, name: String::new() }
    }

    /// Set worker name.
    #[must_use]
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Serialize to bytes.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();

        // Worker ID (4 bytes)
        bytes.extend_from_slice(&self.worker_id.to_be_bytes());

        // Backend (1 byte)
        bytes.push(match self.backend {
            Backend::Cpu => 0,
            Backend::Gpu => 1,
            Backend::Remote => 2,
            Backend::Any => 3,
        });

        // Num workers (2 bytes)
        bytes.extend_from_slice(&self.num_workers.to_be_bytes());

        // Name (variable: 2-byte length + string)
        let name_bytes = self.name.as_bytes();
        bytes.extend_from_slice(&(name_bytes.len() as u16).to_be_bytes());
        bytes.extend_from_slice(name_bytes);

        bytes
    }

    /// Deserialize from bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < 9 {
            return Err(KernelError::InvalidRequest);
        }

        let worker_id = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);

        let backend = match bytes[4] {
            0 => Backend::Cpu,
            1 => Backend::Gpu,
            2 => Backend::Remote,
            3 => Backend::Any,
            _ => return Err(KernelError::InvalidRequest),
        };

        let num_workers = u16::from_be_bytes([bytes[5], bytes[6]]);

        let name_len = u16::from_be_bytes([bytes[7], bytes[8]]) as usize;
        let name = if name_len > 0 && bytes.len() >= 9 + name_len {
            String::from_utf8(bytes[9..9 + name_len].to_vec())
                .map_err(|_| KernelError::InvalidRequest)?
        } else {
            String::new()
        };

        Ok(Self { worker_id, backend, num_workers, name })
    }
}

// ============================================================================
// MESSAGE
// ============================================================================

/// A transport message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    /// Message type
    msg_type: MessageType,
    /// Raw payload
    payload: Vec<u8>,
}

impl Message {
    /// Create a new message.
    #[must_use]
    pub fn new(msg_type: MessageType, payload: Vec<u8>) -> Self {
        Self { msg_type, payload }
    }

    /// Create a heartbeat message.
    #[must_use]
    pub fn heartbeat(worker_id: u32) -> Self {
        let payload = HeartbeatPayload::new(worker_id);
        Self::new(MessageType::Heartbeat, payload.to_bytes())
    }

    /// Create a heartbeat message with full payload.
    #[must_use]
    pub fn heartbeat_full(payload: HeartbeatPayload) -> Self {
        Self::new(MessageType::Heartbeat, payload.to_bytes())
    }

    /// Create a task result message.
    #[must_use]
    pub fn task_result(payload: TaskResultPayload) -> Self {
        Self::new(MessageType::TaskResult, payload.to_bytes())
    }

    /// Create a register message.
    #[must_use]
    pub fn register(payload: RegisterPayload) -> Self {
        Self::new(MessageType::Register, payload.to_bytes())
    }

    /// Create an ack message.
    #[must_use]
    pub fn ack() -> Self {
        Self::new(MessageType::Ack, Vec::new())
    }

    /// Create an error message.
    #[must_use]
    pub fn error(message: impl Into<String>) -> Self {
        Self::new(MessageType::Error, message.into().into_bytes())
    }

    /// Create a shutdown message.
    #[must_use]
    pub fn shutdown() -> Self {
        Self::new(MessageType::Shutdown, Vec::new())
    }

    /// Create a task cancel message.
    #[must_use]
    pub fn task_cancel(task_id: TaskId) -> Self {
        Self::new(MessageType::TaskCancel, task_id.as_u64().to_be_bytes().to_vec())
    }

    /// Get the message type.
    #[must_use]
    pub const fn message_type(&self) -> MessageType {
        self.msg_type
    }

    /// Get the payload.
    #[must_use]
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    /// Get heartbeat payload (if applicable).
    pub fn as_heartbeat(&self) -> Result<HeartbeatPayload> {
        if self.msg_type != MessageType::Heartbeat {
            return Err(KernelError::InvalidRequest);
        }
        HeartbeatPayload::from_bytes(&self.payload)
    }

    /// Get task result payload (if applicable).
    pub fn as_task_result(&self) -> Result<TaskResultPayload> {
        if self.msg_type != MessageType::TaskResult {
            return Err(KernelError::InvalidRequest);
        }
        TaskResultPayload::from_bytes(&self.payload)
    }

    /// Get register payload (if applicable).
    pub fn as_register(&self) -> Result<RegisterPayload> {
        if self.msg_type != MessageType::Register {
            return Err(KernelError::InvalidRequest);
        }
        RegisterPayload::from_bytes(&self.payload)
    }

    /// Get error message (if applicable).
    #[must_use]
    pub fn as_error(&self) -> Option<String> {
        if self.msg_type != MessageType::Error {
            return None;
        }
        String::from_utf8(self.payload.clone()).ok()
    }

    /// Serialize message to bytes.
    ///
    /// Format: [length: 4 bytes] [type: 1 byte] [payload: N bytes]
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let payload_len = self.payload.len() as u32;
        let total_len = 1 + payload_len; // type + payload

        let mut bytes = Vec::with_capacity(4 + total_len as usize);
        bytes.extend_from_slice(&total_len.to_be_bytes());
        bytes.push(self.msg_type.as_u8());
        bytes.extend_from_slice(&self.payload);

        bytes
    }

    /// Deserialize message from bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < 5 {
            return Err(KernelError::InvalidRequest);
        }

        let total_len = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;

        if bytes.len() < 4 + total_len {
            return Err(KernelError::InvalidRequest);
        }

        let msg_type = MessageType::from_u8(bytes[4]).ok_or(KernelError::InvalidRequest)?;

        let payload = bytes[5..4 + total_len].to_vec();

        Ok(Self { msg_type, payload })
    }

    /// Get the total size in bytes.
    #[must_use]
    pub fn size(&self) -> usize {
        4 + 1 + self.payload.len()
    }
}

// ============================================================================
// TESTS (EXTREME TDD)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------------------
    // MessageType Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_message_type_roundtrip() {
        let types = [
            MessageType::TaskSubmit,
            MessageType::TaskResult,
            MessageType::Heartbeat,
            MessageType::TaskCancel,
            MessageType::Ack,
            MessageType::Error,
            MessageType::Shutdown,
            MessageType::Register,
            MessageType::Deregister,
            MessageType::Status,
        ];

        for msg_type in types {
            let byte = msg_type.as_u8();
            let recovered = MessageType::from_u8(byte);
            assert_eq!(recovered, Some(msg_type));
        }
    }

    #[test]
    fn test_message_type_invalid() {
        assert_eq!(MessageType::from_u8(255), None);
    }

    // ------------------------------------------------------------------------
    // HeartbeatPayload Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_heartbeat_payload_new() {
        let payload = HeartbeatPayload::new(42);
        assert_eq!(payload.worker_id, 42);
        assert!(payload.timestamp > 0);
        assert_eq!(payload.pending_tasks, 0);
    }

    #[test]
    fn test_heartbeat_payload_builders() {
        let payload =
            HeartbeatPayload::new(1).with_pending_tasks(10).with_cpu_load(75).with_memory_usage(50);

        assert_eq!(payload.pending_tasks, 10);
        assert_eq!(payload.cpu_load, 75);
        assert_eq!(payload.memory_usage, 50);
    }

    #[test]
    fn test_heartbeat_payload_roundtrip() {
        let original = HeartbeatPayload::new(42)
            .with_pending_tasks(100)
            .with_cpu_load(80)
            .with_memory_usage(60);

        let bytes = original.to_bytes();
        let recovered = HeartbeatPayload::from_bytes(&bytes).unwrap();

        assert_eq!(recovered.worker_id, original.worker_id);
        assert_eq!(recovered.timestamp, original.timestamp);
        assert_eq!(recovered.pending_tasks, original.pending_tasks);
        assert_eq!(recovered.cpu_load, original.cpu_load);
        assert_eq!(recovered.memory_usage, original.memory_usage);
    }

    #[test]
    fn test_heartbeat_payload_invalid() {
        let result = HeartbeatPayload::from_bytes(&[0u8; 5]);
        assert!(result.is_err());
    }

    // ------------------------------------------------------------------------
    // TaskResultPayload Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_task_result_payload_new() {
        let payload =
            TaskResultPayload::new(TaskId::new(123), TaskState::Completed, Duration::from_secs(5));

        assert_eq!(payload.task_id, 123);
        assert_eq!(payload.state, TaskState::Completed);
        assert_eq!(payload.duration_ms, 5000);
        assert!(payload.exit_code.is_none());
        assert!(payload.error.is_none());
    }

    #[test]
    fn test_task_result_payload_with_exit_code() {
        let payload =
            TaskResultPayload::new(TaskId::new(1), TaskState::Failed, Duration::from_secs(1))
                .with_exit_code(1);

        assert_eq!(payload.exit_code, Some(1));
    }

    #[test]
    fn test_task_result_payload_with_error() {
        let payload =
            TaskResultPayload::new(TaskId::new(1), TaskState::Failed, Duration::from_secs(1))
                .with_error("Task failed");

        assert_eq!(payload.error, Some("Task failed".to_string()));
    }

    #[test]
    fn test_task_result_payload_roundtrip() {
        let original = TaskResultPayload::new(
            TaskId::new(12345),
            TaskState::Failed,
            Duration::from_millis(1234),
        )
        .with_exit_code(42)
        .with_error("Test error");

        let bytes = original.to_bytes();
        let recovered = TaskResultPayload::from_bytes(&bytes).unwrap();

        assert_eq!(recovered.task_id, original.task_id);
        assert_eq!(recovered.state, original.state);
        assert_eq!(recovered.exit_code, original.exit_code);
        assert_eq!(recovered.duration_ms, original.duration_ms);
        assert_eq!(recovered.error, original.error);
    }

    // ------------------------------------------------------------------------
    // RegisterPayload Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_register_payload_new() {
        let payload = RegisterPayload::new(1, Backend::Cpu, 4);
        assert_eq!(payload.worker_id, 1);
        assert_eq!(payload.backend, Backend::Cpu);
        assert_eq!(payload.num_workers, 4);
    }

    #[test]
    fn test_register_payload_with_name() {
        let payload = RegisterPayload::new(1, Backend::Cpu, 4).with_name("worker-1");
        assert_eq!(payload.name, "worker-1");
    }

    #[test]
    fn test_register_payload_roundtrip() {
        let original = RegisterPayload::new(42, Backend::Gpu, 8).with_name("gpu-worker");

        let bytes = original.to_bytes();
        let recovered = RegisterPayload::from_bytes(&bytes).unwrap();

        assert_eq!(recovered.worker_id, original.worker_id);
        assert_eq!(recovered.backend, original.backend);
        assert_eq!(recovered.num_workers, original.num_workers);
        assert_eq!(recovered.name, original.name);
    }

    // ------------------------------------------------------------------------
    // Message Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_message_heartbeat() {
        let msg = Message::heartbeat(42);
        assert_eq!(msg.message_type(), MessageType::Heartbeat);

        let payload = msg.as_heartbeat().unwrap();
        assert_eq!(payload.worker_id, 42);
    }

    #[test]
    fn test_message_ack() {
        let msg = Message::ack();
        assert_eq!(msg.message_type(), MessageType::Ack);
        assert!(msg.payload().is_empty());
    }

    #[test]
    fn test_message_error() {
        let msg = Message::error("Something went wrong");
        assert_eq!(msg.message_type(), MessageType::Error);
        assert_eq!(msg.as_error(), Some("Something went wrong".to_string()));
    }

    #[test]
    fn test_message_shutdown() {
        let msg = Message::shutdown();
        assert_eq!(msg.message_type(), MessageType::Shutdown);
    }

    #[test]
    fn test_message_task_cancel() {
        let msg = Message::task_cancel(TaskId::new(12345));
        assert_eq!(msg.message_type(), MessageType::TaskCancel);

        let payload = msg.payload();
        let task_id = u64::from_be_bytes([
            payload[0], payload[1], payload[2], payload[3], payload[4], payload[5], payload[6],
            payload[7],
        ]);
        assert_eq!(task_id, 12345);
    }

    #[test]
    fn test_message_roundtrip() {
        let original = Message::heartbeat(42);
        let bytes = original.to_bytes();
        let recovered = Message::from_bytes(&bytes).unwrap();

        assert_eq!(recovered.message_type(), original.message_type());
        assert_eq!(recovered.payload(), original.payload());
    }

    #[test]
    fn test_message_size() {
        let msg = Message::ack();
        assert_eq!(msg.size(), 5); // 4 (length) + 1 (type) + 0 (payload)

        let msg_with_payload = Message::error("test");
        assert_eq!(msg_with_payload.size(), 9); // 4 + 1 + 4 (payload)
    }

    #[test]
    fn test_message_from_bytes_invalid() {
        // Too short
        let result = Message::from_bytes(&[0u8; 3]);
        assert!(result.is_err());

        // Invalid type
        let mut bytes = 1u32.to_be_bytes().to_vec();
        bytes.push(255); // Invalid type
        let result = Message::from_bytes(&bytes);
        assert!(result.is_err());
    }

    #[test]
    fn test_message_register() {
        let payload = RegisterPayload::new(1, Backend::Cpu, 8).with_name("test-worker");
        let msg = Message::register(payload.clone());

        assert_eq!(msg.message_type(), MessageType::Register);

        let recovered = msg.as_register().unwrap();
        assert_eq!(recovered.worker_id, payload.worker_id);
        assert_eq!(recovered.backend, payload.backend);
        assert_eq!(recovered.name, payload.name);
    }

    #[test]
    fn test_message_task_result() {
        let payload =
            TaskResultPayload::new(TaskId::new(100), TaskState::Completed, Duration::from_secs(10));
        let msg = Message::task_result(payload.clone());

        assert_eq!(msg.message_type(), MessageType::TaskResult);

        let recovered = msg.as_task_result().unwrap();
        assert_eq!(recovered.task_id, payload.task_id);
        assert_eq!(recovered.state, payload.state);
    }

    // ------------------------------------------------------------------------
    // Wrong Type Access Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_message_wrong_type_heartbeat() {
        let msg = Message::ack();
        let result = msg.as_heartbeat();
        assert!(result.is_err());
    }

    #[test]
    fn test_message_wrong_type_result() {
        let msg = Message::ack();
        let result = msg.as_task_result();
        assert!(result.is_err());
    }

    #[test]
    fn test_message_wrong_type_register() {
        let msg = Message::ack();
        let result = msg.as_register();
        assert!(result.is_err());
    }

    #[test]
    fn test_message_wrong_type_error() {
        let msg = Message::ack();
        let result = msg.as_error();
        assert!(result.is_none());
    }
}
