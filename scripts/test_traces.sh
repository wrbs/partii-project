#!/bin/bash

cd "$(dirname "${BASH_SOURCE[0]}")/.."

set -euxo pipefail

make

cd src
cargo run compare-traces ../vendor/no-aslr/no-aslr ../resources/test_bc/$1.byte