//! VMM: Lightweight Virtual Machine Monitor
//!
//! Pure Rust KVM-based virtualization for microVMs.
//! Inspired by Firecracker but with zero external dependencies.
//!
//! ## Example
//!
//! ```rust,ignore
//! use pepita::vmm::{MicroVm, VmConfig};
//!
//! let config = VmConfig::builder()
//!     .vcpus(2)
//!     .memory_mb(256)
//!     .kernel_path("/path/to/vmlinux")
//!     .build()?;
//!
//! let vm = MicroVm::create(config)?;
//! vm.run()?;
//! ```

// SAFETY: KVM operations require unsafe but are carefully audited
#![allow(unsafe_code)]

use crate::error::{KernelError, Result};

#[cfg(feature = "std")]
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

// ============================================================================
// KVM CONSTANTS
// ============================================================================

/// KVM API version (expected: 12)
pub const KVM_API_VERSION: u32 = 12;

/// Maximum vCPUs per VM
pub const MAX_VCPUS: u32 = 254;

/// Default memory region slots
pub const MAX_MEMORY_SLOTS: u32 = 32;

// KVM ioctls (from linux/kvm.h) - for future KVM integration
#[allow(dead_code)]
mod kvm_ioctls {
    pub(super) const KVM_GET_API_VERSION: u64 = 0xAE00;
    pub(super) const KVM_CREATE_VM: u64 = 0xAE01;
    pub(super) const KVM_CHECK_EXTENSION: u64 = 0xAE03;
    pub(super) const KVM_GET_VCPU_MMAP_SIZE: u64 = 0xAE04;
    pub(super) const KVM_CREATE_VCPU: u64 = 0xAE41;
    pub(super) const KVM_SET_USER_MEMORY_REGION: u64 = 0x4020_AE46;
    pub(super) const KVM_RUN: u64 = 0xAE80;
    pub(super) const KVM_GET_REGS: u64 = 0x8090_AE81;
    pub(super) const KVM_SET_REGS: u64 = 0x4090_AE82;
    pub(super) const KVM_GET_SREGS: u64 = 0x8138_AE83;
    pub(super) const KVM_SET_SREGS: u64 = 0x4138_AE84;
    pub(super) const KVM_CAP_USER_MEMORY: u32 = 3;
    pub(super) const KVM_CAP_IRQCHIP: u32 = 0;
    pub(super) const KVM_CAP_HLT: u32 = 1;
}

// ============================================================================
// VM STATE
// ============================================================================

/// Virtual machine state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum VmState {
    /// VM is being configured
    #[default]
    Created = 0,
    /// VM is running
    Running = 1,
    /// VM is paused
    Paused = 2,
    /// VM has exited
    Stopped = 3,
    /// VM encountered an error
    Error = 4,
}

impl VmState {
    /// Check if VM can be started
    #[must_use]
    pub const fn can_start(&self) -> bool {
        matches!(self, Self::Created | Self::Paused)
    }

    /// Check if VM is active
    #[must_use]
    pub const fn is_active(&self) -> bool {
        matches!(self, Self::Running | Self::Paused)
    }
}

// ============================================================================
// VM CONFIGURATION
// ============================================================================

/// VM configuration
#[derive(Debug, Clone)]
pub struct VmConfig {
    /// Number of vCPUs
    pub vcpus: u32,
    /// Memory size in MB
    pub memory_mb: u64,
    /// Kernel path (if booting Linux)
    pub kernel_path: Option<String>,
    /// Kernel command line
    pub kernel_cmdline: Option<String>,
    /// Initrd path
    pub initrd_path: Option<String>,
    /// Enable KVM acceleration
    pub enable_kvm: bool,
    /// Socket path for vsock
    pub vsock_path: Option<String>,
}

impl Default for VmConfig {
    fn default() -> Self {
        Self {
            vcpus: 1,
            memory_mb: 128,
            kernel_path: None,
            kernel_cmdline: None,
            initrd_path: None,
            enable_kvm: true,
            vsock_path: None,
        }
    }
}

impl VmConfig {
    /// Create a new builder
    #[must_use]
    pub fn builder() -> VmConfigBuilder {
        VmConfigBuilder::new()
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        if self.vcpus == 0 || self.vcpus > MAX_VCPUS {
            return Err(KernelError::InvalidArgument);
        }
        if self.memory_mb == 0 {
            return Err(KernelError::InvalidArgument);
        }
        Ok(())
    }

