//! SIMD: Vectorized Operations
//!
//! Pure Rust SIMD operations with runtime CPU feature detection.
//! Supports AVX-512, AVX2, SSE4.2 on x86_64 and NEON on aarch64.
//!
//! ## Example
//!
//! ```rust,ignore
//! use pepita::simd::{SimdCapabilities, SimdOps};
//!
//! let caps = SimdCapabilities::detect();
//! println!("Best vector width: {} bits", caps.best_vector_width());
//!
//! let ops = SimdOps::new();
//! let a = vec![1.0f32; 1024];
//! let b = vec![2.0f32; 1024];
//! let mut c = vec![0.0f32; 1024];
//! ops.vadd_f32(&a, &b, &mut c);
//! ```

// SAFETY: SIMD intrinsics require unsafe but are well-audited
#![allow(unsafe_code)]

// ============================================================================
// SIMD CAPABILITIES
// ============================================================================

/// SIMD feature detection (runtime)
#[derive(Debug, Clone, Copy, Default)]
pub struct SimdCapabilities {
    /// SSE4.1 support
    pub sse41: bool,
    /// SSE4.2 support
    pub sse42: bool,
    /// AVX support
    pub avx: bool,
    /// AVX2 support
    pub avx2: bool,
    /// FMA (Fused Multiply-Add) support
    pub fma: bool,
    /// AVX-512 Foundation support
    pub avx512f: bool,
    /// AVX-512 Vector Length Extensions
    pub avx512vl: bool,
    /// ARM NEON support
    pub neon: bool,
}

impl SimdCapabilities {
    /// Detect CPU SIMD capabilities at runtime
    #[must_use]
    pub fn detect() -> Self {
        #[cfg(target_arch = "x86_64")]
        {
            Self {
                sse41: std::arch::is_x86_feature_detected!("sse4.1"),
                sse42: std::arch::is_x86_feature_detected!("sse4.2"),
                avx: std::arch::is_x86_feature_detected!("avx"),
                avx2: std::arch::is_x86_feature_detected!("avx2"),
                fma: std::arch::is_x86_feature_detected!("fma"),
                avx512f: std::arch::is_x86_feature_detected!("avx512f"),
                avx512vl: std::arch::is_x86_feature_detected!("avx512vl"),
                neon: false,
            }
        }

        #[cfg(target_arch = "aarch64")]
        {
            Self {
                sse41: false,
                sse42: false,
                avx: false,
                avx2: false,
                fma: false,
                avx512f: false,
                avx512vl: false,
                neon: true, // Always available on aarch64
            }
        }

        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
        {
            Self::default()
        }
    }

    /// Get best available vector width in bits
    #[must_use]
    pub const fn best_vector_width(&self) -> u32 {
        if self.avx512f {
            512
        } else if self.avx2 || self.avx {
            256
        } else if self.sse42 || self.sse41 {
            128
        } else if self.neon {
            128
        } else {
            64 // Scalar
        }
    }

    /// Get best available float width (elements per vector)
    #[must_use]
    pub const fn best_f32_width(&self) -> usize {
        (self.best_vector_width() / 32) as usize
    }

    /// Get best available double width (elements per vector)
    #[must_use]
    pub const fn best_f64_width(&self) -> usize {
        (self.best_vector_width() / 64) as usize
    }

    /// Check if any SIMD is available
    #[must_use]
    pub const fn has_simd(&self) -> bool {
        self.sse41 || self.neon
    }

    /// Get human-readable description
    #[must_use]
    pub fn description(&self) -> &'static str {
        if self.avx512f {
            "AVX-512"
        } else if self.avx2 {
            "AVX2"
        } else if self.avx {
            "AVX"
        } else if self.sse42 {
            "SSE4.2"
        } else if self.sse41 {
            "SSE4.1"
        } else if self.neon {
            "NEON"
        } else {
            "Scalar"
        }
    }
}

// ============================================================================
// SIMD OPERATIONS
// ============================================================================

