// Demo with a while loop from "demo.ml"

use crate::commands::clever_dis::data::Closure;
use crate::commands::clever_dis::ssa::{get_referencing, translate_blocks};
use anyhow::Result;
use expect_test::expect;

const WHILE_LOOP_JSON: &str = include_str!("./while_loop.json");

#[test]
fn test_while_looper() -> Result<()> {
    let closure: Closure = serde_json::from_str(WHILE_LOOP_JSON)?;
    let blocks = translate_blocks(&closure.blocks)?;
    let e = expect![[r#"
        [
            {},
            {
                2,
                0,
            },
            {
                1,
            },
            {
                1,
            },
        ]
    "#]];
    e.assert_debug_eq(&get_referencing(&blocks));

    // Todo
    // Go through the blocks in numerical order.
    // If all referencing blocks have a smaller number, unify and emit phi nodes where differences
    // are
    // If any referencing blocks have a greater number emit a phi node and a "patch request"
    // When we do that block later, make sure to patch up the phi node from any previous blocks

    Ok(())
}
