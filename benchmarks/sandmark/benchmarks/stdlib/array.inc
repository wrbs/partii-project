(executable (name array_bench) (modules array_bench) (modes byte exe))

(alias (name buildbench) (deps array_bench.exe))
(alias (name buildbench_byte) (deps array_bench.bc))