/// SIMD-accelerated operations
#[derive(Debug, Clone)]
pub struct SimdOps {
    /// Detected capabilities
    caps: SimdCapabilities,
}

impl SimdOps {
    /// Create a new SIMD operations instance
    #[must_use]
    pub fn new() -> Self {
        Self {
            caps: SimdCapabilities::detect(),
        }
    }

    /// Create with specific capabilities (for testing)
    #[must_use]
    pub const fn with_caps(caps: SimdCapabilities) -> Self {
        Self { caps }
    }

    /// Get capabilities
    #[must_use]
    pub const fn caps(&self) -> &SimdCapabilities {
        &self.caps
    }

    // ========================================================================
    // VECTOR ADDITION
    // ========================================================================

    /// Vector addition: c = a + b (f32)
    pub fn vadd_f32(&self, a: &[f32], b: &[f32], c: &mut [f32]) {
        assert_eq!(a.len(), b.len());
        assert_eq!(a.len(), c.len());

        #[cfg(target_arch = "x86_64")]
        {
            if self.caps.avx512f && a.len() >= 16 {
                unsafe {
                    self.vadd_f32_avx512(a, b, c);
                }
                return;
            }
            if self.caps.avx2 && a.len() >= 8 {
                unsafe {
                    self.vadd_f32_avx2(a, b, c);
                }
                return;
            }
            if self.caps.sse42 && a.len() >= 4 {
                unsafe {
                    self.vadd_f32_sse(a, b, c);
                }
                return;
            }
        }

        #[cfg(target_arch = "aarch64")]
        {
            if self.caps.neon && a.len() >= 4 {
                unsafe {
                    self.vadd_f32_neon(a, b, c);
                }
                return;
            }
        }

        self.vadd_f32_scalar(a, b, c);
    }

    /// Vector addition: c = a + b (f64)
    pub fn vadd_f64(&self, a: &[f64], b: &[f64], c: &mut [f64]) {
        assert_eq!(a.len(), b.len());
        assert_eq!(a.len(), c.len());

        #[cfg(target_arch = "x86_64")]
        {
            if self.caps.avx512f && a.len() >= 8 {
                unsafe {
                    self.vadd_f64_avx512(a, b, c);
                }
                return;
            }
            if self.caps.avx2 && a.len() >= 4 {
                unsafe {
                    self.vadd_f64_avx2(a, b, c);
                }
                return;
            }
        }

        self.vadd_f64_scalar(a, b, c);
    }

    // ========================================================================
    // VECTOR MULTIPLICATION
    // ========================================================================

    /// Vector multiplication: c = a * b (f32)
    pub fn vmul_f32(&self, a: &[f32], b: &[f32], c: &mut [f32]) {
        assert_eq!(a.len(), b.len());
        assert_eq!(a.len(), c.len());

        #[cfg(target_arch = "x86_64")]
        {
            if self.caps.avx512f && a.len() >= 16 {
                unsafe {
                    self.vmul_f32_avx512(a, b, c);
                }
                return;
            }
            if self.caps.avx2 && a.len() >= 8 {
                unsafe {
                    self.vmul_f32_avx2(a, b, c);
                }
                return;
            }
        }

        self.vmul_f32_scalar(a, b, c);
    }

    // ========================================================================
    // DOT PRODUCT
    // ========================================================================

    /// Dot product: sum(a[i] * b[i])
    #[must_use]
    pub fn dot_f32(&self, a: &[f32], b: &[f32]) -> f32 {
        assert_eq!(a.len(), b.len());

        #[cfg(target_arch = "x86_64")]
        {
            if self.caps.avx2 && a.len() >= 8 {
                return unsafe { self.dot_f32_avx2(a, b) };
            }
        }

        self.dot_f32_scalar(a, b)
    }

    // ========================================================================
    // SCALAR FALLBACKS
    // ========================================================================

    fn vadd_f32_scalar(&self, a: &[f32], b: &[f32], c: &mut [f32]) {
        for i in 0..a.len() {
            c[i] = a[i] + b[i];
        }
    }

