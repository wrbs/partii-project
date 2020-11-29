(executable (name set_bench) (modules set_bench) (modes byte exe))

(alias (name buildbench) (deps set_bench.exe))
(alias (name buildbench_byte) (deps set_bench.bc))
