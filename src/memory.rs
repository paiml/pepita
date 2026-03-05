//! Memory management types.
//!
//! This module provides type-safe wrappers for memory addresses and
//! abstractions for kernel memory management operations.
//!
//! ## Type Safety
//!
//! Following the Poka-yoke (mistake-proofing) principle, we use newtype
//! wrappers to prevent mixing up different address types:
//!
//! - [`PhysAddr`]: Physical memory address
//! - [`VirtAddr`]: Virtual memory address
//! - [`Pfn`]: Page frame number
//!
//! ## Example
//!
//! ```rust
//! use pepita::memory::{PhysAddr, VirtAddr, Pfn, PAGE_SIZE};
//!
//! // Create addresses
//! let phys = PhysAddr::new(0x1000_0000);
//! let virt = VirtAddr::new(0xFFFF_8000_0000_0000);
//! let pfn = Pfn::from_addr(phys);
//!
//! assert_eq!(pfn.to_addr(), phys);
//! ```

// ============================================================================
// CONSTANTS
// ============================================================================

/// Page size in bytes (4 KiB for `x86_64/aarch64`)
pub const PAGE_SIZE: usize = 4096;

/// Page shift (log2 of page size)
pub const PAGE_SHIFT: u32 = 12;

/// Page mask for alignment checks
pub const PAGE_MASK: u64 = !(PAGE_SIZE as u64 - 1);

// ============================================================================
// PHYSICAL ADDRESS
// ============================================================================

/// Physical memory address.
///
/// A type-safe wrapper around a raw physical address.
/// Prevents accidental mixing with virtual addresses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[repr(transparent)]
pub struct PhysAddr(u64);

impl PhysAddr {
    /// Create a new physical address.
    #[must_use]
    pub const fn new(addr: u64) -> Self {
        Self(addr)
    }

    /// Create a null physical address.
    #[must_use]
    pub const fn null() -> Self {
        Self(0)
    }

    /// Get the raw address value.
    #[must_use]
    pub const fn as_u64(&self) -> u64 {
        self.0
    }

    /// Check if the address is null (zero).
    #[must_use]
    pub const fn is_null(&self) -> bool {
        self.0 == 0
    }

    /// Check if the address is page-aligned.
    #[must_use]
    pub const fn is_page_aligned(&self) -> bool {
        (self.0 & (PAGE_SIZE as u64 - 1)) == 0
    }

    /// Align the address down to a page boundary.
    #[must_use]
    pub const fn page_align_down(&self) -> Self {
        Self(self.0 & PAGE_MASK)
    }

    /// Align the address up to a page boundary.
    #[must_use]
    pub const fn page_align_up(&self) -> Self {
        Self((self.0 + PAGE_SIZE as u64 - 1) & PAGE_MASK)
    }

    /// Get the offset within a page.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub const fn page_offset(&self) -> usize {
        (self.0 & (PAGE_SIZE as u64 - 1)) as usize
    }

    /// Add an offset to the address.
    #[must_use]
    pub const fn add(&self, offset: u64) -> Self {
        Self(self.0.wrapping_add(offset))
    }

    /// Subtract an offset from the address.
    #[must_use]
    pub const fn sub(&self, offset: u64) -> Self {
        Self(self.0.wrapping_sub(offset))
    }

    /// Check if the address is within a range.
    #[must_use]
    pub const fn is_in_range(&self, start: Self, end: Self) -> bool {
        self.0 >= start.0 && self.0 < end.0
    }
}

// ============================================================================
// VIRTUAL ADDRESS
// ============================================================================

/// Virtual memory address.
///
/// A type-safe wrapper around a raw virtual address.
/// Prevents accidental mixing with physical addresses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[repr(transparent)]
pub struct VirtAddr(u64);

impl VirtAddr {
    /// Create a new virtual address.
    #[must_use]
    pub const fn new(addr: u64) -> Self {
        Self(addr)
    }

    /// Create a null virtual address.
    #[must_use]
    pub const fn null() -> Self {
        Self(0)
    }

    /// Create from a raw pointer.
    ///
    /// # Safety
    ///
    /// The pointer must be valid.
    #[must_use]
    pub fn from_ptr<T>(ptr: *const T) -> Self {
        Self(ptr as u64)
    }

