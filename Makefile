# ==============================================================================
# Makefile for BashMenu-RS
# ==============================================================================

.PHONY: all help check test fmt fmt-fix clippy shellcheck verify build release install clean locale-check

all: help

help:
	@echo "BashMenu-RS Developer Task Runner"
	@echo "================================="
	@echo "  make check        - Run formatting, clippy, shellcheck, and locale checks"
	@echo "  make verify       - Run full pre-commit verification suite (fmt, clippy, tests, shellcheck, locales)"
	@echo "  make test         - Run all workspace unit and integration tests"
	@echo "  make fmt          - Check formatting (or 'make fmt-fix' to format in-place)"
	@echo "  make clippy       - Run clippy linter with '-D warnings'"
	@echo "  make shellcheck   - Run shellcheck on all shell scripts"
	@echo "  make locale-check - Validate all locale YAML files for 100% key parity"
	@echo "  make build        - Compile all workspace binaries in debug mode"
	@echo "  make release      - Compile all workspace binaries in release mode"
	@echo "  make install      - Run install.sh to install binaries to ~/.local/bin"
	@echo "  make clean        - Clean Cargo build artifacts"

fmt:
	cargo fmt --all --check

fmt-fix:
	cargo fmt --all

clippy:
	cargo clippy --workspace --all-targets -- -D warnings

shellcheck:
	shellcheck *.sh scripts/*.sh

test:
	cargo test --workspace

locale-check:
	cargo run -p ymlcheck-cli -- -l locales/*.yml

check: fmt clippy shellcheck locale-check

verify: fmt clippy shellcheck locale-check test
	@echo "All pre-commit verification checks passed successfully!"

build:
	cargo build --workspace

release:
	cargo build --workspace --release

install:
	./install.sh

clean:
	cargo clean
