#!/bin/bash

cd "$(dirname "${BASH_SOURCE[0]}")/.."

PROGRAMS="arrays exceptions factorial float_fields hello includestruct offsetref ppa strings_and_bytes vect arith_small"

make

cd src

for PROGRAM in ${PROGRAMS}; do
  cargo run compare-traces "$@" ../vendor/no-aslr/no-aslr ../test-programs/out/${PROGRAM}.byte || (echo "!!! Failed on ${PROGRAM}, exiting"; exit 1)
done
