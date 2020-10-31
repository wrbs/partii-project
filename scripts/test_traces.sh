#!/bin/bash

cd "$(dirname "${BASH_SOURCE[0]}")/.."

set -euxo pipefail

make

PROGRAM=$1
shift 1

cd src
cargo run compare-traces "$@" ../vendor/no-aslr/no-aslr ../resources/test_bc/${PROGRAM}.byte