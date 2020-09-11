# Entrypoint for all the tooling set up

-include Makefile.shared

OCAML_DIR := ocaml-jit
OCAML_STATIC_LIBS := $(OCAML_DIR)/runtime/$(RUST_JIT_DEBUG_LIB) $(OCAML_DIR)/runtime/$(RUST_JIT_RELEASE_LIB)

RUST_DIR := src

DEBUG_TARGET := $(RUST_DIR)/target/debug
RELEASE_TARGET := $(RUST_DIR)/target/debug

STATIC_LIB_CRATE := $(RUST_DIR)/ocaml-jit-staticlib
STATIC_LIB_FILE := libocaml_jit_staticlib.a

BUILT_DIR := dist

RESOURCES_DIR := resources

# Main targets
# ============

.PHONY: only_runtime
runtime_only:
	make cargo_builds
	make -C $(OCAML_DIR)/runtime
	make -C $(OCAML_DIR) install

.PHONY: all
all:
	make cargo_builds
	make -C $(OCAML_DIR)
	make -C $(OCAML_DIR) install
	make -C $(RESOURCES_DIR) all

.PHONY: ocamltests
ocamltests:
	make -C $(OCAML_DIR) tests

.PHONY: clean
clean:
	make -C $(OCAML_DIR) clean
	rm -rf $(BUILT_DIR)
	cd $(RUST_DIR) && cargo clean
	make -C $(RESOURCES_DIR) clean

.PHONY: fullclean
fullclean: clean
	make -C $(OCAML_DIR) distclean

.PHONY: setup
setup: fullclean
	cd $(OCAML_DIR) && ./configure --enable-rust-jit --prefix=$(abspath .)/$(BUILT_DIR)
	echo "BUILT_DIR_ABS=$(abspath .)/$(BUILT_DIR)" > Makefile.toolchain

.PHONY: cargo_builds
cargo_builds:
	cd $(STATIC_LIB_CRATE) && cargo build
	cd $(STATIC_LIB_CRATE) && cargo build --release
	cp $(DEBUG_TARGET)/$(STATIC_LIB_FILE) $(OCAML_DIR)/runtime/$(RUST_JIT_DEBUG_LIB)
	cp $(RELEASE_TARGET)/$(STATIC_LIB_FILE) $(OCAML_DIR)/runtime/$(RUST_JIT_RELEASE_LIB)

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
	cd $(RUST_DIR) && cargo fmt


# Linting
# =======

.PHONY: lint
lint:
	prettier --check .
	cd $(RUST_DIR) && cargo clippy --all


# Static lib
# ==========
#
# This gets linked in with the ocaml's runtime source

$(OCAML_DIR)/runtime/$(RUST_JIT_DEBUG_LIB): $(DEBUG_TARGET)/$(STATIC_LIB_FILE)
	cp $< $@

$(OCAML_DIR)/runtime/$(RUST_JIT_RELEASE_LIB): $(RELEASE_TARGET)/$(STATIC_LIB_FILE)
	cp $< $@
