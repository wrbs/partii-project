// Demo with a while loop from "demo.ml"

use crate::commands::clever_dis::data::Closure;
use crate::commands::clever_dis::ssa::data::SSABlock;
use crate::commands::clever_dis::ssa::{get_blocks, relocate_blocks};
use anyhow::Result;
use expect_test::{expect, Expect};
use std::fmt::Write;

fn show_blocks(blocks: &[SSABlock], expect: Expect) {
    let mut actual = String::new();
    for (i, b) in blocks.iter().enumerate() {
        write!(actual, "Block {}:\n{}{}\n", i, b, b.final_state).unwrap();
    }
    expect.assert_eq(&actual);
}

struct Test {
    source: &'static str,
    first_parse: Expect,
    after_relocate: Expect,
}

fn run_test(test: Test) -> Result<()> {
    let closure: Closure = serde_json::from_str(test.source)?;
    let mut blocks = get_blocks(&closure)?;
    show_blocks(&blocks, test.first_parse);

    relocate_blocks(&mut blocks)?;
    show_blocks(&blocks, test.after_relocate);
    Ok(())
}

// #[test]
// fn test_thing() -> Result<()> {
//     run_test(Test {
//         source: WHILE_LOOP_JSON,
//         first_parse: expect![[]],
//         after_relocate: expect![[]],
//     })
// }