    /// Get memory size in bytes
    #[must_use]
    pub const fn memory_bytes(&self) -> u64 {
        self.memory_mb * 1024 * 1024
    }
}

/// Builder for VM configuration
#[derive(Debug, Default)]
pub struct VmConfigBuilder {
    config: VmConfig,
}

impl VmConfigBuilder {
    /// Create a new builder
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set number of vCPUs
    #[must_use]
    pub const fn vcpus(mut self, count: u32) -> Self {
        self.config.vcpus = count;
        self
    }

    /// Set memory size in MB
    #[must_use]
    pub const fn memory_mb(mut self, mb: u64) -> Self {
        self.config.memory_mb = mb;
        self
    }

    /// Set kernel path
    #[must_use]
    pub fn kernel_path(mut self, path: impl Into<String>) -> Self {
        self.config.kernel_path = Some(path.into());
        self
    }

    /// Set kernel command line
    #[must_use]
    pub fn kernel_cmdline(mut self, cmdline: impl Into<String>) -> Self {
        self.config.kernel_cmdline = Some(cmdline.into());
        self
    }

    /// Set initrd path
    #[must_use]
    pub fn initrd_path(mut self, path: impl Into<String>) -> Self {
        self.config.initrd_path = Some(path.into());
        self
    }

    /// Enable/disable KVM
    #[must_use]
    pub const fn enable_kvm(mut self, enable: bool) -> Self {
        self.config.enable_kvm = enable;
        self
    }

    /// Set vsock path
    #[must_use]
    pub fn vsock_path(mut self, path: impl Into<String>) -> Self {
        self.config.vsock_path = Some(path.into());
        self
    }

    /// Build the configuration
    pub fn build(self) -> Result<VmConfig> {
        self.config.validate()?;
        Ok(self.config)
    }
}

// ============================================================================
// VCPU REGISTERS (x86_64)
// ============================================================================

/// x86_64 general-purpose registers
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct VcpuRegs {
    /// RAX register
    pub rax: u64,
    /// RBX register
    pub rbx: u64,
    /// RCX register
    pub rcx: u64,
    /// RDX register
    pub rdx: u64,
    /// RSI register
    pub rsi: u64,
    /// RDI register
    pub rdi: u64,
    /// RSP register
    pub rsp: u64,
    /// RBP register
    pub rbp: u64,
    /// R8 register
    pub r8: u64,
    /// R9 register
    pub r9: u64,
    /// R10 register
    pub r10: u64,
    /// R11 register
    pub r11: u64,
    /// R12 register
    pub r12: u64,
    /// R13 register
    pub r13: u64,
    /// R14 register
    pub r14: u64,
    /// R15 register
    pub r15: u64,
    /// RIP (instruction pointer)
    pub rip: u64,
    /// RFLAGS
    pub rflags: u64,
}

/// x86_64 segment register
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct Segment {
    /// Base address
    pub base: u64,
    /// Limit
    pub limit: u32,
    /// Selector
    pub selector: u16,
    /// Type
    pub type_: u8,
    /// Present
    pub present: u8,
    /// DPL
    pub dpl: u8,
    /// Default big
    pub db: u8,
    /// Segment granularity
    pub s: u8,
    /// Long mode
    pub l: u8,
    /// Granularity
    pub g: u8,
    /// Available
    pub avl: u8,
    /// Unusable
    pub unusable: u8,
    /// Padding
    pub padding: u8,
}

/// x86_64 descriptor table register
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct Dtable {
    /// Base address
    pub base: u64,
    /// Limit
    pub limit: u16,
    /// Padding
    pub padding: [u16; 3],
}

/// x86_64 special registers
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct VcpuSregs {
    /// CS segment
    pub cs: Segment,
    /// DS segment
    pub ds: Segment,
    /// ES segment
    pub es: Segment,
    /// FS segment
    pub fs: Segment,
    /// GS segment
    pub gs: Segment,
    /// SS segment
    pub ss: Segment,
    /// TR segment
    pub tr: Segment,
    /// LDT segment
    pub ldt: Segment,
    /// GDT
    pub gdt: Dtable,
    /// IDT
    pub idt: Dtable,
    /// CR0
    pub cr0: u64,
    /// CR2
    pub cr2: u64,
    /// CR3
    pub cr3: u64,
    /// CR4
    pub cr4: u64,
    /// CR8
    pub cr8: u64,
    /// EFER
    pub efer: u64,
    /// APIC base
    pub apic_base: u64,
    /// Interrupt bitmap
    pub interrupt_bitmap: [u64; 4],
}