    /// Get the raw address value.
    #[must_use]
    pub const fn as_u64(&self) -> u64 {
        self.0
    }

    /// Check if the address is null (zero).
    #[must_use]
    pub const fn is_null(&self) -> bool {
        self.0 == 0
    }

    /// Check if the address is page-aligned.
    #[must_use]
    pub const fn is_page_aligned(&self) -> bool {
        (self.0 & (PAGE_SIZE as u64 - 1)) == 0
    }

    /// Align the address down to a page boundary.
    #[must_use]
    pub const fn page_align_down(&self) -> Self {
        Self(self.0 & PAGE_MASK)
    }

    /// Align the address up to a page boundary.
    #[must_use]
    pub const fn page_align_up(&self) -> Self {
        Self((self.0 + PAGE_SIZE as u64 - 1) & PAGE_MASK)
    }

    /// Get the offset within a page.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub const fn page_offset(&self) -> usize {
        (self.0 & (PAGE_SIZE as u64 - 1)) as usize
    }

    /// Add an offset to the address.
    #[must_use]
    pub const fn add(&self, offset: u64) -> Self {
        Self(self.0.wrapping_add(offset))
    }

    /// Subtract an offset from the address.
    #[must_use]
    pub const fn sub(&self, offset: u64) -> Self {
        Self(self.0.wrapping_sub(offset))
    }
}

// ============================================================================
// PAGE FRAME NUMBER
// ============================================================================

/// Page frame number (PFN).
///
/// Represents a physical page by its frame number rather than address.
/// Commonly used in kernel memory management.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[repr(transparent)]
pub struct Pfn(u64);

impl Pfn {
    /// Create a new page frame number.
    #[must_use]
    pub const fn new(pfn: u64) -> Self {
        Self(pfn)
    }

    /// Create a null PFN.
    #[must_use]
    pub const fn null() -> Self {
        Self(0)
    }

    /// Create a PFN from a physical address.
    #[must_use]
    pub const fn from_addr(addr: PhysAddr) -> Self {
        Self(addr.as_u64() >> PAGE_SHIFT)
    }

    /// Convert the PFN to a physical address.
    #[must_use]
    pub const fn to_addr(&self) -> PhysAddr {
        PhysAddr::new(self.0 << PAGE_SHIFT)
    }

    /// Get the raw PFN value.
    #[must_use]
    pub const fn as_u64(&self) -> u64 {
        self.0
    }

    /// Check if the PFN is null (zero).
    #[must_use]
    pub const fn is_null(&self) -> bool {
        self.0 == 0
    }

    /// Add to the PFN.
    #[must_use]
    pub const fn add(&self, pages: u64) -> Self {
        Self(self.0.wrapping_add(pages))
    }

    /// Subtract from the PFN.
    #[must_use]
    pub const fn sub(&self, pages: u64) -> Self {
        Self(self.0.wrapping_sub(pages))
    }
}

// ============================================================================
// DMA DIRECTION
// ============================================================================

/// DMA transfer direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DmaDirection {
    /// Transfer from host to device
    ToDevice = 0,
    /// Transfer from device to host
    FromDevice = 1,
    /// Transfer in both directions
    Bidirectional = 2,
    /// No direction (for sync operations)
    None = 3,
}

impl DmaDirection {
    /// Check if this direction includes transfers to the device.
    #[must_use]
    pub const fn to_device(&self) -> bool {
        matches!(self, Self::ToDevice | Self::Bidirectional)
    }

    /// Check if this direction includes transfers from the device.
    #[must_use]
    pub const fn from_device(&self) -> bool {
        matches!(self, Self::FromDevice | Self::Bidirectional)
    }
}

// ============================================================================
// DMA BUFFER
// ============================================================================

/// DMA buffer descriptor.
///
/// Describes a buffer that can be used for DMA transfers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DmaBuffer {
    /// Physical address of the buffer
    pub phys: PhysAddr,
    /// Virtual address of the buffer (kernel mapping)
    pub virt: VirtAddr,
    /// Buffer size in bytes
    pub size: usize,
    /// DMA direction
    pub direction: DmaDirection,
}