    fn vadd_f64_scalar(&self, a: &[f64], b: &[f64], c: &mut [f64]) {
        for i in 0..a.len() {
            c[i] = a[i] + b[i];
        }
    }

    fn vmul_f32_scalar(&self, a: &[f32], b: &[f32], c: &mut [f32]) {
        for i in 0..a.len() {
            c[i] = a[i] * b[i];
        }
    }

    fn dot_f32_scalar(&self, a: &[f32], b: &[f32]) -> f32 {
        a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
    }

    // ========================================================================
    // AVX-512 IMPLEMENTATIONS
    // ========================================================================

    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx512f")]
    unsafe fn vadd_f32_avx512(&self, a: &[f32], b: &[f32], c: &mut [f32]) {
        use std::arch::x86_64::*;

        let chunks = a.len() / 16;
        for i in 0..chunks {
            let offset = i * 16;
            let va = _mm512_loadu_ps(a.as_ptr().add(offset));
            let vb = _mm512_loadu_ps(b.as_ptr().add(offset));
            let vc = _mm512_add_ps(va, vb);
            _mm512_storeu_ps(c.as_mut_ptr().add(offset), vc);
        }

        // Handle remainder
        let start = chunks * 16;
        for i in start..a.len() {
            c[i] = a[i] + b[i];
        }
    }

    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx512f")]
    unsafe fn vadd_f64_avx512(&self, a: &[f64], b: &[f64], c: &mut [f64]) {
        use std::arch::x86_64::*;

        let chunks = a.len() / 8;
        for i in 0..chunks {
            let offset = i * 8;
            let va = _mm512_loadu_pd(a.as_ptr().add(offset));
            let vb = _mm512_loadu_pd(b.as_ptr().add(offset));
            let vc = _mm512_add_pd(va, vb);
            _mm512_storeu_pd(c.as_mut_ptr().add(offset), vc);
        }

        let start = chunks * 8;
        for i in start..a.len() {
            c[i] = a[i] + b[i];
        }
    }

    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx512f")]
    unsafe fn vmul_f32_avx512(&self, a: &[f32], b: &[f32], c: &mut [f32]) {
        use std::arch::x86_64::*;

        let chunks = a.len() / 16;
        for i in 0..chunks {
            let offset = i * 16;
            let va = _mm512_loadu_ps(a.as_ptr().add(offset));
            let vb = _mm512_loadu_ps(b.as_ptr().add(offset));
            let vc = _mm512_mul_ps(va, vb);
            _mm512_storeu_ps(c.as_mut_ptr().add(offset), vc);
        }

        let start = chunks * 16;
        for i in start..a.len() {
            c[i] = a[i] * b[i];
        }
    }

