(executable (name stress) (modules stress) (modes byte exe))

(alias (name buildbench) (deps stress.exe))
(alias (name buildbench_byte) (deps stress.bc))
