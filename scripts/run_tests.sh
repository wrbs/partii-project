#!/bin/bash

set -euxo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

source ./toolchain.env

PROGRAMS="arrays exceptions factorial float_fields hello includestruct offsetref ppa strings_and_bytes vect arith_small extra_args_tests"

make

cd ${RUST_DIR}

for PROGRAM in ${PROGRAMS}; do
  cargo run compare-traces "$@" ${NO_ASLR_DIR}/no-aslr ${TEST_PROGRAMS_DIR}/out/${PROGRAM}.byte || (echo "!!! Failed on ${PROGRAM}, exiting"; exit 1)
done
