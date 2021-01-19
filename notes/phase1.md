# Phase 1 - Mich term

Initial design

- just translate
- hope it will be faster
- focus on full correctness

Methods used:

- testing traces after disabling ASLR with hand-crafted tests picked to cover the range of
  instructions
- running test cases from the OCaml compiler
- getting the compiler to reliably self-host under JIT
- porting sandmark

Milestones were hit: the initial design and self-hosting was achieved by the start of Christmas
break. Over Christmas the benchmarks were set up and showed good promising speedup results.

Many aspects of the engineering were novel and there were lessons learned about how to be effective
and correct with what is essentially a refactoring problem on a compiler.

The challenge at this point was to come up with the set of extensions to really focus on novel
ideas.

There are three places to look:

- Smaller optimisations to the emitted code to try to speed up slow things in the assembly
    - Store caml_state in a register!
    - Use a jump table instead of switch statements
- Compiler optimisations while emitting code - use an intermediate layer
- Threading in optional usage of the JIT with the ability to specialise functions based on
  information gained during execution

## Machinery needed

- Ability to compare automatically two different versions of the compiler against each-other with
  as little overhead as possible
- Ability to benchmark better inside the compiler

## Stuff possibly worth looking at

- https://www.cl.cam.ac.uk/~mom22/jit/jit.pdf
- https://github.com/wdv4758h/awesome-jit

- https://dl.acm.org/doi/10.1145/857076.857077 (Pycket: a tracing JIT for a functional language 2015)
- https://arxiv.org/abs/1011.1783 (OcamlJIT2 2010)
- https://dl.acm.org/doi/10.1145/2858949.2784740 (A brief history of just in time 2003)
- https://www.usenix.org/legacy/publications/library/proceedings/jvm01/full_papers/paleczny/paleczny.pdf (the Java HotSpot server compiler 2001)
- https://dl.acm.org/doi/10.1145/1542476.1542528 (trace-based just-in-type type specialisation for
  dynamic languages 2009, gal et al)


## Research notes

There is a distinction between tracing JITs and method-based JITs [pycket]. Trace-based type
specialisation [gal et al] for JS works by the VM mapping values to types in hot loops in trees.

# Optimised compiler

All of the ideas will require the use of an optimising compiler.

Idea - as we parse the bytecode we keep track of the state of the stack in a virtual way.

Convert to an SSA IR - i.e. recover some of the structure from the bytecode.

# Problem with naive intermediate SSA IR

Everything we use must be on the stack to live well with the garbage collector - it scans the local
roots. Does this limit the opportunity for faster compilation? Everything at alloc time must be on
the stack. 

Allocs happen when making blocks, making closures, or potentially during C calls. At those times
all of the memory addresses of values could change from under us. Every register would need to be
pushed to the OCaml stack if we were storing some values in registers.

Potential solution: the IR would have commands to push and pop registers to the OCaml stack before
making a call that hits the GC. This is fine - it's already needed for calling a C function with
the right calling convention. But it does make things somewhat limited with how much you can get
away with in terms of optimisations.

# Current plan

Work on optimisations first and investigate compiler speed.

If any time left consider learning information from execution and threading (less likely given time
constraints) or incremental compilation (more likely but possibly useless if the compiler's fast
enough).
