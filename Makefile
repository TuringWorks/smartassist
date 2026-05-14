# SmartAssist Makefile
# Build system for the Rust workspace, Tauri desktop, mobile apps, and tests.
#
# Usage:
#   make help              Show all available targets
#   make build             Build the full Rust workspace (debug)
#   make build-release     Build optimized release binaries
#   make test-all          Run unit, integration, BDD, e2e, and architecture tests
#   make lint              Run fmt --check and clippy across the workspace
#   make coverage          Generate HTML/XML coverage reports
#   make tauri-build       Build the desktop control panel (current platform)
#   make android-build     Build the Android companion app
#   make ios-build         Build the iOS companion app
#
# Environment variables:
#   CARGO                  Cargo executable (default: cargo)
#   CARGO_FLAGS            Extra flags passed to cargo (default: empty)
#   FEATURES               Comma-separated features to enable (default: empty)
#   JOBS                   Number of parallel jobs (default: auto)
#   TAPLO                  Path to taplo for TOML formatting (optional)
#   DOCKER                 Docker executable (default: docker)
#   DOCKER_COMPOSE         Docker Compose executable (default: docker compose)
#   GRADLE                 Gradle executable for Android (default: gradle)
#   SWIFT                  Swift executable for iOS (default: swift)
#   NPM                    Node package manager (default: npm)
#   NPX                    NPX executable (default: npx)
#   CARGO_TAURI            Tauri CLI via cargo (default: cargo tauri)

CARGO        ?= cargo
DOCKER       ?= docker
DOCKER_COMPOSE ?= docker compose
GRADLE       ?= gradle
SWIFT        ?= swift
NPM          ?= npm
NPX          ?= npx
CARGO_TAURI  ?= $(CARGO) tauri

ROOT_DIR     := $(dir $(abspath $(lastword $(MAKEFILE_LIST))))
CONTROL_DIR  := $(ROOT_DIR)smartassist-control
ANDROID_DIR  := $(ROOT_DIR)apps/android
IOS_DIR      := $(ROOT_DIR)apps/ios/SmartAssist

# Detect OS for Tauri builds
UNAME_S := $(shell uname -s)
ifeq ($(UNAME_S),Darwin)
  TAURI_TARGET := universal-apple-darwin
endif
ifeq ($(UNAME_S),Linux)
  TAURI_TARGET := x86_64-unknown-linux-gnu
endif

# Parallel jobs
ifdef JOBS
  CARGO_JOBS := -j$(JOBS)
else
  CARGO_JOBS :=
endif

# Feature flags
ifdef FEATURES
  CARGO_FEATURES := --features $(FEATURES)
else
  CARGO_FEATURES :=
endif

# ---------------------------------------------------------------------------
# Help
# ---------------------------------------------------------------------------

.PHONY: help
help:
	@echo "SmartAssist Build System"
	@echo "========================"
	@echo ""
	@echo "Rust Workspace"
	@echo "  build                Build debug binaries for the full workspace"
	@echo "  build-release        Build release binaries"
	@echo "  build-gateway        Build only the gateway binary"
	@echo "  build-cli            Build only the CLI binary"
	@echo "  build-all-features   Build with every Cargo feature enabled"
	@echo ""
	@echo "Tests"
	@echo "  test                 Run unit tests"
	@echo "  test-integration     Run integration tests"
	@echo "  test-bdd             Run BDD (cucumber) tests"
	@echo "  test-e2e             Run end-to-end smoke tests"
	@echo "  test-arch            Run architecture boundary tests"
	@echo "  test-all             Run unit + integration + BDD + e2e + arch tests"
	@echo ""
	@echo "Code Quality"
	@echo "  fmt                  Run rustfmt on all source files"
	@echo "  fmt-check            Check formatting without modifying files"
	@echo "  clippy               Run Clippy lints across the workspace"
	@echo "  lint                 Run fmt-check + clippy"
	@echo ""
	@echo "Coverage"
	@echo "  coverage             Generate coverage with cargo-tarpaulin"
	@echo "  coverage-open        Open the HTML coverage report"
	@echo ""
	@echo "Desktop (Tauri)"
	@echo "  tauri-dev            Start the Tauri control panel in dev mode"
	@echo "  tauri-build          Build the desktop app for the current platform"
	@echo "  tauri-build-mac      Build macOS desktop app (universal binary)"
	@echo "  tauri-build-linux    Build Linux desktop app"
	@echo "  tauri-build-windows  Build Windows desktop app"
	@echo ""
	@echo "Mobile"
	@echo "  android-build        Build Android APK"
	@echo "  android-test         Run Android unit tests"
	@echo "  ios-build            Build iOS Swift package"
	@echo "  ios-test             Run iOS Swift tests"
	@echo ""
	@echo "Docker"
	@echo "  docker-build         Build the SmartAssist Docker image"
	@echo "  docker-compose-up    Start the full Docker stack"
	@echo "  docker-compose-down  Stop the full Docker stack"
	@echo ""
	@echo "Run / Deploy"
	@echo "  run-gateway          Run the gateway server (debug)"
	@echo "  run-cli              Run the CLI in REPL mode"
	@echo "  run-tui              Run the terminal UI"
	@echo "  run-doctor           Run the doctor health check"
	@echo "  install              Install binaries locally with cargo install"
	@echo ""
	@echo "Maintenance"
	@echo "  clean                Run cargo clean"
	@echo "  clean-all            cargo clean + remove node_modules + mobile build artifacts"
	@echo "  update               Update Cargo.lock and npm dependencies"

