-include ../Makefile.toolchain

OCAMLC = $(BUILT_DIR_ABS)/bin/ocamlc

SRC_DIR = src
OUT_DIR = out
BUILD_DIR = build

TEST_SOURCES = $(wildcard $(SRC_DIR)/*.ml)
TEST_COMPILED = $(patsubst $(SRC_DIR)/%.ml, $(OUT_DIR)/%.byte, $(TEST_SOURCES))

.PHONY: all
all: $(TEST_COMPILED)

.PHONY: clean
clean:
	rm -rf $(BUILD_DIR)
	rm -rf $(OUT_DIR)

.PHONY: dirs
dirs:
	mkdir -p $(BUILD_DIR)
	mkdir -p $(OUT_DIR)

$(BUILD_DIR)/%.ml: $(SRC_DIR)/%.ml | dirs
	cp $< $@

$(BUILD_DIR)/%.byte: $(BUILD_DIR)/%.ml
	$(OCAMLC) -o $@ $<

$(OUT_DIR)/%.byte: $(BUILD_DIR)/%.byte | dirs
	cp $< $@
