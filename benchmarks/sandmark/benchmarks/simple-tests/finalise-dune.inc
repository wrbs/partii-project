(executable (name finalise) (modules finalise) (modes byte exe))

(alias (name buildbench) (deps finalise.exe))
(alias (name buildbench_byte) (deps finalise.bc))