impl DmaBuffer {
    /// Create a new DMA buffer descriptor.
    #[must_use]
    pub const fn new(phys: PhysAddr, virt: VirtAddr, size: usize, direction: DmaDirection) -> Self {
        Self {
            phys,
            virt,
            size,
            direction,
        }
    }

    /// Check if the buffer is valid.
    #[must_use]
    pub const fn is_valid(&self) -> bool {
        !self.phys.is_null() && !self.virt.is_null() && self.size > 0
    }

    /// Check if the buffer is page-aligned.
    #[must_use]
    pub const fn is_page_aligned(&self) -> bool {
        self.phys.is_page_aligned() && self.virt.is_page_aligned()
    }

    /// Get the number of pages covered by this buffer.
    #[must_use]
    pub const fn page_count(&self) -> usize {
        self.size.div_ceil(PAGE_SIZE)
    }
}

// ============================================================================
// ALLOCATION FLAGS
// ============================================================================

/// Memory allocation flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AllocFlags(u32);

impl AllocFlags {
    /// No special flags
    pub const NONE: Self = Self(0);

    /// Allocation can wait (may sleep)
    pub const WAIT: Self = Self(1 << 0);

    /// Zero the allocated memory
    pub const ZERO: Self = Self(1 << 1);

    /// Allocate from DMA-able memory
    pub const DMA: Self = Self(1 << 2);

    /// Allocate from high memory
    pub const HIGHMEM: Self = Self(1 << 3);

    /// Create new flags.
    #[must_use]
    pub const fn new() -> Self {
        Self::NONE
    }

    /// Add the wait flag.
    #[must_use]
    pub const fn with_wait(self) -> Self {
        Self(self.0 | Self::WAIT.0)
    }

    /// Add the zero flag.
    #[must_use]
    pub const fn with_zero(self) -> Self {
        Self(self.0 | Self::ZERO.0)
    }

    /// Add the DMA flag.
    #[must_use]
    pub const fn with_dma(self) -> Self {
        Self(self.0 | Self::DMA.0)
    }

    /// Check if waiting is allowed.
    #[must_use]
    pub const fn can_wait(&self) -> bool {
        (self.0 & Self::WAIT.0) != 0
    }

    /// Check if memory should be zeroed.
    #[must_use]
    pub const fn should_zero(&self) -> bool {
        (self.0 & Self::ZERO.0) != 0
    }

    /// Check if DMA-able memory is required.
    #[must_use]
    pub const fn needs_dma(&self) -> bool {
        (self.0 & Self::DMA.0) != 0
    }
}

// ============================================================================
// PAGE ALLOCATOR TRAIT
// ============================================================================

/// Page allocator trait.
///
/// Implement this trait to provide page allocation for the kernel.
pub trait PageAllocator {
    /// Allocate contiguous pages.
    ///
    /// # Arguments
    ///
    /// * `order` - Log2 of the number of pages to allocate
    /// * `flags` - Allocation flags
    ///
    /// # Returns
    ///
    /// The PFN of the first page, or `None` if allocation failed.
    fn alloc_pages(&mut self, order: u32, flags: AllocFlags) -> Option<Pfn>;

    /// Free previously allocated pages.
    ///
    /// # Arguments
    ///
    /// * `pfn` - The PFN of the first page
    /// * `order` - Log2 of the number of pages
    fn free_pages(&mut self, pfn: Pfn, order: u32);

    /// Get the number of free pages.
    fn free_count(&self) -> usize;
}

// ============================================================================
// MMAP PROTECTION
// ============================================================================

/// Memory protection flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Protection(u32);

impl Protection {
    /// No access
    pub const NONE: Self = Self(0);

    /// Read access
    pub const READ: Self = Self(1 << 0);

    /// Write access
    pub const WRITE: Self = Self(1 << 1);

    /// Execute access
    pub const EXEC: Self = Self(1 << 2);

    /// Read + Write
    pub const READ_WRITE: Self = Self(Self::READ.0 | Self::WRITE.0);

    /// Check if readable.
    #[must_use]
    pub const fn is_readable(&self) -> bool {
        (self.0 & Self::READ.0) != 0
    }

    /// Check if writable.
    #[must_use]
    pub const fn is_writable(&self) -> bool {
        (self.0 & Self::WRITE.0) != 0
    }

    /// Check if executable.
    #[must_use]
    pub const fn is_executable(&self) -> bool {
        (self.0 & Self::EXEC.0) != 0
    }
}

