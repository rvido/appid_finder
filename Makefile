# App ID Finder - Makefile
#
#   make            build for the host (same as `make host`)
#   make host       release build for this machine
#   make arm64      static ARM64 Linux build (musl), built with `cross`
#   make all        both builds
#   make test       offline tests
#   make lint       cargo fmt --check and clippy
#   make clean      remove build output
#
# `make arm64` needs `cross` (cargo install cross) and Docker or Podman.
# Pick the engine with CROSS_CONTAINER_ENGINE=podman (default: docker).

BIN         := appid_finder
ARM64       := aarch64-unknown-linux-musl
CARGO       ?= cargo
CROSS       ?= cross

HOST_BIN    := target/release/$(BIN)
ARM64_BIN   := target/$(ARM64)/release/$(BIN)

.PHONY: default host arm64 all test lint clean

default: host

host:
	$(CARGO) build --release
	@echo "Built: $(HOST_BIN)"

# musl targets link statically by default; +crt-static makes it explicit.
# CROSS_CUSTOM_TOOLCHAIN=1 stops cross from running `rustup toolchain add`
# (it tries to update the host toolchain). The host needs the target:
#   rustup target add aarch64-unknown-linux-musl
# -A linker_messages hides the cross linker wrapper's debug trace.
arm64:
	CROSS_CUSTOM_TOOLCHAIN=1 \
	RUSTFLAGS="-C target-feature=+crt-static -A linker_messages" \
	$(CROSS) build --release --target $(ARM64)
	@echo "Built: $(ARM64_BIN)"
	@file $(ARM64_BIN) 2>/dev/null || true

all: host arm64

test:
	$(CARGO) test

lint:
	$(CARGO) fmt --check
	$(CARGO) clippy --all-targets

clean:
	$(CARGO) clean
