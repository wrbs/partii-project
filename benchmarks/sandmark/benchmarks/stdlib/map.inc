(executable (name map_bench) (modules map_bench) (modes byte exe))

(alias (name buildbench) (deps map_bench.exe))
(alias (name buildbench_byte) (deps map_bench.bc))
