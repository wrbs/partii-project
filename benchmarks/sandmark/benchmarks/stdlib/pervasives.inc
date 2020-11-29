(executable (name pervasives_bench) (modules pervasives_bench) (modes byte exe))

(alias (name buildbench) (deps pervasives_bench.exe))
(alias (name buildbench_byte) (deps pervasives_bench.bc))
