#!/bin/bash

cd "$(dirname "${BASH_SOURCE[0]}")/.."

set -euxo pipefail

source ./toolchain.env

make

PROGRAM=$1
OUT_DIR="/tmp/graphs/${PROGRAM}"

shift 1

mkdir -p "${OUT_DIR}"

cd ${RUST_DIR}

cargo run clever-dis --dot "${OUT_DIR}" ${TEST_PROGRAMS_DIR}/out/${PROGRAM}.byte

$BROWSER "${OUT_DIR}/root.svg"