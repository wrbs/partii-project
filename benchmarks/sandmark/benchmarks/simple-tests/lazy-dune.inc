(executable (name lazylist) (modules lazylist) (modes byte exe))
(executable (name lazy_primes) (modules lazy_primes) (modes byte exe))

(alias (name buildbench) (deps lazylist.exe lazy_primes.exe))
(alias (name buildbench_byte) (deps lazylist.bc lazy_primes.bc))
