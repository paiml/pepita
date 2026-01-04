//! GPU: Compute Shader Execution
//!
//! Pure Rust GPU compute abstraction using wgpu API design.
//! Provides a unified interface for GPU compute operations without
//! external dependencies (mock implementation for sovereignty).
//!
//! ## Example
//!
//! ```rust,ignore
//! use pepita::gpu::{GpuDevice, ComputeShader, Buffer};
//!
//! let device = GpuDevice::default_device()?;
//! let shader = ComputeShader::from_wgsl(include_str!("add.wgsl"))?;
//!
//! let input = device.create_buffer(&data, BufferUsage::STORAGE)?;
//! let output = device.create_buffer_uninit(size, BufferUsage::STORAGE)?;
//!
//! device.dispatch(&shader, &[input, output], (64, 1, 1))?;
//! ```

use crate::error::{KernelError, Result};

#[cfg(feature = "std")]
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
#[cfg(feature = "std")]
use std::sync::Arc;

// ============================================================================
// GPU BACKEND TYPE
// ============================================================================

/// GPU backend type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum GpuBackend {
    /// No GPU available
    #[default]
    None = 0,
    /// Vulkan backend
    Vulkan = 1,
    /// Metal backend (macOS/iOS)
    Metal = 2,
    /// DirectX 12 backend (Windows)
    Dx12 = 3,
    /// OpenGL backend (fallback)
    OpenGL = 4,
    /// WebGPU backend (browser)
    WebGpu = 5,
}

impl GpuBackend {
    /// Get backend name
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Vulkan => "vulkan",
            Self::Metal => "metal",
            Self::Dx12 => "dx12",
            Self::OpenGL => "opengl",
            Self::WebGpu => "webgpu",
        }
    }

    /// Check if this is a native backend
    #[must_use]
    pub const fn is_native(&self) -> bool {
        matches!(self, Self::Vulkan | Self::Metal | Self::Dx12)
    }

    /// Check if available
    #[must_use]
    pub const fn is_available(&self) -> bool {
        !matches!(self, Self::None)
    }
}

// ============================================================================
// BUFFER USAGE FLAGS
// ============================================================================

/// Buffer usage flags (bitfield)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BufferUsage(u32);

impl BufferUsage {
    /// Map read access
    pub const MAP_READ: Self = Self(1 << 0);
    /// Map write access
    pub const MAP_WRITE: Self = Self(1 << 1);
    /// Copy source
    pub const COPY_SRC: Self = Self(1 << 2);
    /// Copy destination
    pub const COPY_DST: Self = Self(1 << 3);
    /// Index buffer
    pub const INDEX: Self = Self(1 << 4);
    /// Vertex buffer
    pub const VERTEX: Self = Self(1 << 5);
    /// Uniform buffer
    pub const UNIFORM: Self = Self(1 << 6);
    /// Storage buffer
    pub const STORAGE: Self = Self(1 << 7);
    /// Indirect buffer
    pub const INDIRECT: Self = Self(1 << 8);

    /// Create empty usage
    #[must_use]
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Combine usages
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Check if usage contains another
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Get raw value
    #[must_use]
    pub const fn bits(self) -> u32 {
        self.0
    }
}

// ============================================================================
// GPU LIMITS
// ============================================================================

/// GPU device limits
#[derive(Debug, Clone, Copy)]
pub struct GpuLimits {
    /// Maximum buffer size
    pub max_buffer_size: u64,
    /// Maximum storage buffer binding size
    pub max_storage_buffer_binding_size: u32,
    /// Maximum uniform buffer binding size
    pub max_uniform_buffer_binding_size: u32,
    /// Maximum compute workgroup size X
    pub max_compute_workgroup_size_x: u32,
    /// Maximum compute workgroup size Y
    pub max_compute_workgroup_size_y: u32,
    /// Maximum compute workgroup size Z
    pub max_compute_workgroup_size_z: u32,
    /// Maximum compute workgroups per dimension
    pub max_compute_workgroups_per_dimension: u32,
    /// Maximum bind groups
    pub max_bind_groups: u32,
}

impl Default for GpuLimits {
    fn default() -> Self {
        Self {
            max_buffer_size: 256 * 1024 * 1024, // 256 MB
            max_storage_buffer_binding_size: 128 * 1024 * 1024,
            max_uniform_buffer_binding_size: 64 * 1024,
            max_compute_workgroup_size_x: 256,
            max_compute_workgroup_size_y: 256,
            max_compute_workgroup_size_z: 64,
            max_compute_workgroups_per_dimension: 65535,
            max_bind_groups: 4,
        }
    }
}

