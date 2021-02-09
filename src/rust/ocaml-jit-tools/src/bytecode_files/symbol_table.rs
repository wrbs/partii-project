use super::ml_data::input_value;
use super::trailer::{Trailer, SYMB_SECTION};
use crate::bytecode_files::MLValue;
use anyhow::{bail, Result};
use std::collections::HashMap;
use std::fs::File;

pub fn parse_symbol_table(f: &mut File, trailer: &Trailer) -> Result<HashMap<usize, String>> {
    let mut section = trailer
        .find_required_section(SYMB_SECTION)?
        .read_section(f)?;

    let (blocks, val) = input_value(&mut section)?;

    let mut entries = HashMap::new();
    match &val {
        MLValue::Block { tag: 0, items } => match items.as_slice() {
            [MLValue::Int(_next_item), map] => {
                find_rec(map, &mut entries)?;
            }
            _ => bail!("Unexpected symbol table format - Num_tbl.t"),
        },
        _ => bail!("Unexpected symbol table format - Num_tbl.t"),
    }

    Ok(entries)
}

/*
 * The symbol table is a IntMap.Make(Ident)
 *  type 'a t =
 *    Empty                                               (* tag 0 *)
 *  | Node of {l:'a t; v:key; d:'a; r:'a t; h:int}        (* tag 1 *)
 *
 * An Ident is of type
 * | Local of { name: string; stamp: int }                (* tag 0 *)
 * | Scoped of { name: string; stamp: int; scope: int }   (* tag 1 *)
 * | Global of string                                     (* tag 2 *)
 * | Predef of { name: string; stamp: int }               (* tag 3 *)
 *
 * In all cases the string is the first field in an ident so we don't need to care about tags
 */

fn find_rec(val: &MLValue, entries: &mut HashMap<usize, String>) -> Result<()> {
    match val {
        MLValue::Block { tag: 0, items } => match items.as_slice() {
            [l, v, MLValue::Int(index), r, _] => {
                find_rec(l, entries)?;
                match v {
                    MLValue::Block { tag: _, items } => match items.get(0) {
                        Some(MLValue::StringUtf8(s)) => {
                            entries.insert(*index as usize, s.clone());
                        }
                        _ => bail!("Unexpected symbol table format - Ident"),
                    },
                    _ => bail!("Unexpected symbol table format - Map.Make Node"),
                }
                find_rec(r, entries)?;
            }
            _ => bail!("Unexpected symbol table format - Map.Make t"),
        },
        _ => (),
    }

    Ok(())
}
