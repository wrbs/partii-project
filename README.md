# Part II Project

## Initial setup

Make sure you have rust installed (google it) and enough build basics for OCaml

Then run

    make setup

Which will configure the included ocaml source (in `ocaml-jit`) to build using
the jit

Run

    make all

to build everything (including OCaml). This will take a while. If you have a
multicore processor using

    MAKEFLAGS="-j {number of threads}" make all
    
helps a lot.

However, in most development iterations you can probably get away with only
rebuilding the runtime.

## Project overview

The project is developed primarily in Rust and C. The aim is to use some form
of JIT compilation to replace the OCaml bytecode interpreter.

### Directory contents

- `ocmal-jit`: a fork of the OCaml compiler's source tree. The bits of
   interest for this project are in:
    - `ocaml-jit/runtime` - the runtime C library
- `src`: main Rust source for the project. There are a few different Rust
  crates included:
    - `src/ocaml-jit-staticlib`: the static library that links into the OCaml
      runtime providing the JIT
    - `src/ocaml-jit-tools`: standalone tools for testing and debugging the
      core without having to go through the OCaml runtime
    - `src/ocaml-bytecode`: depended on by both of these files, it contains
      the core logic
- `docs`: dissertation LaTeX source
- `resources`: test files
- `dist`: once you run the setup, this is used as the prefix for OCaml's
  `make install`.  You, using `export PATH="$PWD/dist/bin:$PATH` (or absolute
   paths to binaries) and our Makefiles can then use our custom compiler
   version.

### Bytecode vs native code

Most actual uses of OCaml use the native code compiler. The main use of the
bytecode compiler in most actual uses is

1. running the toplevel (REPL)
2. bootstrapping the OCaml compiler
3. as an easier target for experimenting with new compiler features before
 having to write platform-specific codegen
4 m.akes porting OCaml to new platforms is easier as only (already fairly
 portable) C needs to be ported before needing to do the work on 

Experimenting with making a JIT is could be an improvement to the first 2 of
these.

### OCaml's runtime system

To understand how it hooks in you need to understand a little bit about how
the OCaml compiler and runtime works.

OCaml has a runtime system written in C. Some of the things this does is:

- garbage collection
- abstract OS functionality between platforms
- abstract most platform differences (32-bit/64-bit is still significant).
- deals with C primitives and loading dynamic libraries
- has support for serializing and deserializing OCaml values to binary

It lives in the [runtime](ocaml-jit/runtime) directory in the OCaml source tree.

There are a two main different targets we're interested in this programming

- `libcamlrun.a` - The bytecode static library
- `ocamlrun` - An executable for running ocaml bytecode

The native code compiler also links to this library, but some details are
different in terms of `#define` flags set and object files linked in. We're
not interested in this.

### What happens when you execute a program

When ocaml builds a bytecode program it creates an executable file with the `#!`
line set up to reference the absolute path to the `ocamlrun` program that was
built. You can also execute directly using `ocamlrun`.

The runtime library parses the bytecode file. The format is custom and
not really documented so is best understood by
reading either [this project's rust parser](src/ocaml-bytecode/src/trailer.rs)
or looking into the compiler's source. However, it's very simple:

It consists of a trailer (at the end of the file) with a magic number and a
count of the number of *sections* in the file. Based on the number of
sections, it reads a table of contents section immediately preceding
that containing `(section name (4*u8), section length)` tuples. These
sections are assumed to be packed in order immediately preceding the table
of contents.

By keeping everything packed towards the end of the file it can permit any
format of `#!` of any length, or even another stub for other platforms.

#### Sections

- `CODE`: Bytecode
- `DATA`: Initial global data
- `PRIMS`: names of imported C primitives used by the program (which later
 refers to them by number).
- some other stuff for debugging and shared library loading

### Startup

These sections are loaded. When we load the bytecode there's a point which we
can hook into rust on.

After everything is loaded it jumps to the interpreter giving the first
bytecode element to read.

### The toplevel

The toplevel is the OCaml name for the REPL prompt allowing interactive
development and testing of OCaml. 

It works by compiling the code you write to bytecode and using some
meta-features in the runtime (`meta.c`, `Meta` compiler module) to execute it
in a semi-sandbox.

When it does this it calls the same hooks as the startup calls.

### How we add a JIT to all this

The rust builds a static library from the rust code.

The existing C source is modified (`#ifdef USE_RUST_JIT`) to add and override
some hooks.

1. When `ocamlrun` starts up (for initialising rust stuff such as its own
   runtime's panic system)
2. When new bytecode is loaded, either by `ocamlrun` startup or the toplevel's
   meta stuff we hook into the rust code
3. We override the caml interpreter function
4. (Toplevel only) when code is marked by the toplevel as no longer needed

The idea is at 2 we can do some at program load full program
analysis/preparation of the bytecode and at 3 we can do some JITing.

It may turn out that just compiling everything at 2 is more sensible - it'll
depend on how complicated we make this program.

## Development

If you're only making changes to the rust component and the runtime component of
OCaml you can run only

    make

This will still rebuild `ocamlrun` and install it but it won't recompile the
rest of the compiler.

To run the OCaml tests run

    make ocamltests

To clean up run

    make clean
    
To clean up and also remove the configure stuff

    make fullclean
    
If you make changes to the ocaml configure scripts (`configure.ac`)

    cd ocaml-jit && autoconf
    cd .. && make setup
