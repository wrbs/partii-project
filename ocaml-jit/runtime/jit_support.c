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

