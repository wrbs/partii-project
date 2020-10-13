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

/* Callbacks from C to Rust */

void rust_jit_trace(code_t pc, uint64_t sp, int64_t value);

/* Exposing some of the macro stuff as functions to avoid having to rewrite them in Rust when it's
 * out of scope for the JIT */

value jit_support_alloc_small(int64_t wosize, uint8_t tag);


#endif /* CAML_INTERNALS */

#endif /* CAML_JIT_SUPPORT */