    // ========================================================================
    // AVX2 IMPLEMENTATIONS
    // ========================================================================

    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx2")]
    unsafe fn vadd_f32_avx2(&self, a: &[f32], b: &[f32], c: &mut [f32]) {
        use std::arch::x86_64::*;

        let chunks = a.len() / 8;
        for i in 0..chunks {
            let offset = i * 8;
            let va = _mm256_loadu_ps(a.as_ptr().add(offset));
            let vb = _mm256_loadu_ps(b.as_ptr().add(offset));
            let vc = _mm256_add_ps(va, vb);
            _mm256_storeu_ps(c.as_mut_ptr().add(offset), vc);
        }

        let start = chunks * 8;
        for i in start..a.len() {
            c[i] = a[i] + b[i];
        }
    }

    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx2")]
    unsafe fn vadd_f64_avx2(&self, a: &[f64], b: &[f64], c: &mut [f64]) {
        use std::arch::x86_64::*;

        let chunks = a.len() / 4;
        for i in 0..chunks {
            let offset = i * 4;
            let va = _mm256_loadu_pd(a.as_ptr().add(offset));
            let vb = _mm256_loadu_pd(b.as_ptr().add(offset));
            let vc = _mm256_add_pd(va, vb);
            _mm256_storeu_pd(c.as_mut_ptr().add(offset), vc);
        }

        let start = chunks * 4;
        for i in start..a.len() {
            c[i] = a[i] + b[i];
        }
    }

    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx2")]
    unsafe fn vmul_f32_avx2(&self, a: &[f32], b: &[f32], c: &mut [f32]) {
        use std::arch::x86_64::*;

        let chunks = a.len() / 8;
        for i in 0..chunks {
            let offset = i * 8;
            let va = _mm256_loadu_ps(a.as_ptr().add(offset));
            let vb = _mm256_loadu_ps(b.as_ptr().add(offset));
            let vc = _mm256_mul_ps(va, vb);
            _mm256_storeu_ps(c.as_mut_ptr().add(offset), vc);
        }

        let start = chunks * 8;
        for i in start..a.len() {
            c[i] = a[i] * b[i];
        }
    }

    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx2")]
    unsafe fn dot_f32_avx2(&self, a: &[f32], b: &[f32]) -> f32 {
        use std::arch::x86_64::*;

        let mut sum = _mm256_setzero_ps();
        let chunks = a.len() / 8;

        for i in 0..chunks {
            let offset = i * 8;
            let va = _mm256_loadu_ps(a.as_ptr().add(offset));
            let vb = _mm256_loadu_ps(b.as_ptr().add(offset));
            let prod = _mm256_mul_ps(va, vb);
            sum = _mm256_add_ps(sum, prod);
        }

        // Horizontal sum
        let sum128 = _mm_add_ps(
            _mm256_castps256_ps128(sum),
            _mm256_extractf128_ps(sum, 1),
        );
        let sum64 = _mm_add_ps(sum128, _mm_movehl_ps(sum128, sum128));
        let sum32 = _mm_add_ss(sum64, _mm_shuffle_ps(sum64, sum64, 1));
        let mut result = _mm_cvtss_f32(sum32);

        // Handle remainder
        let start = chunks * 8;
        for i in start..a.len() {
            result += a[i] * b[i];
        }

        result
    }

    // ========================================================================
    // SSE IMPLEMENTATIONS
    // ========================================================================

    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "sse4.2")]
    unsafe fn vadd_f32_sse(&self, a: &[f32], b: &[f32], c: &mut [f32]) {
        use std::arch::x86_64::*;

        let chunks = a.len() / 4;
        for i in 0..chunks {
            let offset = i * 4;
            let va = _mm_loadu_ps(a.as_ptr().add(offset));
            let vb = _mm_loadu_ps(b.as_ptr().add(offset));
            let vc = _mm_add_ps(va, vb);
            _mm_storeu_ps(c.as_mut_ptr().add(offset), vc);
        }

        let start = chunks * 4;
        for i in start..a.len() {
            c[i] = a[i] + b[i];
        }
    }

    // ========================================================================
    // NEON IMPLEMENTATIONS (aarch64)
    // ========================================================================

    #[cfg(target_arch = "aarch64")]
    unsafe fn vadd_f32_neon(&self, a: &[f32], b: &[f32], c: &mut [f32]) {
        use std::arch::aarch64::*;

        let chunks = a.len() / 4;
        for i in 0..chunks {
            let offset = i * 4;
            let va = vld1q_f32(a.as_ptr().add(offset));
            let vb = vld1q_f32(b.as_ptr().add(offset));
            let vc = vaddq_f32(va, vb);
            vst1q_f32(c.as_mut_ptr().add(offset), vc);
        }

        let start = chunks * 4;
        for i in start..a.len() {
            c[i] = a[i] + b[i];
        }
    }
}

impl Default for SimdOps {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// MATRIX OPERATIONS
// ============================================================================

/// SIMD-accelerated matrix operations
pub struct MatrixOps {
    /// SIMD operations instance
    #[allow(dead_code)]
    simd: SimdOps,
}

impl MatrixOps {
    /// Create a new matrix operations instance
    #[must_use]
    pub fn new() -> Self {
        Self {
            simd: SimdOps::new(),
        }
    }