// ============================================================================
// EXIT REASON
// ============================================================================

/// VM exit reason
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum ExitReason {
    /// Unknown exit
    Unknown = 0,
    /// Exception or NMI
    Exception = 1,
    /// External interrupt
    ExternalInterrupt = 2,
    /// Triple fault
    TripleFault = 3,
    /// INIT signal
    Init = 4,
    /// Startup IPI
    Sipi = 5,
    /// I/O instruction
    Io = 6,
    /// Halt instruction
    Halt = 7,
    /// CPUID instruction
    Cpuid = 8,
    /// HLT instruction
    Hlt = 9,
    /// MMIO access
    Mmio = 10,
    /// Hypercall
    Hypercall = 11,
    /// Internal error
    InternalError = 12,
    /// Shutdown
    Shutdown = 13,
    /// System event
    SystemEvent = 14,
}

impl From<u32> for ExitReason {
    fn from(value: u32) -> Self {
        match value {
            1 => Self::Exception,
            2 => Self::ExternalInterrupt,
            3 => Self::TripleFault,
            4 => Self::Init,
            5 => Self::Sipi,
            6 => Self::Io,
            7 => Self::Halt,
            8 => Self::Cpuid,
            9 => Self::Hlt,
            10 => Self::Mmio,
            11 => Self::Hypercall,
            12 => Self::InternalError,
            13 => Self::Shutdown,
            14 => Self::SystemEvent,
            _ => Self::Unknown,
        }
    }
}

// ============================================================================
// MEMORY REGION
// ============================================================================

/// Memory region for VM
#[derive(Debug, Clone)]
pub struct MemoryRegion {
    /// Slot number
    pub slot: u32,
    /// Guest physical address
    pub guest_phys_addr: u64,
    /// Memory size
    pub memory_size: u64,
    /// Host virtual address
    pub userspace_addr: u64,
    /// Flags
    pub flags: u32,
}

impl MemoryRegion {
    /// Create a new memory region
    #[must_use]
    pub const fn new(
        slot: u32,
        guest_phys_addr: u64,
        memory_size: u64,
        userspace_addr: u64,
    ) -> Self {
        Self { slot, guest_phys_addr, memory_size, userspace_addr, flags: 0 }
    }

    /// Set readonly flag
    #[must_use]
    pub const fn readonly(mut self) -> Self {
        self.flags |= 1; // KVM_MEM_READONLY
        self
    }
}

// ============================================================================
// VCPU (std only)
// ============================================================================

/// Virtual CPU
#[cfg(feature = "std")]
pub struct Vcpu {
    /// vCPU ID
    id: u32,
    /// vCPU file descriptor (for future KVM integration)
    #[allow(dead_code)]
    fd: i32,
    /// Is running
    running: AtomicBool,
}

#[cfg(feature = "std")]
impl Vcpu {
    /// Create a new vCPU (mock implementation)
    pub fn new(id: u32) -> Result<Self> {
        Ok(Self {
            id,
            fd: -1, // Mock FD
            running: AtomicBool::new(false),
        })
    }

    /// Get vCPU ID
    #[must_use]
    pub const fn id(&self) -> u32 {
        self.id
    }

    /// Check if running
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }

    /// Get registers (mock)
    pub fn get_regs(&self) -> Result<VcpuRegs> {
        Ok(VcpuRegs::default())
    }

    /// Set registers (mock)
    pub fn set_regs(&self, _regs: &VcpuRegs) -> Result<()> {
        Ok(())
    }

    /// Get special registers (mock)
    pub fn get_sregs(&self) -> Result<VcpuSregs> {
        Ok(VcpuSregs::default())
    }

    /// Set special registers (mock)
    pub fn set_sregs(&self, _sregs: &VcpuSregs) -> Result<()> {
        Ok(())
    }

    /// Run vCPU (mock - returns immediately)
    pub fn run(&self) -> Result<ExitReason> {
        self.running.store(true, Ordering::Release);
        // Mock: return Halt immediately
        self.running.store(false, Ordering::Release);
        Ok(ExitReason::Halt)
    }
}

#[cfg(feature = "std")]
impl std::fmt::Debug for Vcpu {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Vcpu").field("id", &self.id).field("running", &self.is_running()).finish()
    }
}

// ============================================================================
// MICROVM (std only)
// ============================================================================

