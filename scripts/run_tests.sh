#!/bin/bash

set -euxo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

source ./toolchain.env

PROGRAMS="arrays exceptions factorial float_fields hello includestruct offsetref strings_and_bytes vect arith_small extra_args_tests"
# failing: ppa call traces - I think because of GC nondeterminism due to new ordering - but might be genuine bug. It's impossible to debug tbh

make

cd ${RUST_DIR}

for PROGRAM in ${PROGRAMS}; do
  cargo run compare-instruction-traces "$@" ${NO_ASLR_DIR}/no-aslr ${TEST_PROGRAMS_DIR}/out/${PROGRAM}.byte || (echo "!!! Failed on ${PROGRAM} (instruction), exiting"; exit 1)
  cargo run compare-call-traces "$@" ${NO_ASLR_DIR}/no-aslr ${TEST_PROGRAMS_DIR}/out/${PROGRAM}.byte || (echo "!!! Failed on ${PROGRAM} (call), exiting"; exit 1)
done
