# Entrypoint for all the tooling set up

-include Makefile.Shared

OCAML_DIR := ocaml-jit
OCAML_STATIC_LIBS := $(OCAML_DIR)/runtime/$(RUST_JIT_DEBUG_LIB) $(OCAML_DIR)/runtime/$(RUST_JIT_RELEASE_LIB)

DEBUG_TARGET := target/debug
RELEASE_TARGET := target/debug

STATIC_LIB_CRATE := ocaml-jit-staticlib
STATIC_LIB_FILE := libocaml_jit_staticlib.a

# Main targets
# ============

.PHONY: all
all: ocaml_all

.PHONY: ocaml_all
ocaml_all: $(OCAML_STATIC_LIBS)
	make -C $(OCAML_DIR)

.PHONY: ocamltests
ocamltests:
	make -C $(OCAML_DIR) tests

.PHONY: clean
clean:
	make -C $(OCAML_DIR) clean
	cargo clean

.PHONY: fullclean
fullclean:
	make -C $(OCAML_DIR) distclean
	cargo clean

.PHONY: setup
setup: fullclean
	cd $(OCAML_DIR) && ./configure --enable-rust-jit

# Static lib
# ==========
#
# This gets linked in with the ocaml's runtime source

$(OCAML_DIR)/runtime/$(RUST_JIT_DEBUG_LIB): $(DEBUG_TARGET)/$(STATIC_LIB_FILE)
	cp $< $@

$(OCAML_DIR)/runtime/$(RUST_JIT_RELEASE_LIB): $(RELEASE_TARGET)/$(STATIC_LIB_FILE)
	cp $< $@

$(DEBUG_TARGET)/$(STATIC_LIB_FILE): cargo_debug_build
$(RELEASE_TARGET)/$(STATIC_LIB_FILE): cargo_release_build

.PHONY: cargo_debug_build
cargo_debug_build:
	cd $(STATIC_LIB_CRATE) && cargo build

.PHONY: cargo_release_build
cargo_release_build:
	cd $(STATIC_LIB_CRATE) && cargo build --release