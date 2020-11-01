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

#define CAML_INTERNALS

#include <stdint.h>
#include "caml/jit_support.h"
#include "caml/memory.h"
#include "caml/mlvalues.h"
#include "caml/domain.h"
#include "caml/stacks.h"
#include "caml/prims.h"
#include "caml/callback.h"

// Set up the macros needed
#undef Alloc_small_origin
#define Alloc_small_origin CAML_FROM_CAML
#define Setup_for_gc
#define Restore_after_gc

value jit_support_main_wrapper(value (*compiled_function)(struct initial_state*), value (*longjmp_handler)(struct initial_state*, value init_accu)) {
    struct longjmp_buffer raise_buf;
    struct initial_state is;

    is.initial_local_roots = Caml_state->local_roots;
    is.initial_sp_offset = (char *) Caml_state->stack_high - (char *) Caml_state->extern_sp;
    is.initial_sp = Caml_state->extern_sp;
    is.initial_external_raise = Caml_state->external_raise;
    caml_callback_depth++;

    if (sigsetjmp(raise_buf.buf, 0)) {
        Caml_state->local_roots = is.initial_local_roots;
        // Check_trap_barrier;
        // if (Caml_state->backtrace_active) {
        //     /* pc has already been pushed on the stack when calling the C
        //        function that raised the exception. No need to push it again
        //        here. */
        //     caml_stash_backtrace(accu, sp, 0);
        // }

        if ((char *) Caml_state->trapsp
            >= (char *) Caml_state->stack_high - is.initial_sp_offset) {
            Caml_state->external_raise = is.initial_external_raise;
            Caml_state->extern_sp = (value *) ((char *) Caml_state->stack_high
                                               - is.initial_sp_offset);
            caml_callback_depth--;
            return Make_exception_result(Caml_state->exn_bucket);
        }

        return longjmp_handler(&is, Caml_state->exn_bucket);
    }
    Caml_state->external_raise = &raise_buf;

    return compiled_function(&is);
}

value jit_support_alloc_small(int64_t wosize, uint8_t tag) {
    value result;
    Alloc_small(result, wosize, tag);
    return result;
}

value jit_support_get_float_field(value ptr, int64_t fieldno) {
    value x;
    double d = Double_flat_field(ptr, fieldno);
    Alloc_small(x, Double_wosize, Double_tag);
    Store_double_val(x, d);
    return x;
}

void jit_support_set_float_field(value ptr, int64_t fieldno, value to) {
    Store_double_flat_field(ptr, fieldno, Double_val(to));
}

value jit_support_vect_length(value ptr) {
    mlsize_t size = Wosize_val(ptr);
    if (Tag_val(ptr) == Double_array_tag) size = size / Double_wosize;
    return Val_long(size);
}

value *jit_support_check_stacks(value* sp) {
    if (sp < Caml_state->stack_threshold) {
        Caml_state->extern_sp = sp;
        caml_realloc_stack(Stack_threshold / sizeof(value));
        return Caml_state->extern_sp;
    } else {
        return sp;
    }
}

value *jit_support_appterm_stacks(int64_t nargs, int64_t slotsize, value* sp) {

    value* newsp;
    int i;

    newsp = sp + slotsize - nargs;
    for(i = nargs - 1; i >= 0; i--) newsp[i] = sp[i];
    return jit_support_check_stacks(newsp);
}


void jit_support_closure(struct jit_state* state, int64_t nvars, void* codeval) {
    int i;
    if (nvars > 0) *--state->sp = state->accu;
    if (nvars < Max_young_wosize) {
        /* nvars + 1 <= Max_young_wosize, can allocate in minor heap */
        Alloc_small(state->accu, 1 + nvars, Closure_tag);
        for (i = 0; i < nvars; i++) Field(state->accu, i + 1) = state->sp[i];
    } else {
        /* PR#6385: must allocate in major heap */
        /* caml_alloc_shr and caml_initialize never trigger a GC,
           so no need to Setup_for_gc */
        state->accu = caml_alloc_shr(1 + nvars, Closure_tag);
        for (i = 0; i < nvars; i++) caml_initialize(&Field(state->accu, i + 1), state->sp[i]);
    }
    /* The code pointer is not in the heap, so no need to go through
       caml_initialize. */
    Code_val(state->accu) = codeval;
    state->sp += nvars;
}

