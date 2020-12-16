# Entrypoint for all the tooling set up

SRC_DIR := src
OCAML_DIR := $(SRC_DIR)/ocaml
RUST_DIR := $(SRC_DIR)/rust

TEST_PROGRAMS_DIR := test-programs

NO_ASLR_DIR := vendor/no-aslr

BUILT_DIR := dist

ROOT_DIR_ABS := $(abspath .)
PREFIX := $(ROOT_DIR_ABS)/$(BUILT_DIR)

TOOLCHAIN_FILE := toolchain.env

# Main targets
# ============

.PHONY: only_runtime
runtime_only:
	$(MAKE) -C $(SRC_DIR) only_runtime
	$(MAKE) install
	$(MAKE) -C $(TEST_PROGRAMS_DIR) all
	$(MAKE) -C $(NO_ASLR_DIR)

.PHONY: all
all:
	$(MAKE) -C $(SRC_DIR) all
	$(MAKE) install
	$(MAKE) -C $(TEST_PROGRAMS_DIR) all
	$(MAKE) -C $(NO_ASLR_DIR)

.PHONY: install
install:
	$(MAKE) -C $(SRC_DIR) install
	cp $(RUST_DIR)/target/release/ocaml-jit-tools $(BUILT_DIR)/bin

.PHONY: tests
tests:
	$(MAKE) -C $(SRC_DIR) tests

.PHONY: clean
clean:
	$(MAKE) -C $(SRC_DIR) clean
	rm -rf $(BUILT_DIR)
	$(MAKE) -C $(TEST_PROGRAMS_DIR) clean
	$(MAKE) -C $(NO_ASLR_DIR) clean

.PHONY: distclean
distclean: clean
	$(MAKE) -C $(SRC_DIR) distclean
	rm -f $(TOOLCHAIN_FILE)

.PHONY: setup
setup: distclean $(TOOLCHAIN_FILE)
	$(MAKE) -C $(SRC_DIR) setup PREFIX=$(PREFIX)

.PHONY: $(TOOLCHAIN_FILE)
$(TOOLCHAIN_FILE):
	echo "BUILT_DIR=$(PREFIX)" > $(TOOLCHAIN_FILE)
	echo "RUST_DIR=$(ROOT_DIR_ABS)/$(RUST_DIR)" >> $(TOOLCHAIN_FILE)
	echo "OCAML_DIR=$(ROOT_DIR_ABS)/$(OCAML_DIR)" >> $(TOOLCHAIN_FILE)
	echo "TEST_PROGRAMS_DIR=$(ROOT_DIR_ABS)/$(TEST_PROGRAMS_DIR)" >> $(TOOLCHAIN_FILE)
	echo "NO_ASLR_DIR=$(ROOT_DIR_ABS)/$(NO_ASLR_DIR)" >> $(TOOLCHAIN_FILE)

# Autoformatting
# ==============

.PHONY: format
format: prettier rustfmt

.PHONY: prettier
prettier:
	if command -v prettier &>/dev/null; then \
		prettier -w .; \
	else \
		echo Prettier not found, install it with npm; \
	fi

.PHONY: rustfmt
rustfmt:
	cd $(RUST_DIR) && cargo fmt --all


# Linting
# =======

.PHONY: lint
lint:
	prettier --check .
	cd $(RUST_DIR) && cargo clippy --all
