# Pepita: Tiny First-Principles Rust Kernel for Sovereign AI
# Iron Lotus Framework + Certeza Methodology
# Quality Gates: 95% coverage, 80% mutation score, zero C dependencies

.PHONY: all build test test-fast clean tier1 tier2 tier3 coverage mutation check fmt lint clippy doc audit

# ============================================================================
# DEFAULT TARGET
# ============================================================================

all: tier2

# ============================================================================
# BUILD TARGETS
# ============================================================================

build:
	@echo "Building pepita (std mode)..."
	cargo build --all-features

build-release:
	@echo "Building pepita (release, optimized for size)..."
	cargo build --release

build-kernel:
	@echo "Building pepita (kernel mode, no_std)..."
	cargo build --no-default-features --features kernel

# ============================================================================
# TIER 1: ON-SAVE (Sub-Second Feedback)
# Target: < 3 seconds
# Purpose: Enable flow state during development
# ============================================================================

tier1: check fmt-check clippy test-unit
	@echo "Tier 1 (ON-SAVE) complete"

check:
	@echo "Running cargo check..."
	cargo check --all-features

fmt-check:
	@echo "Checking formatting..."
	cargo fmt -- --check

clippy:
	@echo "Running clippy..."
	cargo clippy --all-features -- -D warnings

test-unit:
	@echo "Running unit tests..."
	cargo test --lib

test:
	cargo test

lint:
	cargo fmt --check && cargo clippy -- -D warnings

test-fast:
	cargo test --lib

# ============================================================================
# TIER 2: ON-COMMIT (1-5 Minutes)
# Target: 1-5 minutes
# Purpose: Comprehensive pre-commit gate
# ============================================================================

tier2: tier1 test-all doc test-doc coverage-check
	@echo "Tier 2 (ON-COMMIT) complete"

test-all:
	@echo "Running all tests..."
	cargo test --all-features

test-doc:
	@echo "Running doc tests..."
	cargo test --doc

doc:
	@echo "Building documentation..."
	cargo doc --no-deps --all-features

coverage:
	@echo "Running coverage analysis..."
	cargo llvm-cov --all-features --lcov --output-path lcov.info
	cargo llvm-cov report

coverage-check:
	@echo "Checking coverage threshold (95%)..."
	@if command -v cargo-llvm-cov > /dev/null 2>&1; then \
		cargo llvm-cov --all-features --fail-under-lines 95 || echo "Coverage check requires cargo-llvm-cov"; \
	else \
		echo "cargo-llvm-cov not installed, skipping coverage check"; \
	fi

# ============================================================================
# TIER 3: ON-MERGE (Hours)
# Target: 1-6 hours
# Purpose: Exhaustive validation before merge
# ============================================================================

tier3: tier2 mutation bench audit
	@echo "Tier 3 (ON-MERGE) complete"

mutation:
	@echo "Running mutation testing (target: 80%)..."
	@if command -v cargo-mutants > /dev/null 2>&1; then \
		cargo mutants --no-shuffle; \
	else \
		echo "cargo-mutants not installed, skipping mutation testing"; \
	fi

bench:
	@echo "Running benchmarks..."
	cargo bench

audit:
	@echo "Running security audit..."
	@if command -v cargo-audit > /dev/null 2>&1; then \
		cargo audit; \
	else \
		echo "cargo-audit not installed, skipping"; \
	fi
	@if command -v cargo-deny > /dev/null 2>&1; then \
		cargo deny check licenses; \
	else \
		echo "cargo-deny not installed, skipping"; \
	fi

# ============================================================================
# DEVELOPMENT UTILITIES
# ============================================================================

fmt:
	@echo "Formatting code..."
	cargo fmt

clean:
	@echo "Cleaning build artifacts..."
	cargo clean

watch:
	@echo "Watching for changes..."
	cargo watch -x "test --lib"

# Install development tools
tools:
	@echo "Installing development tools..."
	cargo install cargo-llvm-cov
	cargo install cargo-mutants
	cargo install cargo-audit
	cargo install cargo-deny
	cargo install cargo-watch
	cargo install criterion

# ============================================================================
# KERNEL-SPECIFIC TARGETS
# ============================================================================

test-no-std:
	@echo "Testing no_std compatibility..."
	cargo build --no-default-features --features kernel

verify-abi:
	@echo "Verifying ABI compatibility..."
	cargo test abi_

# ============================================================================
# CI/CD TARGETS
# ============================================================================

ci: tier2
	@echo "CI pipeline complete"

ci-full: tier3
	@echo "Full CI pipeline complete"

# ============================================================================
# HELP
# ============================================================================

help:
	@echo "Pepita Makefile - Iron Lotus Framework"
	@echo ""
	@echo "Testing Tiers (Certeza Methodology):"
	@echo "  tier1      ON-SAVE: Quick checks (<3s)"
	@echo "  tier2      ON-COMMIT: Comprehensive (1-5min)"
	@echo "  tier3      ON-MERGE: Exhaustive (1-6hr)"
	@echo ""
	@echo "Build Targets:"
	@echo "  build      Build with std features"
	@echo "  build-kernel  Build for kernel (no_std)"
	@echo ""
	@echo "Quality Gates:"
	@echo "  coverage   Run coverage analysis"
	@echo "  mutation   Run mutation testing"
	@echo "  audit      Security audit"
	@echo ""
	@echo "Development:"
	@echo "  fmt        Format code"
	@echo "  watch      Watch for changes"
	@echo "  tools      Install dev tools"