    /// Get SIMD capabilities
    #[must_use]
    pub fn caps(&self) -> &SimdCapabilities {
        self.simd.caps()
    }

    /// Matrix multiplication: C = A @ B (row-major)
    ///
    /// A: m x k matrix
    /// B: k x n matrix
    /// C: m x n matrix
    pub fn matmul_f32(
        &self,
        a: &[f32],
        b: &[f32],
        c: &mut [f32],
        m: usize,
        k: usize,
        n: usize,
    ) {
        assert_eq!(a.len(), m * k);
        assert_eq!(b.len(), k * n);
        assert_eq!(c.len(), m * n);

        // Zero output
        c.fill(0.0);

        // Naive O(n^3) with cache-friendly tiling
        const TILE_SIZE: usize = 32;

        for i0 in (0..m).step_by(TILE_SIZE) {
            let i_end = (i0 + TILE_SIZE).min(m);
            for j0 in (0..n).step_by(TILE_SIZE) {
                let j_end = (j0 + TILE_SIZE).min(n);
                for k0 in (0..k).step_by(TILE_SIZE) {
                    let k_end = (k0 + TILE_SIZE).min(k);

                    // Inner tile multiplication
                    for i in i0..i_end {
                        for kk in k0..k_end {
                            let a_val = a[i * k + kk];
                            for j in j0..j_end {
                                c[i * n + j] += a_val * b[kk * n + j];
                            }
                        }
                    }
                }
            }
        }
    }

    /// Matrix transpose
    pub fn transpose_f32(&self, src: &[f32], dst: &mut [f32], rows: usize, cols: usize) {
        assert_eq!(src.len(), rows * cols);
        assert_eq!(dst.len(), rows * cols);

        for i in 0..rows {
            for j in 0..cols {
                dst[j * rows + i] = src[i * cols + j];
            }
        }
    }
}

impl Default for MatrixOps {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------------------
    // SimdCapabilities Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_capabilities_detect() {
        let caps = SimdCapabilities::detect();
        // Should detect something on modern CPUs
        println!("Detected: {}", caps.description());
        println!("Vector width: {} bits", caps.best_vector_width());
    }

    #[test]
    fn test_capabilities_width() {
        let mut caps = SimdCapabilities::default();

        caps.avx512f = true;
        assert_eq!(caps.best_vector_width(), 512);
        assert_eq!(caps.best_f32_width(), 16);
        assert_eq!(caps.best_f64_width(), 8);

        caps.avx512f = false;
        caps.avx2 = true;
        assert_eq!(caps.best_vector_width(), 256);
        assert_eq!(caps.best_f32_width(), 8);

        caps.avx2 = false;
        caps.sse42 = true;
        assert_eq!(caps.best_vector_width(), 128);
        assert_eq!(caps.best_f32_width(), 4);
    }

    #[test]
    fn test_capabilities_has_simd() {
        let mut caps = SimdCapabilities::default();
        assert!(!caps.has_simd());

        caps.sse41 = true;
        assert!(caps.has_simd());

        caps.sse41 = false;
        caps.neon = true;
        assert!(caps.has_simd());
    }

    // ------------------------------------------------------------------------
    // SimdOps Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_vadd_f32() {
        let ops = SimdOps::new();

        let a = vec![1.0f32; 1024];
        let b = vec![2.0f32; 1024];
        let mut c = vec![0.0f32; 1024];

        ops.vadd_f32(&a, &b, &mut c);

