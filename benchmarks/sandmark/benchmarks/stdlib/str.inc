(executable (name str_bench) (modules str_bench) (libraries str) (modes byte exe))

(alias (name buildbench) (deps str_bench.exe))
(alias (name buildbench_byte) (deps str_bench.bc))
