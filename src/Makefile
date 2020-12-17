# Need to supply this when doing make setup
PREFIX :=

OCAML_DIR := ocaml

RUST_DIR := rust

DEBUG_TARGET := $(RUST_DIR)/target/debug
RELEASE_TARGET := $(RUST_DIR)/target/release

STATIC_LIB_FILE := libocaml_jit_staticlib.a
RELEASE_COPIED := $(OCAML_DIR)/runtime/libocamljit.a

.PHONY: all
all:
	$(MAKE) cargo_builds
	$(MAKE) $(RELEASE_COPIED)
	$(MAKE) -C $(OCAML_DIR)

.PHONY: setup
setup:
ifeq "$(PREFIX)" ""
	$(error No prefix set)
endif
	@echo $(PREFIX)
	cd $(OCAML_DIR) && ./configure --enable-rust-jit --prefix=$(PREFIX)

.PHONY: install
install:
	mkdir -p $(BUILT_DIR)/bin
	$(MAKE) -C $(OCAML_DIR) install

.PHONY: only_runtime
only_runtime:
	$(MAKE) cargo_builds
	$(MAKE) $(RELEASE_COPIED)
	$(MAKE) -C $(OCAML_DIR)/runtime

.PHONY: tests
tests:
	$(MAKE) -C $(OCAML_DIR) tests

.PHONY: cargo_builds
cargo_builds:
	cd $(RUST_DIR) && cargo build --all --release

$(RELEASE_COPIED): $(RELEASE_TARGET)/$(STATIC_LIB_FILE)
	cp $? $@

.PHONY: clean
clean:
	$(MAKE) -C $(OCAML_DIR) clean
	cd $(RUST_DIR) && cargo clean

.PHONY: distclean
distclean: clean
	$(MAKE) -C $(OCAML_DIR) distclean

