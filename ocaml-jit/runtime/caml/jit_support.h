/**************************************************************************/
/*                                                                        */
/*                          OCaml (JIT support)                           */
/*                                                                        */
/*               William Robson, University of Cambridge                  */
/*                                                                        */
/*   Copyright 2020 William Robson                                        */
/*                                                                        */
/*   All rights reserved.  This file is distributed under the terms of    */
/*   the GNU Lesser General Public License version 2.1, with the          */
/*   special exception on linking described in the file LICENSE.          */
/*                                                                        */
/**************************************************************************/

#ifndef CAML_JIT_SUPPORT
#define CAML_JIT_SUPPORT


#ifdef CAML_INTERNALS

#include <stdint.h>
#include "mlvalues.h"
#include "exec.h"
#include "fail.h"

/* There's some interpreter state I don't want the JIT code to care about keeping track of
 * and will be passing to C primitives anyway. jit_support_main_wrapper is responsible for
 * setting up this stuff and branching into the JIT compiled code with a pointer to this
 * state struct on the stack. Other callbacks can use it by taking a pointer to it from
 * the JIT code */
 
struct initial_state {
    value* initial_sp;
    struct longjmp_buffer * initial_external_raise;
    intnat initial_sp_offset;
    struct caml__roots_block * volatile initial_local_roots;
    struct longjmp_buffer raise_buf;
};

value jit_support_main_wrapper(
    value (*compiled_function)(struct initial_state*),
    value (*longjmp_handler)(struct initial_state*, value init_accu)
);

/* Callbacks from C to Rust */

void rust_jit_trace(uint64_t  pc, uint64_t accu, uint64_t env, uint64_t extra_args, value* sp);

/* Exposing some of the macro stuff as functions to avoid having to rewrite them in Rust when it's
 * out of scope for the JIT */

struct jit_state {
    value accu;
    value env;
    value* sp;
    value extra_args;
};

value jit_support_get_float_field(struct jit_state* state, int64_t fieldno);
void jit_support_set_float_field(value ptr, int64_t fieldno, value to);

value jit_support_vect_length(value ptr);

value *jit_support_check_stacks(value* sp);
value *jit_support_appterm_stacks(int64_t nargs, int64_t slotsize, value* sp);


void jit_support_closure(struct jit_state* state, int64_t nvars, void* codeval);
void jit_support_closure_rec(struct jit_state* state, int64_t nvars, void** codevals, int64_t nfuncs);

void jit_support_make_block(struct jit_state* state, int64_t wosize, int64_t tag);
void jit_support_make_float_block(struct jit_state* state, int64_t size);

void *jit_support_get_primitive(uint64_t primno);

void jit_support_restart(struct jit_state* state);
void* jit_support_grab_closure(struct jit_state* state, void* prev_restart);

void jit_support_stop(struct initial_state* is, value *sp);
long jit_support_raise_check(struct initial_state* is);

value jit_support_get_dyn_met(value tag, value obj);

#endif /* CAML_INTERNALS */

#endif /* CAML_JIT_SUPPORT */