        assert!(c.iter().all(|&x| (x - 3.0).abs() < 1e-6));
    }

    #[test]
    fn test_vadd_f32_odd_size() {
        let ops = SimdOps::new();

        let a = vec![1.0f32; 33]; // Not aligned to any vector width
        let b = vec![2.0f32; 33];
        let mut c = vec![0.0f32; 33];

        ops.vadd_f32(&a, &b, &mut c);

        assert!(c.iter().all(|&x| (x - 3.0).abs() < 1e-6));
    }

    #[test]
    fn test_vadd_f64() {
        let ops = SimdOps::new();

        let a = vec![1.0f64; 256];
        let b = vec![2.0f64; 256];
        let mut c = vec![0.0f64; 256];

        ops.vadd_f64(&a, &b, &mut c);

        assert!(c.iter().all(|&x| (x - 3.0).abs() < 1e-10));
    }

    #[test]
    fn test_vmul_f32() {
        let ops = SimdOps::new();

        let a = vec![2.0f32; 512];
        let b = vec![3.0f32; 512];
        let mut c = vec![0.0f32; 512];

        ops.vmul_f32(&a, &b, &mut c);

        assert!(c.iter().all(|&x| (x - 6.0).abs() < 1e-6));
    }

    #[test]
    fn test_dot_f32() {
        let ops = SimdOps::new();

        let a = vec![1.0f32; 100];
        let b = vec![2.0f32; 100];

        let result = ops.dot_f32(&a, &b);
        assert!((result - 200.0).abs() < 1e-4);
    }

    #[test]
    fn test_dot_f32_varying() {
        let ops = SimdOps::new();

        let a: Vec<f32> = (0..100).map(|i| i as f32).collect();
        let b = vec![1.0f32; 100];

        let result = ops.dot_f32(&a, &b);
        let expected: f32 = (0..100).map(|i| i as f32).sum();
        assert!((result - expected).abs() < 1e-3);
    }

    // ------------------------------------------------------------------------
    // MatrixOps Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_matmul_identity() {
        let ops = MatrixOps::new();

        // 2x2 identity
        let a = vec![1.0, 0.0, 0.0, 1.0];
        let b = vec![1.0, 2.0, 3.0, 4.0];
        let mut c = vec![0.0f32; 4];

        ops.matmul_f32(&a, &b, &mut c, 2, 2, 2);

        assert!((c[0] - 1.0).abs() < 1e-6);
        assert!((c[1] - 2.0).abs() < 1e-6);
        assert!((c[2] - 3.0).abs() < 1e-6);
        assert!((c[3] - 4.0).abs() < 1e-6);
    }

    #[test]
    fn test_matmul_simple() {
        let ops = MatrixOps::new();

        // [1 2]   [5 6]   [19 22]
        // [3 4] x [7 8] = [43 50]
        let a = vec![1.0, 2.0, 3.0, 4.0];
        let b = vec![5.0, 6.0, 7.0, 8.0];
        let mut c = vec![0.0f32; 4];

        ops.matmul_f32(&a, &b, &mut c, 2, 2, 2);

        assert!((c[0] - 19.0).abs() < 1e-5);
        assert!((c[1] - 22.0).abs() < 1e-5);
        assert!((c[2] - 43.0).abs() < 1e-5);
        assert!((c[3] - 50.0).abs() < 1e-5);
    }

    #[test]
    fn test_matmul_larger() {
        let ops = MatrixOps::new();

        let m = 64;
        let k = 64;
        let n = 64;

        let a = vec![1.0f32; m * k];
        let b = vec![1.0f32; k * n];
        let mut c = vec![0.0f32; m * n];

        ops.matmul_f32(&a, &b, &mut c, m, k, n);

        // Each element should be k (sum of 1.0 * 1.0 k times)
        assert!(c.iter().all(|&x| (x - k as f32).abs() < 1e-4));
    }

    #[test]
    fn test_transpose() {
        let ops = MatrixOps::new();

        // [1 2 3]    [1 4]
        // [4 5 6] -> [2 5]
        //            [3 6]
        let src = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let mut dst = vec![0.0f32; 6];

        ops.transpose_f32(&src, &mut dst, 2, 3);

        assert!((dst[0] - 1.0).abs() < 1e-6);
        assert!((dst[1] - 4.0).abs() < 1e-6);
        assert!((dst[2] - 2.0).abs() < 1e-6);
        assert!((dst[3] - 5.0).abs() < 1e-6);
        assert!((dst[4] - 3.0).abs() < 1e-6);
        assert!((dst[5] - 6.0).abs() < 1e-6);
    }
}
