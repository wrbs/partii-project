#!/bin/bash

git archive --format=tar --prefix=ocaml-jit/ HEAD | tar x
rm ocaml-jit/docs/wnr21*

tar czf wnr21.tar.gz ocaml-jit/
rm -rf ocaml-jit