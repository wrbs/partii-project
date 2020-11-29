(executable (name hashtbl_bench) (modules hashtbl_bench) (modes byte exe))

(alias (name buildbench) (deps hashtbl_bench.exe))
(alias (name buildbench_byte) (deps hashtbl_bench.bc))
