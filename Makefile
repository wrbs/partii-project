# Entrypoint for all the tooling set up

-include Makefile.shared

OCAML_DIR := ocaml-jit
DEBUG_COPIED := $(OCAML_DIR)/runtime/$(RUST_JIT_DEBUG_LIB)
RELEASE_COPIED := $(OCAML_DIR)/runtime/$(RUST_JIT_RELEASE_LIB)
OCAML_STATIC_LIBS := $(DEBUG_COPIED) $(RELEASE_COPIED)

RUST_DIR := src

DEBUG_TARGET := $(RUST_DIR)/target/debug
RELEASE_TARGET := $(RUST_DIR)/target/debug

STATIC_LIB_FILE := libocaml_jit_staticlib.a

RESOURCES_DIR := test-programs

NO_ASLR_DIR := vendor/no-aslr

BUILT_DIR := dist
PREFIX := $(abspath .)/$(BUILT_DIR)

# Main targets
# ============

.PHONY: only_runtime
runtime_only:
	$(MAKE) cargo_builds
	$(MAKE) $(OCAML_STATIC_LIBS)
	$(MAKE) -C $(OCAML_DIR)/runtime
	$(MAKE) -C $(OCAML_DIR) install
	$(MAKE) -C $(RESOURCES_DIR) all
	$(MAKE) -C $(NO_ASLR_DIR)

.PHONY: all
all:
	$(MAKE) cargo_builds
	$(MAKE) $(OCAML_STATIC_LIBS)
	$(MAKE) -C $(OCAML_DIR)
	$(MAKE) -C $(OCAML_DIR) install
	$(MAKE) -C $(RESOURCES_DIR) all
	$(MAKE) -C $(NO_ASLR_DIR)

.PHONY: ocamltests
ocamltests:
	$(MAKE) -C $(OCAML_DIR) tests

.PHONY: clean
clean:
	$(MAKE) -C $(OCAML_DIR) clean
	rm -rf $(BUILT_DIR)
	cd $(RUST_DIR) && cargo clean
	$(MAKE) -C $(RESOURCES_DIR) clean

.PHONY: fullclean
fullclean: clean
	$(MAKE) -C $(OCAML_DIR) distclean

.PHONY: setup
setup: fullclean
	@echo $(PREFIX)
	cd $(OCAML_DIR) && ./configure --enable-rust-jit --prefix=$(PREFIX)
	echo "BUILT_DIR_ABS=$(abspath .)/$(BUILT_DIR)" > Makefile.toolchain


.PHONY: cargo_builds
cargo_builds:
	cd $(RUST_DIR) && cargo build --all
	cd $(RUST_DIR) && cargo build --all --release
	mkdir -p $(BUILT_DIR)/bin
	cp $(RELEASE_TARGET)/ocaml-jit-tools $(BUILT_DIR)/bin/

# Copying built libs into the OCaml compiler's tree
# =================================================

$(DEBUG_COPIED): $(DEBUG_TARGET)/$(STATIC_LIB_FILE)
	cp $? $@

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
