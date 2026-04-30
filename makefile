APP_NAME=ide
CMD_PATH = ./cmd/ideomancer/
BIN_DIR = ./bin
INSTALL_DIR = ~/.local/bin

.PHONY: all build

all: build install


build:
	@echo "🔨 Building $(APP_NAME)..."
	@mkdir -p $(BIN_DIR)
	go build -o $(BIN_DIR)/$(APP_NAME) $(CMD_PATH)
	@echo "✅ Built at $(BIN_DIR)/$(APP_NAME)"
	
install:
	@echo "📦 Installing to $(INSTALL_DIR)/$(APP_NAME)"
	@mkdir -p $(INSTALL_DIR)
	cp $(BIN_DIR)/$(APP_NAME) $(INSTALL_DIR)/$(APP_NAME)
	@echo "✅ Installed. Run with: $(APP_NAME)"

clean:
	@echo " Cleaning build output..."
	rm -rf $(BIN_BIN_DIR)
