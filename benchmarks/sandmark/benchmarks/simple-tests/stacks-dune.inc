(executable (name stacks) (modules stacks) (modes byte exe))

(alias (name buildbench) (deps stacks.exe))
(alias (name buildbench_byte) (deps stacks.bc))
