#!/bin/bash

cd "$(dirname "${BASH_SOURCE[0]}")/.."

set -euxo pipefail

source ./toolchain.env

make

PROGRAM=$1
shift 1

cd ${RUST_DIR}

cargo run compare-instruction-traces "$@" ${NO_ASLR_DIR}/no-aslr ${TEST_PROGRAMS_DIR}/out/${PROGRAM}.byte
