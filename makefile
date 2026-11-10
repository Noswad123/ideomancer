APP_NAME ?= ideomancer
BIN_DIR ?= ./bin
INSTALL_DIR ?= $(HOME)/.local/bin
TARGET_DIR ?= target
CARGO ?= cargo

.DEFAULT_GOAL := build
.PHONY: all build install test check clean

all: build

build:
	$(CARGO) build --release --locked --target-dir "$(TARGET_DIR)" --bin "$(APP_NAME)"
	mkdir -p "$(BIN_DIR)"
	cp "$(TARGET_DIR)/release/$(APP_NAME)" "$(BIN_DIR)/$(APP_NAME)"

install: build
	mkdir -p "$(INSTALL_DIR)"
	cp "$(BIN_DIR)/$(APP_NAME)" "$(INSTALL_DIR)/$(APP_NAME)"

test:
	$(CARGO) test --locked --target-dir "$(TARGET_DIR)"

check:
	$(CARGO) fmt --check
	$(CARGO) clippy --locked --target-dir "$(TARGET_DIR)" --all-targets -- -D warnings

clean:
	$(CARGO) clean --target-dir "$(TARGET_DIR)"
	rm -f -- "$(BIN_DIR)/$(APP_NAME)"
