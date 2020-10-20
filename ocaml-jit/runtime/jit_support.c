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

value* jit_support_check_stacks(value* sp) {
    if (sp < Caml_state->stack_threshold) {
        Caml_state->extern_sp = sp;
        caml_realloc_stack(Stack_threshold / sizeof(value));
        return Caml_state->extern_sp;
    } else {
        return sp;
    }
}

value* jit_support_appterm_stacks(int64_t nargs, int64_t slotsize, value* sp) {

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
