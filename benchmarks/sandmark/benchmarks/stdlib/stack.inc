(executable (name stack_bench) (modules stack_bench) (modes byte exe))

(alias (name buildbench) (deps stack_bench.exe))
(alias (name buildbench_byte) (deps stack_bench.bc))
