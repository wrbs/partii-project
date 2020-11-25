#!/bin/bash

make run_config_bytecode_and_native.json
make run_config_bytecode.json

BUILD_BENCH_TARGET=buildbench_both RUN_CONFIG_JSON=run_config_bytecode_and_native.json make ocaml-versions/4.10.0+stock.bench