#[test]
fn test_while_looper() -> Result<()> {
    run_test(Test {
        source: include_str!("./while_loop.json"),
        first_parse: expect![[r#"
            Block 0:
            <0_0> = make block tag:0 vars:[0]
            Exit: jump 1
            Final acc: <0_0>
            End stack: ..., <prev:0> | <arg:0>, <0_0>
            Used prev: []
            Stack delta: -0/+2

            Block 1:
            <1_0> = <prev:0>[0]
            <1_1> = 10 > <1_0>
            Exit: jump_if <1_1> t:2 f:3
            Final acc: <1_0>
            End stack: ..., <prev:1> | <prev:0>
            Used prev: [0]
            Stack delta: -1/+1

            Block 2:
            check signals
            <2_0> = <prev:0>[0]
            <2_1> = global 310
            <2_2> = global 308
            <2_3> = <2_2>[1]
            <2_4> = apply <2_3> [<2_1>, <2_0>]
            <2_5> = <prev:0>[0]
            <2_6> = <2_5> + 1
            set <prev:0>[0] = <2_6>
            Exit: jump 1
            Final acc: <unit>
            End stack: ..., <prev:1> | <prev:0>
            Used prev: [0]
            Stack delta: -1/+1

            Block 3:
            Exit: return 0
            Final acc: 0
            End stack: ..., <prev:2> | 
            Used prev: []
            Stack delta: -2/+0

        "#]],
        after_relocate: expect![[r#"
            Block 0:
            <0_0> = make block tag:0 vars:[0]
            Exit: jump 1
            Final acc: <0_0>
            End stack: ..., <prev:0> | <arg:0>, <0_0>
            Used prev: []
            Stack delta: -0/+2

            Block 1:
            <1_0> = phi 0:<0_0> 2:<special>
            <1_1> = phi 0:<0_0> 2:<special>
            <1_2> = <1_1>[0]
            <1_3> = 10 > <1_2>
            Exit: jump_if <1_3> t:2 f:3
            Final acc: <1_2>
            End stack: ..., <prev:1> | <1_1>
            Used prev: [0]
            Stack delta: -1/+1

            Block 2:
            check signals
            <2_0> = <1_1>[0]
            <2_1> = global 310
            <2_2> = global 308
            <2_3> = <2_2>[1]
            <2_4> = apply <2_3> [<2_1>, <2_0>]
            <2_5> = <1_1>[0]
            <2_6> = <2_5> + 1
            set <1_1>[0] = <2_6>
            Exit: jump 1
            Final acc: <unit>
            End stack: ..., <prev:1> | <1_1>
            Used prev: [0]
            Stack delta: -1/+1

            Block 3:
            Exit: return 0
            Final acc: 0
            End stack: ..., <prev:2> | 
            Used prev: []
            Stack delta: -2/+0

        "#]],
    })
}

#[test]
fn test_seq_filter_map() -> Result<()> {
    run_test(Test {
        source: include_str!("./seq_filter_map.json"),
        first_parse: expect![[r#"
            Block 0:
            grab 2
            <0_0> = apply <arg:1> [0]
            Exit: jump_if <0_0> t:2 f:1
            Final acc: <0_0>
            End stack: ..., <prev:0> | <arg:2>, <arg:1>, <arg:0>, <0_0>
            Used prev: []
            Stack delta: -0/+4

            Block 1:
            Exit: return 0
            Final acc: 0
            End stack: ..., <prev:4> | 
            Used prev: []
            Stack delta: -4/+0

            Block 2:
            <2_0> = <prev:0>[1]
            <2_1> = <prev:0>[0]
            <2_2> = apply <prev:1> [<2_1>]
            Exit: jump_if <2_2> t:4 f:3
            Final acc: <2_2>
            End stack: ..., <prev:2> | <prev:1>, <prev:0>, <2_0>, <2_1>, <2_2>
            Used prev: [0, 1]
            Stack delta: -2/+5

            Block 3:
            Exit: tail_apply <closure:0> [<prev:4>, <prev:2>, 0]
            Final acc: <closure:0>
            End stack: ..., <prev:7> | 
            Used prev: [2, 4]
            Stack delta: -7/+0

            Block 4:
            <4_0> = <prev:0>[0]
            <4_1> = apply <closure:0> [<prev:4>, <prev:2>]
            <4_2> = make block tag:0 vars:[<4_0>, <4_1>]
            Exit: return <4_2>
            Final acc: <4_2>
            End stack: ..., <prev:7> | 
            Used prev: [0, 2, 4]
            Stack delta: -7/+0

        "#]],
        after_relocate: expect![[r#"
            Block 0:
            grab 2
            <0_0> = apply <arg:1> [0]
            Exit: jump_if <0_0> t:2 f:1
            Final acc: <0_0>
            End stack: ..., <prev:0> | <arg:2>, <arg:1>, <arg:0>, <0_0>
            Used prev: []
            Stack delta: -0/+4

            Block 1:
            Exit: return 0
            Final acc: 0
            End stack: ..., <prev:4> | 
            Used prev: []
            Stack delta: -4/+0

            Block 2:
            <2_0> = <0_0>[1]
            <2_1> = <0_0>[0]
            <2_2> = apply <arg:0> [<2_1>]
            Exit: jump_if <2_2> t:4 f:3
            Final acc: <2_2>
            End stack: ..., <prev:2> | <arg:0>, <0_0>, <2_0>, <2_1>, <2_2>
            Used prev: [0, 1]
            Stack delta: -2/+5

            Block 3:
            Exit: tail_apply <closure:0> [<arg:0>, <2_0>, 0]
            Final acc: <closure:0>
            End stack: ..., <prev:7> | 
            Used prev: [2, 4]
            Stack delta: -7/+0

            Block 4:
            <4_0> = <2_2>[0]
            <4_1> = apply <closure:0> [<arg:0>, <2_0>]
            <4_2> = make block tag:0 vars:[<4_0>, <4_1>]
            Exit: return <4_2>
            Final acc: <4_2>
            End stack: ..., <prev:7> | 
            Used prev: [0, 2, 4]
            Stack delta: -7/+0

        "#]],
    })
}

#[test]
fn test_char_uppercase() -> Result<()> {
    run_test(Test {
        source: include_str!("./char_uppercase.json"),
        first_parse: expect![[r#"
            Block 0:
            <0_0> = <arg:0> + -224
            <0_1> = 30 u>= <0_0>
            Exit: jump_if <0_1> t:1 f:2
            Final acc: <0_0>
            End stack: ..., <prev:0> | <arg:0>, <0_0>
            Used prev: []
            Stack delta: -0/+2

            Block 1:
            <1_0> = 23 == <prev:0>
            Exit: jump_if <1_0> t:3 f:4
            Final acc: <prev:0>
            End stack: ..., <prev:1> | <prev:0>
            Used prev: [0]
            Stack delta: -1/+1

            Block 2:
            <2_0> = <prev:0> + 127
            <2_1> = 25 u>= <2_0>
            Exit: jump_if <2_1> t:5 f:6
            Final acc: <2_0>
            End stack: ..., <prev:1> | <prev:0>, <2_0>
            Used prev: [0]
            Stack delta: -1/+2

            Block 3:
            Exit: jump 7
            Final acc: <prev:acc>
            End stack: ..., <prev:1> | 
            Used prev: []
            Stack delta: -1/+0

            Block 4:
            Exit: jump 8
            Final acc: <prev:acc>
            End stack: ..., <prev:1> | 
            Used prev: []
            Stack delta: -1/+0

            Block 5:
            Exit: jump 8
            Final acc: <prev:acc>
            End stack: ..., <prev:2> | 
            Used prev: []
            Stack delta: -2/+0

            Block 6:
            Exit: jump 7
            Final acc: <prev:acc>
            End stack: ..., <prev:2> | 
            Used prev: []
            Stack delta: -2/+0

            Block 7:
            Exit: return <prev:0>
            Final acc: <prev:0>
            End stack: ..., <prev:1> | 
            Used prev: [0]
            Stack delta: -1/+0

            Block 8:
            <8_0> = <prev:0> + -32
            Exit: return <8_0>
            Final acc: <8_0>
            End stack: ..., <prev:1> | 
            Used prev: [0]
            Stack delta: -1/+0

        "#]],
        after_relocate: expect![[r#"
            Block 0:
            <0_0> = <arg:0> + -224
            <0_1> = 30 u>= <0_0>
            Exit: jump_if <0_1> t:1 f:2
            Final acc: <0_0>
            End stack: ..., <prev:0> | <arg:0>, <0_0>
            Used prev: []
            Stack delta: -0/+2

            Block 1:
            <1_0> = 23 == <0_0>
            Exit: jump_if <1_0> t:3 f:4
            Final acc: <0_0>
            End stack: ..., <prev:2> | <arg:0>, <0_0>
            Used prev: [0, 1]
            Stack delta: -2/+2

            Block 2:
            <2_0> = <0_0> + 127
            <2_1> = 25 u>= <2_0>
            Exit: jump_if <2_1> t:5 f:6
            Final acc: <2_0>
            End stack: ..., <prev:2> | <arg:0>, <0_0>, <2_0>
            Used prev: [0, 1]
            Stack delta: -2/+3

            Block 3:
            Exit: jump 7
            Final acc: <0_0>
            End stack: ..., <prev:2> | <arg:0>
            Used prev: [1]
            Stack delta: -2/+1

            Block 4:
            Exit: jump 8
            Final acc: <0_0>
            End stack: ..., <prev:2> | <arg:0>
            Used prev: [1]
            Stack delta: -2/+1

            Block 5:
            Exit: jump 8
            Final acc: <2_0>
            End stack: ..., <prev:3> | <arg:0>
            Used prev: [2]
            Stack delta: -3/+1

            Block 6:
            Exit: jump 7
            Final acc: <2_0>
            End stack: ..., <prev:3> | <arg:0>
            Used prev: [2]
            Stack delta: -3/+1

            Block 7:
            <7_0> = phi 3:<0_0> 6:<2_0>
            Exit: return <arg:0>
            Final acc: <arg:0>
            End stack: ..., <prev:1> | 
            Used prev: [0]
            Stack delta: -1/+0

            Block 8:
            <8_0> = phi 4:<0_0> 5:<2_0>
            <8_1> = <arg:0> + -32
            Exit: return <8_1>
            Final acc: <8_1>
            End stack: ..., <prev:1> | 
            Used prev: [0]
            Stack delta: -1/+0

        "#]],
    })
}
