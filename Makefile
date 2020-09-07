# Entrypoint for all the tooling set up

-include Makefile.shared

OCAML_DIR := ocaml-jit
OCAML_STATIC_LIBS := $(OCAML_DIR)/runtime/$(RUST_JIT_DEBUG_LIB) $(OCAML_DIR)/runtime/$(RUST_JIT_RELEASE_LIB)

DEBUG_TARGET := target/debug
RELEASE_TARGET := target/debug

STATIC_LIB_CRATE := ocaml-jit-staticlib
STATIC_LIB_FILE := libocaml_jit_staticlib.a

# Main targets
# ============

.PHONY: all
all:
	make cargo_builds
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

.PHONY: cargo_builds
cargo_builds:
	cd $(STATIC_LIB_CRATE) && cargo build
	cd $(STATIC_LIB_CRATE) && cargo build --release
	cp $(DEBUG_TARGET)/$(STATIC_LIB_FILE) $(OCAML_DIR)/runtime/$(RUST_JIT_DEBUG_LIB)
	cp $(RELEASE_TARGET)/$(STATIC_LIB_FILE) $(OCAML_DIR)/runtime/$(RUST_JIT_RELEASE_LIB)

# Static lib
# ==========
#
# This gets linked in with the ocaml's runtime source

$(OCAML_DIR)/runtime/$(RUST_JIT_DEBUG_LIB): $(DEBUG_TARGET)/$(STATIC_LIB_FILE)
	cp $< $@

$(OCAML_DIR)/runtime/$(RUST_JIT_RELEASE_LIB): $(RELEASE_TARGET)/$(STATIC_LIB_FILE)
	cp $< $@