// ============================================================================
// GPU DEVICE INFO
// ============================================================================

/// GPU device information
#[derive(Debug, Clone)]
pub struct GpuDeviceInfo {
    /// Device name
    pub name: String,
    /// Vendor name
    pub vendor: String,
    /// Backend type
    pub backend: GpuBackend,
    /// Device type
    pub device_type: GpuDeviceType,
    /// Device limits
    pub limits: GpuLimits,
}

impl Default for GpuDeviceInfo {
    fn default() -> Self {
        Self {
            name: "Mock GPU".to_string(),
            vendor: "Pepita".to_string(),
            backend: GpuBackend::None,
            device_type: GpuDeviceType::Other,
            limits: GpuLimits::default(),
        }
    }
}

/// GPU device type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum GpuDeviceType {
    /// Discrete GPU
    DiscreteGpu = 0,
    /// Integrated GPU
    IntegratedGpu = 1,
    /// Virtual GPU
    VirtualGpu = 2,
    /// CPU (software rendering)
    Cpu = 3,
    /// Other/unknown
    #[default]
    Other = 4,
}

// ============================================================================
// BUFFER (std only)
// ============================================================================

/// GPU buffer
#[cfg(feature = "std")]
#[derive(Debug)]
pub struct Buffer {
    /// Buffer ID
    id: u64,
    /// Buffer size
    size: u64,
    /// Usage flags
    usage: BufferUsage,
    /// Mapped data (mock: CPU memory)
    data: Vec<u8>,
}

#[cfg(feature = "std")]
impl Buffer {
    /// Create a new buffer
    pub fn new(id: u64, size: u64, usage: BufferUsage) -> Self {
        Self {
            id,
            size,
            usage,
            data: vec![0u8; size as usize],
        }
    }

    /// Create with initial data
    pub fn with_data(id: u64, data: &[u8], usage: BufferUsage) -> Self {
        Self {
            id,
            size: data.len() as u64,
            usage,
            data: data.to_vec(),
        }
    }

    /// Get buffer ID
    #[must_use]
    pub const fn id(&self) -> u64 {
        self.id
    }

    /// Get buffer size
    #[must_use]
    pub const fn size(&self) -> u64 {
        self.size
    }

    /// Get usage flags
    #[must_use]
    pub const fn usage(&self) -> BufferUsage {
        self.usage
    }

    /// Get data (for CPU-side access)
    #[must_use]
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Get mutable data
    pub fn data_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }

    /// Map buffer for reading
    pub fn map_read(&self) -> Result<&[u8]> {
        if !self.usage.contains(BufferUsage::MAP_READ) {
            return Err(KernelError::InvalidRequest);
        }
        Ok(&self.data)
    }

    /// Map buffer for writing
    pub fn map_write(&mut self) -> Result<&mut [u8]> {
        if !self.usage.contains(BufferUsage::MAP_WRITE) {
            return Err(KernelError::InvalidRequest);
        }
        Ok(&mut self.data)
    }
}

// ============================================================================
// COMPUTE SHADER (std only)
// ============================================================================

/// Shader stage
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ShaderStage {
    /// Vertex shader
    Vertex = 0,
    /// Fragment shader
    Fragment = 1,
    /// Compute shader
    Compute = 2,
}

/// Compute shader
#[cfg(feature = "std")]
#[derive(Debug)]
pub struct ComputeShader {
    /// Shader ID
    id: u64,
    /// Shader source (WGSL)
    source: String,
    /// Entry point name
    entry_point: String,
    /// Workgroup size
    workgroup_size: (u32, u32, u32),
}

#[cfg(feature = "std")]
impl ComputeShader {
    /// Create from WGSL source
    pub fn from_wgsl(source: impl Into<String>) -> Result<Self> {
        static SHADER_ID: AtomicU64 = AtomicU64::new(1);

        Ok(Self {
            id: SHADER_ID.fetch_add(1, Ordering::Relaxed),
            source: source.into(),
            entry_point: "main".to_string(),
            workgroup_size: (64, 1, 1),
        })
    }

    /// Set entry point
    #[must_use]
    pub fn with_entry_point(mut self, name: impl Into<String>) -> Self {
        self.entry_point = name.into();
        self
    }