/// Lightweight MicroVM
#[cfg(feature = "std")]
pub struct MicroVm {
    /// Configuration
    config: VmConfig,
    /// VM state
    state: std::sync::RwLock<VmState>,
    /// vCPUs
    vcpus: Vec<Vcpu>,
    /// Guest memory (mock: just track size)
    memory_size: u64,
    /// Memory regions
    regions: Vec<MemoryRegion>,
    /// Exit count
    exit_count: AtomicU64,
}

#[cfg(feature = "std")]
impl MicroVm {
    /// Create a new MicroVM
    pub fn create(config: VmConfig) -> Result<Self> {
        config.validate()?;

        // Create vCPUs
        let mut vcpus = Vec::with_capacity(config.vcpus as usize);
        for i in 0..config.vcpus {
            vcpus.push(Vcpu::new(i)?);
        }

        Ok(Self {
            memory_size: config.memory_bytes(),
            config,
            state: std::sync::RwLock::new(VmState::Created),
            vcpus,
            regions: Vec::new(),
            exit_count: AtomicU64::new(0),
        })
    }

    /// Get VM state
    pub fn state(&self) -> VmState {
        *self.state.read().expect("lock poisoned")
    }

    /// Get configuration
    #[must_use]
    pub const fn config(&self) -> &VmConfig {
        &self.config
    }

    /// Get number of vCPUs
    #[must_use]
    pub fn vcpu_count(&self) -> usize {
        self.vcpus.len()
    }

    /// Get memory size
    #[must_use]
    pub const fn memory_size(&self) -> u64 {
        self.memory_size
    }

    /// Add memory region
    pub fn add_memory_region(&mut self, region: MemoryRegion) -> Result<()> {
        if self.regions.len() >= MAX_MEMORY_SLOTS as usize {
            return Err(KernelError::OutOfMemory);
        }
        self.regions.push(region);
        Ok(())
    }

    /// Run the VM (mock - runs briefly)
    pub fn run(&self) -> Result<ExitReason> {
        {
            let mut state = self.state.write().map_err(|_| KernelError::ResourceBusy)?;
            if !state.can_start() {
                return Err(KernelError::InvalidRequest);
            }
            *state = VmState::Running;
        }

        // Mock: run first vCPU
        let exit = if let Some(vcpu) = self.vcpus.first() { vcpu.run()? } else { ExitReason::Halt };

        self.exit_count.fetch_add(1, Ordering::Relaxed);

        {
            let mut state = self.state.write().map_err(|_| KernelError::ResourceBusy)?;
            *state = VmState::Stopped;
        }

        Ok(exit)
    }

    /// Pause the VM
    pub fn pause(&self) -> Result<()> {
        let mut state = self.state.write().map_err(|_| KernelError::ResourceBusy)?;
        if *state != VmState::Running {
            return Err(KernelError::InvalidRequest);
        }
        *state = VmState::Paused;
        Ok(())
    }

    /// Resume the VM
    pub fn resume(&self) -> Result<()> {
        let mut state = self.state.write().map_err(|_| KernelError::ResourceBusy)?;
        if *state != VmState::Paused {
            return Err(KernelError::InvalidRequest);
        }
        *state = VmState::Running;
        Ok(())
    }

    /// Stop the VM
    pub fn stop(&self) -> Result<()> {
        let mut state = self.state.write().map_err(|_| KernelError::ResourceBusy)?;
        *state = VmState::Stopped;
        Ok(())
    }

    /// Get exit count
    #[must_use]
    pub fn exit_count(&self) -> u64 {
        self.exit_count.load(Ordering::Relaxed)
    }
}

#[cfg(feature = "std")]
impl std::fmt::Debug for MicroVm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MicroVm")
            .field("vcpus", &self.vcpu_count())
            .field("memory_mb", &self.config.memory_mb)
            .field("state", &self.state())
            .finish()
    }
}

// ============================================================================
// JAILER (Security Sandbox)
// ============================================================================

/// Jailer configuration for security isolation
#[derive(Debug, Clone, Default)]
pub struct JailerConfig {
    /// Chroot directory
    pub chroot_dir: Option<String>,
    /// UID to run as
    pub uid: Option<u32>,
    /// GID to run as
    pub gid: Option<u32>,
    /// Enable seccomp filtering
    pub seccomp: bool,
    /// Enable network namespace
    pub netns: bool,
    /// CPU set (cpuset)
    pub cpuset: Option<String>,
    /// Memory limit in bytes
    pub memory_limit: Option<u64>,
}

