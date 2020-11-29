(executable (name string_bench) (modules string_bench) (modes byte exe))

(alias (name buildbench) (deps string_bench.exe))
(alias (name buildbench_byte) (deps string_bench.bc))
