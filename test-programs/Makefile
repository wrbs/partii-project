-include ../toolchain.env

OCAMLC = $(BUILT_DIR)/bin/ocamlc
OCAML_JIT_TOOLS = $(BUILT_DIR)/bin/ocaml-jit-tools

SRC_DIR = src
OUT_DIR = out
BUILD_DIR = build

TEST_SOURCES = $(wildcard $(SRC_DIR)/*.ml)
TEST_COMPILED = $(patsubst $(SRC_DIR)/%.ml, $(OUT_DIR)/%.byte, $(TEST_SOURCES))
TEST_DIS = $(patsubst $(SRC_DIR)/%.ml, $(OUT_DIR)/%.dis, $(TEST_SOURCES))

.PHONY: all
all: $(TEST_COMPILED) $(TEST_DIS)

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
	$(OCAMLC) -g -o $@ $<

$(OUT_DIR)/%.byte: $(BUILD_DIR)/%.byte | dirs
	cp $< $@

$(BUILD_DIR)/%.dumpobj: $(OUT_DIR)/%.byte
	$(OCAML_DIR)/tools/dumpobj $< > $@

$(OUT_DIR)/%.dis: $(BUILD_DIR)/%.byte $(BUILD_DIR)/%.dumpobj
	$(OCAML_JIT_TOOLS) process-disassembly --bytecode-file $< --dumpobj-output $(word 2,$^) > $@