    /// Set workgroup size
    #[must_use]
    pub const fn with_workgroup_size(mut self, x: u32, y: u32, z: u32) -> Self {
        self.workgroup_size = (x, y, z);
        self
    }

    /// Get shader ID
    #[must_use]
    pub const fn id(&self) -> u64 {
        self.id
    }

    /// Get source
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Get entry point
    #[must_use]
    pub fn entry_point(&self) -> &str {
        &self.entry_point
    }

    /// Get workgroup size
    #[must_use]
    pub const fn workgroup_size(&self) -> (u32, u32, u32) {
        self.workgroup_size
    }
}

// ============================================================================
// GPU DEVICE (std only)
// ============================================================================

/// GPU device
#[cfg(feature = "std")]
pub struct GpuDevice {
    /// Device info
    info: GpuDeviceInfo,
    /// Is available
    available: AtomicBool,
    /// Buffer counter
    buffer_counter: AtomicU64,
    /// Dispatch counter
    dispatch_count: AtomicU64,
}

#[cfg(feature = "std")]
impl GpuDevice {
    /// Create a mock device (for testing without real GPU)
    pub fn mock() -> Self {
        Self {
            info: GpuDeviceInfo::default(),
            available: AtomicBool::new(true),
            buffer_counter: AtomicU64::new(1),
            dispatch_count: AtomicU64::new(0),
        }
    }

    /// Try to get the default device
    pub fn default_device() -> Result<Self> {
        // Mock: return a mock device
        // Real implementation would enumerate adapters
        Ok(Self::mock())
    }

    /// Get device info
    #[must_use]
    pub const fn info(&self) -> &GpuDeviceInfo {
        &self.info
    }

    /// Check if device is available
    #[must_use]
    pub fn is_available(&self) -> bool {
        self.available.load(Ordering::Acquire)
    }

    /// Create a buffer with data
    pub fn create_buffer(&self, data: &[u8], usage: BufferUsage) -> Result<Buffer> {
        if !self.is_available() {
            return Err(KernelError::DeviceNotReady);
        }
        if data.len() as u64 > self.info.limits.max_buffer_size {
            return Err(KernelError::OutOfMemory);
        }

        let id = self.buffer_counter.fetch_add(1, Ordering::Relaxed);
        Ok(Buffer::with_data(id, data, usage))
    }

    /// Create an uninitialized buffer
    pub fn create_buffer_uninit(&self, size: u64, usage: BufferUsage) -> Result<Buffer> {
        if !self.is_available() {
            return Err(KernelError::DeviceNotReady);
        }
        if size > self.info.limits.max_buffer_size {
            return Err(KernelError::OutOfMemory);
        }

        let id = self.buffer_counter.fetch_add(1, Ordering::Relaxed);
        Ok(Buffer::new(id, size, usage))
    }

    /// Dispatch a compute shader (mock - performs CPU computation)
    pub fn dispatch(
        &self,
        _shader: &ComputeShader,
        _buffers: &[&Buffer],
        workgroups: (u32, u32, u32),
    ) -> Result<()> {
        if !self.is_available() {
            return Err(KernelError::DeviceNotReady);
        }

        // Validate workgroup count
        let limits = &self.info.limits;
        if workgroups.0 > limits.max_compute_workgroups_per_dimension
            || workgroups.1 > limits.max_compute_workgroups_per_dimension
            || workgroups.2 > limits.max_compute_workgroups_per_dimension
        {
            return Err(KernelError::InvalidArgument);
        }

        // Mock: just count dispatches
        self.dispatch_count.fetch_add(1, Ordering::Relaxed);

        Ok(())
    }

    /// Get dispatch count
    #[must_use]
    pub fn dispatch_count(&self) -> u64 {
        self.dispatch_count.load(Ordering::Relaxed)
    }

    /// Copy buffer to buffer
    pub fn copy_buffer(&self, src: &Buffer, dst: &mut Buffer) -> Result<()> {
        if !self.is_available() {
            return Err(KernelError::DeviceNotReady);
        }
        if !src.usage.contains(BufferUsage::COPY_SRC) {
            return Err(KernelError::InvalidRequest);
        }
        if !dst.usage.contains(BufferUsage::COPY_DST) {
            return Err(KernelError::InvalidRequest);
        }

        let len = src.size.min(dst.size) as usize;
        dst.data[..len].copy_from_slice(&src.data[..len]);

        Ok(())
    }

