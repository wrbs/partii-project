# Entrypoint for all the tooling set up

-include Makefile.shared

OCAML_DIR := src/ocaml
RELEASE_COPIED := $(OCAML_DIR)/runtime/$(RUST_JIT_RELEASE_LIB)

RUST_DIR := src/rust

DEBUG_TARGET := $(RUST_DIR)/target/debug
RELEASE_TARGET := $(RUST_DIR)/target/debug

STATIC_LIB_FILE := libocaml_jit_staticlib.a

TEST_PROGRAMS_DIR := test-programs

NO_ASLR_DIR := vendor/no-aslr

BUILT_DIR := dist
ROOT_DIR_ABS := $(abspath .)
PREFIX := $(ROOT_DIR_ABS)/$(BUILT_DIR)

OPAM_PREFIX :=

TOOLCHAIN_FILE := toolchain.env

# Main targets
# ============

.PHONY: only_runtime
runtime_only:
	$(MAKE) cargo_builds
	$(MAKE) $(RELEASE_COPIED)
	$(MAKE) -C $(OCAML_DIR)/runtime
	$(MAKE) -C $(OCAML_DIR) install
	$(MAKE) -C $(TEST_PROGRAMS_DIR) all
	$(MAKE) -C $(NO_ASLR_DIR)

.PHONY: all
all:
	$(MAKE) cargo_builds
	$(MAKE) $(RELEASE_COPIED)
	$(MAKE) -C $(OCAML_DIR)
	$(MAKE) -C $(OCAML_DIR) install
	$(MAKE) -C $(TEST_PROGRAMS_DIR) all
	$(MAKE) -C $(NO_ASLR_DIR)

.PHONY: ocamltests
ocamltests:
	$(MAKE) -C $(OCAML_DIR) tests

.PHONY: clean
clean:
	$(MAKE) -C $(OCAML_DIR) clean
	rm -rf $(BUILT_DIR)
	cd $(RUST_DIR) && cargo clean
	$(MAKE) -C $(TEST_PROGRAMS_DIR) clean
	rm -f $(RELEASE_COPIED)

.PHONY: fullclean
fullclean: clean
	$(MAKE) -C $(OCAML_DIR) distclean
	rm -f $(TOOLCHAIN_FILE)

.PHONY: setup
setup: fullclean $(TOOLCHAIN_FILE)
	@echo $(PREFIX)
	cd $(OCAML_DIR) && ./configure --enable-rust-jit --prefix=$(PREFIX)

.PHONY: $(TOOLCHAIN_FILE)
$(TOOLCHAIN_FILE):
	echo "BUILT_DIR=$(PREFIX)" > $(TOOLCHAIN_FILE)
	echo "RUST_DIR=$(ROOT_DIR_ABS)/$(RUST_DIR)" >> $(TOOLCHAIN_FILE)
	echo "OCAML_DIR=$(ROOT_DIR_ABS)/$(OCAML_DIR)" >> $(TOOLCHAIN_FILE)
	echo "TEST_PROGRAMS_DIR=$(ROOT_DIR_ABS)/$(TEST_PROGRAMS_DIR)" >> $(TOOLCHAIN_FILE)
	echo "NO_ASLR_DIR=$(ROOT_DIR_ABS)/$(NO_ASLR_DIR)" >> $(TOOLCHAIN_FILE)

.PHONY: cargo_builds
cargo_builds:
	cd $(RUST_DIR) && cargo build --all
	cd $(RUST_DIR) && cargo build --all --release
	mkdir -p $(BUILT_DIR)/bin
	cp $(RELEASE_TARGET)/ocaml-jit-tools $(BUILT_DIR)/bin/

.PHONY: opam_build
opam_build:
	$(MAKE) fullclean
	cd $(OCAML_DIR) && ./configure --enable-rust-jit --prefix=$(OPAM_PREFIX)
	$(MAKE) cargo_builds
	$(MAKE) $(RELEASE_COPIED)
	$(MAKE) -C $(OCAML_DIR)

.PHONY: opam_install
opam_install:
	$(MAKE) -C $(OCAML_DIR) install

# Copying built libs into the OCaml compiler's tree
# =================================================

$(RELEASE_COPIED): $(RELEASE_TARGET)/$(STATIC_LIB_FILE)
	cp $? $@

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