# ---------------------------------------------------------------------------
# Rust Workspace
# ---------------------------------------------------------------------------

.PHONY: build build-release build-gateway build-cli build-all-features

build:
	$(CARGO) build $(CARGO_JOBS) $(CARGO_FEATURES)

build-release:
	$(CARGO) build --release $(CARGO_JOBS) $(CARGO_FEATURES)

build-gateway:
	$(CARGO) build --bin smartassist-gateway --release $(CARGO_JOBS) $(CARGO_FEATURES)

build-cli:
	$(CARGO) build --bin smartassist --release $(CARGO_JOBS) $(CARGO_FEATURES)

build-all-features:
	$(CARGO) build --workspace --all-features $(CARGO_JOBS)

# ---------------------------------------------------------------------------
# Tests
# ---------------------------------------------------------------------------

.PHONY: test test-integration test-bdd test-e2e test-arch test-all

test:
	$(CARGO) test --workspace $(CARGO_JOBS) $(CARGO_FEATURES)

test-integration:
	$(CARGO) test --test integration --workspace $(CARGO_JOBS) $(CARGO_FEATURES)

test-bdd:
	$(CARGO) test --test cucumber -p smartassist-bdd-tests $(CARGO_JOBS)

test-e2e:
	$(CARGO) test --test e2e -p smartassist-e2e-tests $(CARGO_JOBS)

test-arch:
	$(CARGO) test --test architecture -p smartassist-integration-tests $(CARGO_JOBS)

test-all: test test-integration test-bdd test-e2e test-arch

# ---------------------------------------------------------------------------
# Code Quality
# ---------------------------------------------------------------------------

.PHONY: fmt fmt-check clippy lint

fmt:
	$(CARGO) fmt --all

fmt-check:
	$(CARGO) fmt --all -- --check

clippy:
	$(CARGO) clippy --workspace --all-targets $(CARGO_FEATURES) -- -D warnings

lint: fmt-check clippy

# ---------------------------------------------------------------------------
# Coverage
# ---------------------------------------------------------------------------

.PHONY: coverage coverage-open

coverage:
	$(CARGO) tarpaulin --config tarpaulin.toml $(CARGO_JOBS)

coverage-open:
	open $(ROOT_DIR)coverage/tarpaulin-report.html 2>/dev/null || \
	xdg-open $(ROOT_DIR)coverage/tarpaulin-report.html 2>/dev/null || \
	echo "Open coverage/tarpaulin-report.html manually"

# ---------------------------------------------------------------------------
# Desktop (Tauri)
# ---------------------------------------------------------------------------

.PHONY: tauri-dev tauri-build tauri-build-mac tauri-build-linux tauri-build-windows

tauri-dev:
	cd $(CONTROL_DIR) && $(NPM) run tauri dev

tauri-build:
	cd $(CONTROL_DIR) && $(NPM) run tauri build

tauri-build-mac:
	cd $(CONTROL_DIR) && $(NPM) run tauri build -- --target $(TAURI_TARGET)

tauri-build-linux:
	cd $(CONTROL_DIR) && $(NPM) run tauri build -- --target x86_64-unknown-linux-gnu

tauri-build-windows:
	cd $(CONTROL_DIR) && $(NPM) run tauri build -- --target x86_64-pc-windows-msvc

# ---------------------------------------------------------------------------
# Mobile
# ---------------------------------------------------------------------------

.PHONY: android-build android-test ios-build ios-test

android-build:
	cd $(ANDROID_DIR) && $(GRADLE) assembleDebug

android-test:
	cd $(ANDROID_DIR) && $(GRADLE) test

ios-build:
	cd $(IOS_DIR) && $(SWIFT) build

ios-test:
	cd $(IOS_DIR) && $(SWIFT) test

# ---------------------------------------------------------------------------
# Docker
# ---------------------------------------------------------------------------

.PHONY: docker-build docker-compose-up docker-compose-down

docker-build:
	$(DOCKER) build -t smartassist:latest -f $(ROOT_DIR)Dockerfile $(ROOT_DIR)

docker-compose-up:
	$(DOCKER_COMPOSE) -f $(ROOT_DIR)docker-compose.yml up -d --build

docker-compose-down:
	$(DOCKER_COMPOSE) -f $(ROOT_DIR)docker-compose.yml down

# ---------------------------------------------------------------------------
# Run / Deploy
# ---------------------------------------------------------------------------

.PHONY: run-gateway run-cli run-tui run-doctor install

run-gateway:
	$(CARGO) run --bin smartassist-gateway $(CARGO_FEATURES)

run-cli:
	$(CARGO) run --bin smartassist $(CARGO_FEATURES)

run-tui:
	$(CARGO) run --bin smartassist -- tui $(CARGO_FEATURES)

run-doctor:
	$(CARGO) run --bin smartassist -- doctor --full $(CARGO_FEATURES)

install:
	$(CARGO) install --path $(ROOT_DIR)crates/smartassist-cli
	$(CARGO) install --path $(ROOT_DIR)crates/smartassist-gateway

# ---------------------------------------------------------------------------
# Maintenance
# ---------------------------------------------------------------------------

.PHONY: clean clean-all update

clean:
	$(CARGO) clean

clean-all: clean
	cd $(CONTROL_DIR) && rm -rf node_modules dist
	cd $(ANDROID_DIR) && rm -rf app/build .gradle
	cd $(IOS_DIR) && rm -rf .build
	$(DOCKER) system prune -f

update:
	$(CARGO) update
	cd $(CONTROL_DIR) && $(NPM) update
