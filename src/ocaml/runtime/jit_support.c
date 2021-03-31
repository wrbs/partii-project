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
#define Setup_for_gc \
  { state->sp -= 3; state->sp[0] = state->accu; state->sp[1] = state->env; state->sp[2] = Val_unit; \
      Caml_state->extern_sp = state->sp; }
#define Restore_after_gc \
  { state->sp = Caml_state->extern_sp; state->accu = state->sp[0]; state->env = state->sp[1]; state->sp += 3; }


value jit_support_get_float_field(struct jit_state* state, int64_t fieldno) {
    value x;
    double d = Double_flat_field(state->accu, fieldno);
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

/*
void resolve_ip(uint64_t *val);
#ifdef USE_RUST_JIT
    uint64_t **bp;
    asm ("mov %%rbp, %0;" : "=r" (bp));



    while(bp != 0) {
        printf("BP=%p\n", bp);
        resolve_ip(*(bp + 1));
        bp = (uint64_t **) *bp;

    }

    asm("int $3");

#endif
*/

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
    return newsp;
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
    state->accu = Field(state->env, 1);
    state->extra_args += num_args;
}

void* jit_support_grab_closure(struct jit_state* state, void* restart_code) {
    mlsize_t num_args, i;
    void* next_pc;

    num_args = 1 + state->extra_args; /* arg1 + extra args */
    Alloc_small(state->accu, num_args + 2, Closure_tag);
    Field(state->accu, 1) = state->env;
    for (i = 0; i < num_args; i++) Field(state->accu, i + 2) = state->sp[i];
    Code_val(state->accu) = restart_code; /* Point to the code to handle a RESTART */
    state->sp += num_args;
    next_pc = (void*)(state->sp[0]);
    state->env = state->sp[1];
    state->extra_args = Long_val(state->sp[2]);
    state->sp += 3;

    return next_pc;
}

value jit_support_get_dyn_met(value tag, value obj) {
    value meths = Field (obj, 0);
    int li = 3, hi = Field(meths,0), mi;
    while (li < hi) {
        mi = ((li+hi) >> 1) | 1;
        if (tag < Field(meths,mi)) hi = mi-2;
        else li = mi;
    }
    return Field (meths, li-1);
}