void jit_support_closure_rec(struct jit_state* state, int64_t nvars, void** codevals, int64_t nfuncs) {
    mlsize_t blksize = nfuncs * 2 - 1 + nvars;
    int i;
    value * p;

    if (nvars > 0) *--state->sp = state->accu;
    if (blksize <= Max_young_wosize) {
        Alloc_small(state->accu, blksize, Closure_tag);
        p = &Field(state->accu, nfuncs * 2 - 1);
        for (i = 0; i < nvars; i++, p++) *p = state->sp[i];
    } else {
        /* PR#6385: must allocate in major heap */
        /* caml_alloc_shr and caml_initialize never trigger a GC,
           so no need to Setup_for_gc */
        state->accu = caml_alloc_shr(blksize, Closure_tag);
        p = &Field(state->accu, nfuncs * 2 - 1);
        for (i = 0; i < nvars; i++, p++) caml_initialize(p, state->sp[i]);
    }
    state->sp += nvars;
    /* The code pointers and infix headers are not in the heap,
       so no need to go through caml_initialize. */
    p = &Field(state->accu, 0);
    *p = (value) codevals[0];
    *--state->sp = state->accu;
    p++;
    for (i = 1; i < nfuncs; i++) {
        *p = Make_header(i * 2, Infix_tag, Caml_white);  /* color irrelevant. */
        p++;
        *p = (value) codevals[i];
        *--state->sp = (value) p;
        p++;
    }
}

void jit_support_make_block(struct jit_state* state, int64_t _wosize, int64_t _tag) {
    mlsize_t wosize = (mlsize_t) _wosize;
    tag_t tag = (tag_t) _tag;
    mlsize_t i;
    value block;
    if (wosize <= Max_young_wosize) {
        Alloc_small(block, wosize, tag);
        Field(block, 0) = state->accu;
        for (i = 1; i < wosize; i++) Field(block, i) = *state->sp++;
    } else {
        block = caml_alloc_shr(wosize, tag);
        caml_initialize(&Field(block, 0), state->accu);
        for (i = 1; i < wosize; i++) caml_initialize(&Field(block, i), *state->sp++);
    }
    state->accu = block;
}

void jit_support_make_float_block(struct jit_state* state, int64_t size) {
    mlsize_t i;
    value block;
    if (size <= Max_young_wosize / Double_wosize) {
        Alloc_small(block, size * Double_wosize, Double_array_tag);
    } else {
        block = caml_alloc_shr(size * Double_wosize, Double_array_tag);
    }
    Store_double_flat_field(block, 0, Double_val(state->accu));
    for (i = 1; i < size; i++){
        Store_double_flat_field(block, i, Double_val(*state->sp));
        ++state->sp;
    }
    state->accu = block;
}

void *jit_support_get_primitive(uint64_t primno) {
    return Primitive(primno);
}

void jit_support_restart(struct jit_state* state) {
    int num_args = Wosize_val(state->env) - 2;
    int i;
    state->sp -= num_args;
    for (i = 0; i < num_args; i++) state->sp[i] = Field(state->env, i + 2);
    state->env = Field(state->env, 1);
    state->extra_args += num_args;
}

void* jit_support_grab_closure(struct jit_state* state, void* prev_restart) {
    mlsize_t num_args, i;
    void* next_pc;

    num_args = 1 + state->extra_args; /* arg1 + extra args */
    Alloc_small(state->accu, num_args + 2, Closure_tag);
    Field(state->accu, 1) = state->env;
    for (i = 0; i < num_args; i++) Field(state->accu, i + 2) = state->sp[i];
    Code_val(state->accu) = prev_restart; /* Point to the preceding RESTART instr. */
    state->sp += num_args;
    next_pc = (void*)(state->sp[0]);
    state->env = state->sp[1];
    state->extra_args = Long_val(state->sp[2]);
    state->sp += 3;

    return next_pc;
}

void jit_support_stop(struct initial_state* is, value *sp) {
    Caml_state->external_raise = is->initial_external_raise;
    Caml_state->extern_sp = sp;
    caml_callback_depth--;
}

long jit_support_raise_check(struct initial_state* is) {
    if ((char *) Caml_state->trapsp
        >= (char *) Caml_state->stack_high - is->initial_sp_offset) {
        Caml_state->external_raise = is->initial_external_raise;
        Caml_state->extern_sp = (value *) ((char *) Caml_state->stack_high
                                           - is->initial_sp_offset);
        caml_callback_depth--;
        return 1;
    } else {
        return 0;
    }
}