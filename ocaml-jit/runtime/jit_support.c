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

// Set up the macros needed
#undef Alloc_small_origin
#define Alloc_small_origin CAML_FROM_CAML
#define Setup_for_gc
#define Restore_after_gc

value jit_support_alloc_small(int64_t wosize, uint8_t tag) {
    value result;
    Alloc_small(result, wosize, tag);
    return result;
}

value jit_support_get_field(value ptr, int64_t fieldno) {
    return Field(ptr, fieldno);
}

void jit_support_set_field(value ptr, int64_t fieldno, value to) {
    caml_modify(&Field(ptr, fieldno), to);
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

value *jit_support_get_initial_sp() {
    return Caml_state->extern_sp;
}

void jit_support_closure_rec(struct jit_state* state, int64_t nvars, void* codeval) {
    int nfuncs = 1;
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
    *p = (value) codeval;
    *--state->sp = state->accu;
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
 
void *jit_support_get_primitive(uint64_t primno) {
    return Primitive(primno);
}