    /// Submit and wait (mock - immediate)
    pub fn submit_and_wait(&self) -> Result<()> {
        if !self.is_available() {
            return Err(KernelError::DeviceNotReady);
        }
        Ok(())
    }
}

#[cfg(feature = "std")]
impl Default for GpuDevice {
    fn default() -> Self {
        Self::mock()
    }
}

#[cfg(feature = "std")]
impl std::fmt::Debug for GpuDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GpuDevice")
            .field("name", &self.info.name)
            .field("backend", &self.info.backend)
            .field("available", &self.is_available())
            .finish()
    }
}

// ============================================================================
// COMPUTE PIPELINE (std only)
// ============================================================================

/// Compute pipeline configuration
#[cfg(feature = "std")]
pub struct ComputePipeline {
    /// Shader
    shader: Arc<ComputeShader>,
    /// Bind group layouts (number of bind groups)
    bind_groups: u32,
}

#[cfg(feature = "std")]
impl ComputePipeline {
    /// Create a new compute pipeline
    pub fn new(shader: ComputeShader) -> Self {
        Self {
            shader: Arc::new(shader),
            bind_groups: 1,
        }
    }

    /// Set number of bind groups
    #[must_use]
    pub const fn with_bind_groups(mut self, count: u32) -> Self {
        self.bind_groups = count;
        self
    }

    /// Get shader
    #[must_use]
    pub fn shader(&self) -> &ComputeShader {
        &self.shader
    }
}

