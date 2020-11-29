(executable (name weakretain) (modules weakretain) (modes byte exe))
; (executable (name weak_htbl) (modules weak_htbl))

;(alias (name buildbench) (deps weakretain.exe weak_htbl.exe))
(alias (name buildbench) (deps weakretain.exe))
(alias (name buildbench_byte) (deps weakretain.bc))
