(executable (name lists) (modules lists) (modes byte exe))

(alias (name buildbench) (deps lists.exe))
(alias (name buildbench_byte) (deps lists.bc))
