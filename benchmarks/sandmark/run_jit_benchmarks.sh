#!/bin/bash

set -e

make run_config_macro_bytecode_and_native.json
make run_config_macro_bytecode.json

export CFLAGS="-fno-omit-frame-pointer"

BUILD_BENCH_TARGET=buildbench_both RUN_CONFIG_JSON=run_config_macro_bytecode.json make ocaml-versions/4.11.1+jit+cranelift50.bench
BUILD_BENCH_TARGET=buildbench_both RUN_CONFIG_JSON=run_config_macro_bytecode.json make ocaml-versions/4.11.1+jit.bench
BUILD_BENCH_TARGET=buildbench_both RUN_CONFIG_JSON=run_config_macro_bytecode_and_native.json make ocaml-versions/4.11.1+stock.bench

