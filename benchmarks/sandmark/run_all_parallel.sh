#!/bin/bash

# If using RUN_BENCH_TARGET=run_orunchrt the parallel benchmarks
# use `chrt -r 1`. You may need to setup permissions to allow the
# user to execute `chrt`. For example, this could be done with:
#   sudo setcap cap_sys_nice=ep /usr/bin/chrt
#

make multicore_parallel_run_config_macro.json

RUN_BENCH_TARGET=run_pausetimes_multicore BUILD_BENCH_TARGET=multibench_parallel \
	RUN_CONFIG_JSON=multicore_parallel_run_config_macro.json \
	make ocaml-versions/4.06.1+multicore+pausetimes+parallel.bench
RUN_BENCH_TARGET=run_pausetimes_multicore BUILD_BENCH_TARGET=multibench_parallel \
	RUN_CONFIG_JSON=multicore_parallel_run_config_macro.json \
	make ocaml-versions/4.06.1+multicore+stw+pausetimes+parallel.bench
RUN_BENCH_TARGET=run_pausetimes_multicore BUILD_BENCH_TARGET=multibench_parallel \
	RUN_CONFIG_JSON=multicore_parallel_run_config_macro.json \
	make ocaml-versions/4.10.0+multicore+pausetimes+parallel.bench

BUILD_BENCH_TARGET=multibench_parallel \
	RUN_CONFIG_JSON=multicore_parallel_run_config_macro.json \
	make ocaml-versions/4.06.1+multicore+parallel.bench
BUILD_BENCH_TARGET=multibench_parallel \
	RUN_CONFIG_JSON=multicore_parallel_run_config_macro.json \
	make ocaml-versions/4.06.1+multicore+stw+parallel.bench
BUILD_BENCH_TARGET=multibench_parallel \
	RUN_CONFIG_JSON=multicore_parallel_run_config_macro.json \
	make ocaml-versions/4.10.0+multicore+parallel.bench
