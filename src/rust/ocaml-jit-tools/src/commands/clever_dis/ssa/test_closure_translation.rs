// Demo with a while loop from "demo.ml"

use crate::commands::clever_dis::data::Closure;
use crate::commands::clever_dis::ssa::data::SSABlock;
use crate::commands::clever_dis::ssa::translate_closure;
use anyhow::{Error, Result};
use expect_test::{expect, Expect};

const WHILE_LOOP_JSON: &str = include_str!("./while_loop.json");

fn expect_block(block: &SSABlock, expect: Expect) {
    expect.assert_eq(&format!("{}{}", block, block.final_state));
}

#[test]
fn test_while_looper() -> Result<()> {
    let closure: Closure = serde_json::from_str(WHILE_LOOP_JSON)?;
    let _ = translate_closure(&closure)?;
    Ok(())
}