impl JailerConfig {
    /// Create a minimal secure configuration
    #[must_use]
    pub fn minimal() -> Self {
        Self {
            chroot_dir: Some("/srv/jailer".to_string()),
            uid: Some(65534), // nobody
            gid: Some(65534), // nogroup
            seccomp: true,
            netns: true,
            cpuset: None,
            memory_limit: Some(256 * 1024 * 1024), // 256 MB
        }
    }

    /// Create a production configuration
    #[must_use]
    pub fn production(chroot: impl Into<String>) -> Self {
        Self {
            chroot_dir: Some(chroot.into()),
            uid: Some(65534),
            gid: Some(65534),
            seccomp: true,
            netns: true,
            cpuset: None,
            memory_limit: None,
        }
    }
}

/// Security jailer for VM isolation
#[cfg(feature = "std")]
pub struct Jailer {
    /// Configuration
    config: JailerConfig,
    /// Is active
    active: AtomicBool,
}

#[cfg(feature = "std")]
impl Jailer {
    /// Create a new jailer
    #[must_use]
    pub fn new(config: JailerConfig) -> Self {
        Self { config, active: AtomicBool::new(false) }
    }

    /// Get configuration
    #[must_use]
    pub const fn config(&self) -> &JailerConfig {
        &self.config
    }

    /// Check if jailer is active
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }

    /// Enter jail (mock - just marks as active)
    pub fn enter(&self) -> Result<()> {
        // In a real implementation, this would:
        // 1. chroot to chroot_dir
        // 2. setuid/setgid
        // 3. Apply seccomp filters
        // 4. Enter namespaces
        self.active.store(true, Ordering::Release);
        Ok(())
    }

    /// Exit jail (mock)
    pub fn exit(&self) -> Result<()> {
        self.active.store(false, Ordering::Release);
        Ok(())
    }
}

