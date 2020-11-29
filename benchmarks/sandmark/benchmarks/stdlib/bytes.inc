(executable (name bytes_bench) (modules bytes_bench) (modes byte exe))

(alias (name buildbench) (deps bytes_bench.exe))
(alias (name buildbench_byte) (deps bytes_bench.bc))
