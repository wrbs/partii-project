(executable (name big_array_bench) (modules big_array_bench) (libraries bigarray) (modes byte exe))

(alias (name buildbench) (deps big_array_bench.exe))
(alias (name buildbench_byte) (deps big_array_bench.bc))
