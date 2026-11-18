APP_NAME ?= ideomancer
BIN_DIR ?= ./bin
INSTALL_DIR ?= $(HOME)/.local/bin
TARGET_DIR ?= target
CARGO ?= cargo
PYTHON ?= python3.12
JOBS ?= 2
TEST_THREADS ?= $(JOBS)
export RUSTUP_AUTO_INSTALL := 0

.DEFAULT_GOAL := build
.PHONY: all build install test check clean cache-status cache-clean

all: build

build:
	$(CARGO) build --release --locked --jobs "$(JOBS)" --target-dir "$(TARGET_DIR)" --bin "$(APP_NAME)"
	mkdir -p "$(BIN_DIR)"
	cp "$(TARGET_DIR)/release/$(APP_NAME)" "$(BIN_DIR)/$(APP_NAME)"

install: build
	mkdir -p "$(INSTALL_DIR)"
	cp "$(BIN_DIR)/$(APP_NAME)" "$(INSTALL_DIR)/$(APP_NAME)"

test:
	$(CARGO) test --locked --offline --jobs "$(JOBS)" --target-dir "$(TARGET_DIR)" -- --test-threads="$(TEST_THREADS)"

check:
	$(CARGO) fmt --check
	$(CARGO) clippy --locked --offline --jobs "$(JOBS)" --target-dir "$(TARGET_DIR)" --all-targets -- -D warnings

# Cache-only feature closeout: fixed project target, no bin/install/global cleanup.
cache-status:
	$(PYTHON) -B scripts/feature_cache.py

cache-clean:
	$(PYTHON) -B scripts/feature_cache.py --apply

clean:
	$(CARGO) clean --target-dir "$(TARGET_DIR)"
	rm -f -- "$(BIN_DIR)/$(APP_NAME)"