#[cfg(feature = "std")]
impl std::fmt::Debug for Jailer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Jailer")
            .field("active", &self.is_active())
            .field("seccomp", &self.config.seccomp)
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
    // VmState Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_vm_state_default() {
        assert_eq!(VmState::default(), VmState::Created);
    }

    #[test]
    fn test_vm_state_can_start() {
        assert!(VmState::Created.can_start());
        assert!(VmState::Paused.can_start());
        assert!(!VmState::Running.can_start());
        assert!(!VmState::Stopped.can_start());
    }

    #[test]
    fn test_vm_state_is_active() {
        assert!(VmState::Running.is_active());
        assert!(VmState::Paused.is_active());
        assert!(!VmState::Created.is_active());
        assert!(!VmState::Stopped.is_active());
    }

    // ------------------------------------------------------------------------
    // VmConfig Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_config_default() {
        let config = VmConfig::default();
        assert_eq!(config.vcpus, 1);
        assert_eq!(config.memory_mb, 128);
        assert!(config.enable_kvm);
    }

    #[test]
    fn test_config_builder() {
        let config = VmConfig::builder()
            .vcpus(4)
            .memory_mb(512)
            .kernel_path("/boot/vmlinux")
            .kernel_cmdline("console=ttyS0")
            .build()
            .unwrap();

        assert_eq!(config.vcpus, 4);
        assert_eq!(config.memory_mb, 512);
        assert_eq!(config.kernel_path, Some("/boot/vmlinux".to_string()));
    }

    #[test]
    fn test_config_validate_zero_vcpus() {
        let config = VmConfig::builder().vcpus(0).memory_mb(128);
        assert!(config.build().is_err());
    }

    #[test]
    fn test_config_validate_too_many_vcpus() {
        let config = VmConfig::builder().vcpus(255).memory_mb(128);
        assert!(config.build().is_err());
    }

    #[test]
    fn test_config_memory_bytes() {
        let config = VmConfig::builder().memory_mb(256).build().expect("build failed");
        assert_eq!(config.memory_bytes(), 256 * 1024 * 1024);
    }

    // ------------------------------------------------------------------------
    // VcpuRegs Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_vcpu_regs_default() {
        let regs = VcpuRegs::default();
        assert_eq!(regs.rax, 0);
        assert_eq!(regs.rip, 0);
    }

    // ------------------------------------------------------------------------
    // ExitReason Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_exit_reason_from() {
        assert_eq!(ExitReason::from(6), ExitReason::Io);
        assert_eq!(ExitReason::from(9), ExitReason::Hlt);
        assert_eq!(ExitReason::from(999), ExitReason::Unknown);
    }

    // ------------------------------------------------------------------------
    // MemoryRegion Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_memory_region_new() {
        let region = MemoryRegion::new(0, 0x1000, 4096, 0x7f0000);
        assert_eq!(region.slot, 0);
        assert_eq!(region.guest_phys_addr, 0x1000);
        assert_eq!(region.memory_size, 4096);
        assert_eq!(region.flags, 0);
    }

    #[test]
    fn test_memory_region_readonly() {
        let region = MemoryRegion::new(0, 0, 4096, 0).readonly();
        assert_eq!(region.flags, 1);
    }

    // ------------------------------------------------------------------------
    // Vcpu Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_vcpu_new() {
        let vcpu = Vcpu::new(0).unwrap();
        assert_eq!(vcpu.id(), 0);
        assert!(!vcpu.is_running());
    }

    #[test]
    fn test_vcpu_get_set_regs() {
        let vcpu = Vcpu::new(0).unwrap();
        let regs = vcpu.get_regs().unwrap();
        vcpu.set_regs(&regs).unwrap();
    }

    #[test]
    fn test_vcpu_run() {
        let vcpu = Vcpu::new(0).unwrap();
        let exit = vcpu.run().unwrap();
        assert_eq!(exit, ExitReason::Halt);
    }

    // ------------------------------------------------------------------------
    // MicroVm Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_microvm_create() {
        let config = VmConfig::builder().vcpus(2).memory_mb(256).build().expect("build failed");

        let vm = MicroVm::create(config).unwrap();
        assert_eq!(vm.vcpu_count(), 2);
        assert_eq!(vm.memory_size(), 256 * 1024 * 1024);
        assert_eq!(vm.state(), VmState::Created);
    }

    #[test]
    fn test_microvm_run() {
        let config = VmConfig::builder().build().expect("build failed");
        let vm = MicroVm::create(config).unwrap();

        let exit = vm.run().unwrap();
        assert_eq!(exit, ExitReason::Halt);
        assert_eq!(vm.state(), VmState::Stopped);
        assert_eq!(vm.exit_count(), 1);
    }

    #[test]
    fn test_microvm_add_memory_region() {
        let config = VmConfig::builder().build().expect("build failed");
        let mut vm = MicroVm::create(config).unwrap();

        let region = MemoryRegion::new(0, 0, 4096, 0x7f0000);
        vm.add_memory_region(region).unwrap();
    }

    #[test]
    fn test_microvm_pause_resume() {
        let config = VmConfig::builder().build().expect("build failed");
        let vm = MicroVm::create(config).unwrap();

        // Can't pause when not running
        assert!(vm.pause().is_err());

        // Start and check state
        vm.run().unwrap();
        // After run completes, state is Stopped
        assert_eq!(vm.state(), VmState::Stopped);
    }

    #[test]
    fn test_microvm_debug() {
        let config = VmConfig::builder().vcpus(2).memory_mb(128).build().expect("build failed");
        let vm = MicroVm::create(config).unwrap();
        let debug = format!("{:?}", vm);
        assert!(debug.contains("MicroVm"));
        assert!(debug.contains("vcpus"));
    }

    // ------------------------------------------------------------------------
    // JailerConfig Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_jailer_config_default() {
        let config = JailerConfig::default();
        assert!(config.chroot_dir.is_none());
        assert!(!config.seccomp);
    }

    #[test]
    fn test_jailer_config_minimal() {
        let config = JailerConfig::minimal();
        assert!(config.chroot_dir.is_some());
        assert!(config.seccomp);
        assert!(config.netns);
    }

    #[test]
    fn test_jailer_config_production() {
        let config = JailerConfig::production("/var/lib/jailer");
        assert_eq!(config.chroot_dir, Some("/var/lib/jailer".to_string()));
        assert!(config.seccomp);
    }

    // ------------------------------------------------------------------------
    // Jailer Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_jailer_new() {
        let config = JailerConfig::minimal();
        let jailer = Jailer::new(config);
        assert!(!jailer.is_active());
    }

    #[test]
    fn test_jailer_enter_exit() {
        let config = JailerConfig::minimal();
        let jailer = Jailer::new(config);

        jailer.enter().unwrap();
        assert!(jailer.is_active());

        jailer.exit().unwrap();
        assert!(!jailer.is_active());
    }

    #[test]
    fn test_jailer_debug() {
        let config = JailerConfig::minimal();
        let jailer = Jailer::new(config);
        let debug = format!("{:?}", jailer);
        assert!(debug.contains("Jailer"));
        assert!(debug.contains("seccomp"));
    }
}
