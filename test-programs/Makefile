-include ../Makefile.toolchain

OCAMLC = $(BUILT_DIR_ABS)/bin/ocamlc

TEST_SOURCES_DIR = src
TEST_COMPILED_DIR = out

TEST_SOURCES = $(wildcard $(TEST_SOURCES_DIR)/*.ml)
TEST_COMPILED = $(patsubst $(TEST_SOURCES_DIR)/%.ml, $(TEST_COMPILED_DIR)/%.byte, $(TEST_SOURCES))

.PHONY: all
all: $(TEST_COMPILED)

.PHONY: clean
clean:
	rm -f test_bc/*.byte

$(TEST_COMPILED_DIR)/%.byte: $(TEST_SOURCES_DIR)/%.ml
	$(OCAMLC) -o $@ $<