#[cfg(feature = "std")]
impl std::fmt::Debug for ComputePipeline {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ComputePipeline")
            .field("shader_id", &self.shader.id())
            .field("bind_groups", &self.bind_groups)
            .finish()
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------------------
    // GpuBackend Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_gpu_backend_default() {
        assert_eq!(GpuBackend::default(), GpuBackend::None);
    }

    #[test]
    fn test_gpu_backend_name() {
        assert_eq!(GpuBackend::Vulkan.name(), "vulkan");
        assert_eq!(GpuBackend::Metal.name(), "metal");
        assert_eq!(GpuBackend::None.name(), "none");
    }

    #[test]
    fn test_gpu_backend_is_native() {
        assert!(GpuBackend::Vulkan.is_native());
        assert!(GpuBackend::Metal.is_native());
        assert!(GpuBackend::Dx12.is_native());
        assert!(!GpuBackend::OpenGL.is_native());
        assert!(!GpuBackend::WebGpu.is_native());
    }

    #[test]
    fn test_gpu_backend_is_available() {
        assert!(GpuBackend::Vulkan.is_available());
        assert!(!GpuBackend::None.is_available());
    }

    // ------------------------------------------------------------------------
    // BufferUsage Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_buffer_usage_empty() {
        let usage = BufferUsage::empty();
        assert_eq!(usage.bits(), 0);
    }

    #[test]
    fn test_buffer_usage_union() {
        let usage = BufferUsage::STORAGE.union(BufferUsage::COPY_SRC);
        assert!(usage.contains(BufferUsage::STORAGE));
        assert!(usage.contains(BufferUsage::COPY_SRC));
        assert!(!usage.contains(BufferUsage::UNIFORM));
    }

    #[test]
    fn test_buffer_usage_contains() {
        let usage = BufferUsage::STORAGE;
        assert!(usage.contains(BufferUsage::STORAGE));
        assert!(!usage.contains(BufferUsage::UNIFORM));
    }

    // ------------------------------------------------------------------------
    // GpuLimits Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_gpu_limits_default() {
        let limits = GpuLimits::default();
        assert!(limits.max_buffer_size > 0);
        assert!(limits.max_compute_workgroup_size_x > 0);
    }

    // ------------------------------------------------------------------------
    // GpuDeviceInfo Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_gpu_device_info_default() {
        let info = GpuDeviceInfo::default();
        assert!(!info.name.is_empty());
        assert_eq!(info.backend, GpuBackend::None);
    }

    // ------------------------------------------------------------------------
    // Buffer Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_buffer_new() {
        let buffer = Buffer::new(1, 1024, BufferUsage::STORAGE);
        assert_eq!(buffer.id(), 1);
        assert_eq!(buffer.size(), 1024);
        assert_eq!(buffer.data().len(), 1024);
    }

    #[test]
    fn test_buffer_with_data() {
        let data = vec![1u8, 2, 3, 4];
        let buffer = Buffer::with_data(1, &data, BufferUsage::STORAGE);
        assert_eq!(buffer.size(), 4);
        assert_eq!(buffer.data(), &data);
    }

    #[test]
    fn test_buffer_map_read() {
        let buffer = Buffer::new(1, 64, BufferUsage::MAP_READ);
        let data = buffer.map_read().unwrap();
        assert_eq!(data.len(), 64);
    }

    #[test]
    fn test_buffer_map_read_invalid() {
        let buffer = Buffer::new(1, 64, BufferUsage::STORAGE);
        assert!(buffer.map_read().is_err());
    }

    #[test]
    fn test_buffer_map_write() {
        let mut buffer = Buffer::new(1, 64, BufferUsage::MAP_WRITE);
        let data = buffer.map_write().unwrap();
        data[0] = 42;
        assert_eq!(buffer.data()[0], 42);
    }

    // ------------------------------------------------------------------------
    // ComputeShader Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_compute_shader_from_wgsl() {
        let shader = ComputeShader::from_wgsl("@compute fn main() {}").unwrap();
        assert!(!shader.source().is_empty());
        assert_eq!(shader.entry_point(), "main");
    }

    #[test]
    fn test_compute_shader_with_entry_point() {
        let shader = ComputeShader::from_wgsl("")
            .unwrap()
            .with_entry_point("compute_main");
        assert_eq!(shader.entry_point(), "compute_main");
    }

    #[test]
    fn test_compute_shader_with_workgroup_size() {
        let shader = ComputeShader::from_wgsl("")
            .unwrap()
            .with_workgroup_size(128, 1, 1);
        assert_eq!(shader.workgroup_size(), (128, 1, 1));
    }

    // ------------------------------------------------------------------------
    // GpuDevice Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_gpu_device_mock() {
        let device = GpuDevice::mock();
        assert!(device.is_available());
    }

    #[test]
    fn test_gpu_device_default() {
        let device = GpuDevice::default_device().unwrap();
        assert!(device.is_available());
    }

    #[test]
    fn test_gpu_device_create_buffer() {
        let device = GpuDevice::mock();
        let data = vec![1u8, 2, 3, 4];
        let buffer = device.create_buffer(&data, BufferUsage::STORAGE).unwrap();
        assert_eq!(buffer.size(), 4);
    }

    #[test]
    fn test_gpu_device_create_buffer_uninit() {
        let device = GpuDevice::mock();
        let buffer = device.create_buffer_uninit(1024, BufferUsage::STORAGE).unwrap();
        assert_eq!(buffer.size(), 1024);
    }

    #[test]
    fn test_gpu_device_dispatch() {
        let device = GpuDevice::mock();
        let shader = ComputeShader::from_wgsl("").unwrap();

        device.dispatch(&shader, &[], (64, 1, 1)).unwrap();
        assert_eq!(device.dispatch_count(), 1);
    }

    #[test]
    fn test_gpu_device_dispatch_invalid_workgroups() {
        let device = GpuDevice::mock();
        let shader = ComputeShader::from_wgsl("").unwrap();

        let result = device.dispatch(&shader, &[], (100000, 1, 1));
        assert!(result.is_err());
    }

    #[test]
    fn test_gpu_device_copy_buffer() {
        let device = GpuDevice::mock();

        let src_data = vec![1u8, 2, 3, 4];
        let src = device
            .create_buffer(&src_data, BufferUsage::COPY_SRC)
            .unwrap();
        let mut dst = device
            .create_buffer_uninit(4, BufferUsage::COPY_DST)
            .unwrap();

        device.copy_buffer(&src, &mut dst).unwrap();
        assert_eq!(dst.data(), &src_data);
    }

    #[test]
    fn test_gpu_device_debug() {
        let device = GpuDevice::mock();
        let debug = format!("{:?}", device);
        assert!(debug.contains("GpuDevice"));
    }

    // ------------------------------------------------------------------------
    // ComputePipeline Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_compute_pipeline_new() {
        let shader = ComputeShader::from_wgsl("").unwrap();
        let pipeline = ComputePipeline::new(shader);
        assert!(pipeline.shader().id() > 0);
    }

    #[test]
    fn test_compute_pipeline_with_bind_groups() {
        let shader = ComputeShader::from_wgsl("").unwrap();
        let pipeline = ComputePipeline::new(shader).with_bind_groups(2);
        let debug = format!("{:?}", pipeline);
        assert!(debug.contains("bind_groups: 2"));
    }
}
