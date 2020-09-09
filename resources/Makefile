-include ../Makefile.toolchain

OCAMLC = $(BUILT_DIR_ABS)/bin/ocamlc

TEST_SOURCES_DIR = test_sources
TEST_COMPILED_DIR = test_bc

TEST_SOURCES = $(wildcard $(TEST_SOURCES_DIR)/*.ml)
TEST_CASES = $(patsubst $(TEST_SOURCES_DIR)/%.ml, $(TEST_COMPILED_DIR)/%.byte, $(TEST_SOURCES))

.PHONY: all
all: $(TEST_CASES)

.PHONY: clean
clean:
	rm test_bc/*.byte

$(TEST_COMPILED_DIR)/%.byte: $(TEST_SOURCES_DIR)/%.ml $(OCAMLC)
	$(OCAMLC) -o $@ $<