// ============================================================================
// TESTS (EXTREME TDD)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------------------
    // Constants Tests (Falsification Checklist Points 14-15)
    // ------------------------------------------------------------------------

    #[test]
    fn abi_page_size() {
        assert_eq!(PAGE_SIZE, 4096, "PAGE_SIZE must be 4096");
    }

    #[test]
    fn abi_page_shift() {
        assert_eq!(PAGE_SHIFT, 12, "PAGE_SHIFT must be 12");
        assert_eq!(1 << PAGE_SHIFT, PAGE_SIZE);
    }

    #[test]
    fn abi_page_mask() {
        assert_eq!(PAGE_MASK, !0xFFF_u64);
    }

    // ------------------------------------------------------------------------
    // PhysAddr Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_phys_addr_new() {
        let addr = PhysAddr::new(0x1234_5678);
        assert_eq!(addr.as_u64(), 0x1234_5678);
    }

    #[test]
    fn test_phys_addr_null() {
        let addr = PhysAddr::null();
        assert!(addr.is_null());
        assert_eq!(addr.as_u64(), 0);
    }

    #[test]
    fn test_phys_addr_page_aligned() {
        assert!(PhysAddr::new(0x1000).is_page_aligned());
        assert!(PhysAddr::new(0x0).is_page_aligned());
        assert!(!PhysAddr::new(0x1001).is_page_aligned());
        assert!(!PhysAddr::new(0xFFF).is_page_aligned());
    }

    #[test]
    fn test_phys_addr_align_down() {
        assert_eq!(
            PhysAddr::new(0x1234).page_align_down(),
            PhysAddr::new(0x1000)
        );
        assert_eq!(
            PhysAddr::new(0x1000).page_align_down(),
            PhysAddr::new(0x1000)
        );
    }

    #[test]
    fn test_phys_addr_align_up() {
        assert_eq!(PhysAddr::new(0x1001).page_align_up(), PhysAddr::new(0x2000));
        assert_eq!(PhysAddr::new(0x1000).page_align_up(), PhysAddr::new(0x1000));
    }

    #[test]
    fn test_phys_addr_page_offset() {
        assert_eq!(PhysAddr::new(0x1234).page_offset(), 0x234);
        assert_eq!(PhysAddr::new(0x1000).page_offset(), 0);
    }

    #[test]
    fn test_phys_addr_arithmetic() {
        let addr = PhysAddr::new(0x1000);
        assert_eq!(addr.add(0x100), PhysAddr::new(0x1100));
        assert_eq!(addr.sub(0x100), PhysAddr::new(0x0F00));
    }

    #[test]
    fn test_phys_addr_range() {
        let addr = PhysAddr::new(0x1500);
        assert!(addr.is_in_range(PhysAddr::new(0x1000), PhysAddr::new(0x2000)));
        assert!(!addr.is_in_range(PhysAddr::new(0x2000), PhysAddr::new(0x3000)));
    }

    // ------------------------------------------------------------------------
    // VirtAddr Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_virt_addr_new() {
        let addr = VirtAddr::new(0xFFFF_8000_0000_0000);
        assert_eq!(addr.as_u64(), 0xFFFF_8000_0000_0000);
    }

    #[test]
    fn test_virt_addr_null() {
        let addr = VirtAddr::null();
        assert!(addr.is_null());
    }

    #[test]
    fn test_virt_addr_from_ptr() {
        let value: u64 = 42;
        let ptr = &value as *const u64;
        let addr = VirtAddr::from_ptr(ptr);
        assert!(!addr.is_null());
    }

    #[test]
    fn test_virt_addr_page_aligned() {
        assert!(VirtAddr::new(0x1000).is_page_aligned());
        assert!(!VirtAddr::new(0x1001).is_page_aligned());
    }

    // ------------------------------------------------------------------------
    // Pfn Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_pfn_new() {
        let pfn = Pfn::new(100);
        assert_eq!(pfn.as_u64(), 100);
    }

    #[test]
    fn test_pfn_from_addr() {
        let addr = PhysAddr::new(0x100_000); // 1 MB
        let pfn = Pfn::from_addr(addr);
        assert_eq!(pfn.as_u64(), 0x100); // 256 pages
    }

    #[test]
    fn test_pfn_to_addr() {
        let pfn = Pfn::new(0x100);
        let addr = pfn.to_addr();
        assert_eq!(addr.as_u64(), 0x100_000);
    }

    #[test]
    fn test_pfn_roundtrip() {
        let original = PhysAddr::new(0x1234_0000);
        let pfn = Pfn::from_addr(original);
        let recovered = pfn.to_addr();
        assert_eq!(recovered, original.page_align_down());
    }

    #[test]
    fn test_pfn_arithmetic() {
        let pfn = Pfn::new(100);
        assert_eq!(pfn.add(10), Pfn::new(110));
        assert_eq!(pfn.sub(10), Pfn::new(90));
    }

    // ------------------------------------------------------------------------
    // DmaDirection Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_dma_direction() {
        assert!(DmaDirection::ToDevice.to_device());
        assert!(!DmaDirection::ToDevice.from_device());

        assert!(!DmaDirection::FromDevice.to_device());
        assert!(DmaDirection::FromDevice.from_device());

        assert!(DmaDirection::Bidirectional.to_device());
        assert!(DmaDirection::Bidirectional.from_device());
    }

    // ------------------------------------------------------------------------
    // DmaBuffer Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_dma_buffer_new() {
        let buf = DmaBuffer::new(
            PhysAddr::new(0x1000),
            VirtAddr::new(0xFFFF_8000_0000_1000),
            4096,
            DmaDirection::Bidirectional,
        );
        assert!(buf.is_valid());
        assert!(buf.is_page_aligned());
        assert_eq!(buf.page_count(), 1);
    }

    #[test]
    fn test_dma_buffer_invalid() {
        let buf = DmaBuffer::new(PhysAddr::null(), VirtAddr::null(), 0, DmaDirection::None);
        assert!(!buf.is_valid());
    }

    #[test]
    fn test_dma_buffer_page_count() {
        let buf1 = DmaBuffer::new(
            PhysAddr::new(0x1000),
            VirtAddr::new(0x1000),
            4096,
            DmaDirection::ToDevice,
        );
        assert_eq!(buf1.page_count(), 1);

        let buf2 = DmaBuffer::new(
            PhysAddr::new(0x1000),
            VirtAddr::new(0x1000),
            4097,
            DmaDirection::ToDevice,
        );
        assert_eq!(buf2.page_count(), 2);

        let buf3 = DmaBuffer::new(
            PhysAddr::new(0x1000),
            VirtAddr::new(0x1000),
            8192,
            DmaDirection::ToDevice,
        );
        assert_eq!(buf3.page_count(), 2);
    }

    // ------------------------------------------------------------------------
    // AllocFlags Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_alloc_flags_default() {
        let flags = AllocFlags::default();
        assert!(!flags.can_wait());
        assert!(!flags.should_zero());
        assert!(!flags.needs_dma());
    }

    #[test]
    fn test_alloc_flags_builders() {
        let flags = AllocFlags::new().with_wait().with_zero().with_dma();
        assert!(flags.can_wait());
        assert!(flags.should_zero());
        assert!(flags.needs_dma());
    }

    // ------------------------------------------------------------------------
    // Protection Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_protection_none() {
        let prot = Protection::NONE;
        assert!(!prot.is_readable());
        assert!(!prot.is_writable());
        assert!(!prot.is_executable());
    }

    #[test]
    fn test_protection_read_write() {
        let prot = Protection::READ_WRITE;
        assert!(prot.is_readable());
        assert!(prot.is_writable());
        assert!(!prot.is_executable());
    }

    // ------------------------------------------------------------------------
    // Type Safety Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_type_sizes() {
        // All address types should be 8 bytes
        assert_eq!(core::mem::size_of::<PhysAddr>(), 8);
        assert_eq!(core::mem::size_of::<VirtAddr>(), 8);
        assert_eq!(core::mem::size_of::<Pfn>(), 8);
    }

    #[test]
    fn test_type_alignment() {
        // All address types should be 8-byte aligned
        assert_eq!(core::mem::align_of::<PhysAddr>(), 8);
        assert_eq!(core::mem::align_of::<VirtAddr>(), 8);
        assert_eq!(core::mem::align_of::<Pfn>(), 8);
    }
}
