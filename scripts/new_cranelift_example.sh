#!/bin/bash

if [[ $# -ne 3 ]] ; then
    echo "Usage: $0 PROGRAM CLOSURE CASE_NAME"
    exit 1
fi

program="$1"
closure="$2"
case_name="$3"

output_dir="/tmp/graphs-new/${program}"
closure_loc="${output_dir}/closure_${closure}.json"
case_dir="src/rust/ocaml-jit-shared/src/cranelift_compiler/test_cases/${case_name}"
test_rs="src/rust/ocaml-jit-shared/src/cranelift_compiler/test.rs"

cd "$(dirname "$0")"
cd ..

if [[ ! -d "${output_dir}" ]]; then
    NO_OPEN=1 scripts/show_chart_new.sh "${program}" --output-closure-json
fi

if [[ ! -f "${closure_loc}" ]]; then
    echo "No such closure: ${closure_loc}"
    exit 1
fi

mkdir -p "${case_dir}"
cp "${closure_loc}" "${case_dir}/closure.json"
echo "test_case!(${case_name});" >> "${test_rs}"