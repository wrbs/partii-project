use super::*;
use crate::commands::clever_dis::data::BlockExit;
use expect_test::{expect, Expect};

use ocaml_jit_shared::Primitive;
use std::fmt::Debug;
use Instruction::*;

fn check_debug<T: Debug>(thing: &T, expected: Expect) {
    expected.assert_eq(&format!("{:?}", thing));
}

fn check_debug_pretty<T: Debug>(thing: &T, expected: Expect) {
    expected.assert_eq(&format!("{:#?}", thing));
}

#[test]
fn test_state() {
    let start_state = State {
        stack: vec![],
        acc: SSAVar::PrevAcc,
        stack_start: 0,
    };

    // Basic pick behaviour
    {
        let mut state = start_state.clone();

        // Make sure picking has the correct behaviour
        check_debug(&state.stack, expect![[r#"[]"#]]);
        check_debug(&state.pick(0), expect![[r#"PrevStack(0)"#]]);
        check_debug(&state.pick(1), expect![[r#"PrevStack(1)"#]]);
        check_debug(&state.pick(2), expect![[r#"PrevStack(2)"#]]);

        // Push something
        state.push(SSAVar::Computed(0));
        check_debug(&state.stack, expect![[r#"[Computed(0)]"#]]);
        check_debug(&state.pick(0), expect![[r#"Computed(0)"#]]);
        check_debug(&state.pick(1), expect![[r#"PrevStack(0)"#]]);
        check_debug(&state.pick(2), expect![[r#"PrevStack(1)"#]]);
    }

    // Popping with a completely empty stack
    {
        let mut state = start_state.clone();

        check_debug(
            &state,
            expect![[r#"State { stack: [], acc: PrevAcc, stack_start: 0 }"#]],
        );

        state.pop(3);
        check_debug(
            &state,
            expect![[r#"State { stack: [], acc: PrevAcc, stack_start: 3 }"#]],
        );
        check_debug(&state.pick(0), expect![[r#"PrevStack(3)"#]]);
        check_debug(&state.pick(1), expect![[r#"PrevStack(4)"#]]);
        check_debug(&state.pick(2), expect![[r#"PrevStack(5)"#]]);
    }

    // Push a few before popping from the pushed things
    {
        let mut state = start_state.clone();

        state.push(SSAVar::Computed(0));
        state.push(SSAVar::Computed(1));
        state.push(SSAVar::Computed(2));
        state.push(SSAVar::Computed(3));
        check_debug_pretty(
            &state,
            expect![[r#"
                State {
                    stack: [
                        Computed(
                            0,
                        ),
                        Computed(
                            1,
                        ),
                        Computed(
                            2,
                        ),
                        Computed(
                            3,
                        ),
                    ],
                    acc: PrevAcc,
                    stack_start: 0,
                }"#]],
        );

        state.pop(2);
        check_debug(
            &state,
            expect![[
                r#"State { stack: [Computed(0), Computed(1)], acc: PrevAcc, stack_start: 0 }"#
            ]],
        );

        state.pop(3);
        check_debug(
            &state,
            expect![[r#"State { stack: [], acc: PrevAcc, stack_start: 1 }"#]],
        );
    }
    // Assignments 1
    {
        let mut state = start_state.clone();
        state.assign(0, SSAVar::Computed(12));
        check_debug(
            &state,
            expect![[r#"State { stack: [Computed(12)], acc: PrevAcc, stack_start: 1 }"#]],
        );
    }

    // Assignments 2
    {
        let mut state = start_state.clone();
        state.push(SSAVar::Computed(12));
        check_debug(
            &state,
            expect![[r#"State { stack: [Computed(12)], acc: PrevAcc, stack_start: 0 }"#]],
        );

        state.assign(0, SSAVar::Computed(23));
        check_debug(
            &state,
            expect![[r#"State { stack: [Computed(23)], acc: PrevAcc, stack_start: 0 }"#]],
        );

        state.assign(1, SSAVar::Computed(24));
        check_debug(
            &state,
            expect![[
                r#"State { stack: [Computed(24), Computed(23)], acc: PrevAcc, stack_start: 1 }"#
            ]],
        );

        state.assign(5, SSAVar::Computed(25));
        check_debug_pretty(
            &state,
            expect![[r#"
                State {
                    stack: [
                        Computed(
                            25,
                        ),
                        PrevStack(
                            4,
                        ),
                        PrevStack(
                            3,
                        ),
                        PrevStack(
                            2,
                        ),
                        Computed(
                            24,
                        ),
                        Computed(
                            23,
                        ),
                    ],
                    acc: PrevAcc,
                    stack_start: 5,
                }"#]],
        );
    }
}

#[test]
fn test_block_translation() {
    fn check_advanced(
        instructions: Vec<Instruction<usize>>,
        is_entry_block: bool,
        exit: BlockExit,
        expected: Expect,
    ) {
        let block = Block {
            instructions,
            exit,
            closures: vec![],
            traps: vec![],
        };

        let (ssa_block, final_state) = translate_block(&block, is_entry_block);
        let actual = format!("{}\n{}", ssa_block, final_state);
        expected.assert_eq(&actual);
    }

    fn check(instructions: Vec<Instruction<usize>>, exit: BlockExit, expected: Expect) {
        check_advanced(instructions, false, exit, expected);
    }

    check(
        vec![
            CheckSignals,
            Acc(0),
            Push,
            GetGlobal(310),
            Push,
            GetGlobal(308),
            GetField(1),
            Apply2,
            Acc(0),
            OffsetInt(1),
            Assign(0),
        ],
        BlockExit::UnconditionalJump(1),
        expect![[r#"
            check signals
            v0 = g310
            v1 = g308
            v2 = v1[1]
            v3 = apply v2 [v0, <prev:0>]
            v4 = <prev:0> + 1
            Exit: jump 1

            Final acc: v4
            End stack: ..., <prev:1> | v4
            Stack delta: -1/+1
        "#]],
    );

    check(
        vec![
            Acc(0),
            GetField(0),
            Push,
            OffsetClosure(0),
            Apply1,
            MakeBlock(1, 2),
            Return(2),
        ],
        BlockExit::Return,
        expect![[r#"
            v0 = <prev:0>[0]
            v1 = apply <closure:0> [v0]
            v2 = make block tag:2 vars:[v1]
            Exit: return v2

            Final acc: v2
            End stack: ..., <prev:2> | 
            Stack delta: -2/+0
        "#]],
    );

    check(
        vec![
            ClosureRec(vec![1], 0),
            ClosureRec(vec![2], 0),
            ClosureRec(vec![3], 0),
            Acc(0),
            Push,
            Acc(3),
            Push,
            Acc(3),
            MakeBlock(3, 0),
            Pop(3),
            SetGlobal(12),
            Branch(2),
        ],
        BlockExit::UnconditionalJump(2),
        expect![[r#"
            v0 = make rec closure codes:[1] vars:[]
            v1 = make rec closure codes:[2] vars:[]
            v2 = make rec closure codes:[3] vars:[]
            v3 = make block tag:0 vars:[v1, v0, v2]
            set g12 = v3
            Exit: jump 2

            Final acc: <unit>
            End stack: ..., <prev:0> | 
            Stack delta: -0/+0
        "#]],
    );

    check(
        vec![
            Acc(0),
            GetField(1),
            Push,
            Acc(1),
            GetField(0),
            Push,
            Acc(1),
            Push,
            Acc(2),
            Prim(Primitive::MulFloat),
            Push,
            Acc(1),
            Push,
            Acc(2),
            Prim(Primitive::MulFloat),
            Prim(Primitive::AddFloat),
            Prim(Primitive::SqrtFloat),
            Return(3),
        ],
        BlockExit::Return,
        expect![[r#"
            v0 = <prev:0>[1]
            v1 = <prev:0>[0]
            v2 = mul.f v0 v0
            v3 = mul.f v1 v1
            v4 = add.f v3 v2
            v5 = sqrt.f v4
            Exit: return v5

            Final acc: v5
            End stack: ..., <prev:1> | 
            Stack delta: -1/+0
        "#]],
    );

    // Float blocks
    check(
        vec![
            Push,
            GetGlobal(49),
            Push,
            GetGlobal(50),
            MakeFloatBlock(2),
            Push,
            GetGlobal(51),
            Push,
            Acc(1),
            SetFloatField(1),
            Push,
            GetGlobal(52),
            Push,
            Acc(2),
            GetFloatField(1),
            CCall2(304),
            BranchIfNot(5),
        ],
        BlockExit::ConditionalJump(5, 6),
        expect![[r#"
            v0 = g49
            v1 = g50
            v2 = make float block [v1, v0]
            v3 = g51
            set float v2[1] = v3
            v4 = g52
            v5 = float v2[1]
            v6 = ccall 304 [v5, v4]
            Exit: jump_if v6 t:6 f:5

            Final acc: v6
            End stack: ..., <prev:0> | <prev:acc>, v2, <unit>
            Stack delta: -0/+3
        "#]],
    );

    // Stdlib.failwith
    check(
        vec![
            Acc(0),
            Push,
            GetGlobal(2),
            MakeBlock(2, 0),
            Raise(RaiseKind::Regular),
        ],
        BlockExit::Raise,
        expect![[r#"
            v0 = g2
            v1 = make block tag:0 vars:[v0, <prev:0>]
            Exit: raise v1

            Final acc: v1
            End stack: ..., <prev:0> | 
            Stack delta: -0/+0
        "#]],
    );

    // from List.revmap
    check(
        vec![
            Acc(1),
            GetField(1),
            Push,
            Acc(2),
            GetField(0),
            Push,
            Acc(1),
            Push,
            Acc(3),
            Push,
            Acc(2),
            Push,
            EnvAcc(1),
            Apply1,
            MakeBlock(2, 0),
            Push,
            OffsetClosure(0),
            ApplyTerm(2, 6),
        ],
        BlockExit::TailCall,
        expect![[r#"
            v0 = <prev:1>[1]
            v1 = <prev:1>[0]
            v2 = apply <env:1> [v1]
            v3 = make block tag:0 vars:[v2, <prev:0>]
            Exit: tail_apply <closure:0> [v3, v0]

            Final acc: <closure:0>
            End stack: ..., <prev:2> | 
            Stack delta: -2/+0
        "#]],
    );

    // Test BranchCmp
    check(
        vec![Acc(0), BranchCmp(Comp::Ge, 2, 1)],
        BlockExit::ConditionalJump(1, 2),
        expect![[r#"
            v0 = 2 >= <prev:0>
            Exit: jump_if v0 t:1 f:2

            Final acc: <prev:0>
            End stack: ..., <prev:0> | 
            Stack delta: -0/+0
        "#]],
    );

    // Monster case - pervasives
    check(
        vec![
            GetGlobal(89),
            Push,
            GetGlobal(89),
            GetField(32),
            MakeBlock(1, 0),
            Pop(1),
            Push,
            GetGlobal(100),
            GetField(0),
            Push,
            GetGlobal(100),
            GetField(0),
            Pop(1),
            Apply1,
            Push,
            Closure(407, 0),
            Push,
            Acc(0),
            MakeBlock(1, 0),
            Pop(1),
            Push,
            GetGlobal(100),
            GetField(0),
            Push,
            GetGlobal(100),
            GetField(0),
            Pop(1),
            Apply1,
            Push,
            Closure(408, 0),
            Push,
            Closure(409, 0),
            Push,
            Const(0),
            Push,
            Acc(2),
            Apply1,
            Const(0),
            Push,
            Acc(1),
            Apply1,
            Push,
            Acc(1),
            Push,
            Acc(3),
            Push,
            Acc(5),
            Push,
            Acc(7),
            MakeBlock(4, 0),
            Pop(5),
            SetGlobal(311),
            Const(0),
            Push,
            GetGlobal(45),
            GetField(102),
            Apply1,
            MakeBlock(0, 0),
            SetGlobal(312),
            Stop,
        ],
        BlockExit::Stop,
        expect![[r#"
            v0 = g89
            v1 = g89
            v2 = v1[32]
            v3 = make block tag:0 vars:[v2]
            v4 = g100
            v5 = v4[0]
            v6 = g100
            v7 = v6[0]
            v8 = apply v7 [v3]
            v9 = make closure code:407 vars:[]
            v10 = make block tag:0 vars:[v9]
            v11 = g100
            v12 = v11[0]
            v13 = g100
            v14 = v13[0]
            v15 = apply v14 [v10]
            v16 = make closure code:408 vars:[]
            v17 = make closure code:409 vars:[]
            v18 = apply v16 [0]
            v19 = apply v17 [0]
            v20 = make block tag:0 vars:[v8, v15, v16, v17]
            set g311 = v20
            v21 = g45
            v22 = v21[102]
            v23 = apply v22 [0]
            set g312 = <atom:0>
            Exit: stop <unit>

            Final acc: <unit>
            End stack: ..., <prev:0> | 
            Stack delta: -0/+0
        "#]],
    );

    // Block entries
    // no grab
    check_advanced(
        vec![Acc(0), Pop(1), Stop],
        true,
        BlockExit::Stop,
        expect![[r#"
            Exit: stop a0

            Final acc: a0
            End stack: ..., <prev:0> | 
            Stack delta: -0/+0
        "#]],
    );

    // with grab
    check_advanced(
        vec![Grab(1), Acc(1), BranchIfNot(1)],
        true,
        BlockExit::ConditionalJump(1, 2),
        expect![[r#"
            grab 1
            Exit: jump_if a1 t:2 f:1

            Final acc: a1
            End stack: ..., <prev:0> | a1, a0
            Stack delta: -0/+2
        "#]],
    );

    // mutable_fields.ml
    check(
        vec![
            Closure(405, 0),
            Push,
            Const(71),
            Push,
            GetGlobal(301),
            MakeBlock(2, 0),
            Push,
            Acc(0),
            Push,
            Acc(2),
            Apply1,
            GetGlobal(302),
            Push,
            Acc(1),
            SetField(0),
            Const(12),
            Push,
            Acc(1),
            SetField(1),
            Acc(0),
            Push,
            Acc(2),
            Apply1,
            Pop(1),
            Push,
            Acc(1),
            MakeBlock(1, 0),
            Pop(2),
            SetGlobal(303),
            Const(0),
            Push,
            GetGlobal(45),
            GetField(102),
            Apply1,
            MakeBlock(0, 0),
            SetGlobal(304),
            Stop,
        ],
        BlockExit::Stop,
        expect![[r#"
            v0 = make closure code:405 vars:[]
            v1 = g301
            v2 = make block tag:0 vars:[v1, 71]
            v3 = apply v0 [v2]
            v4 = g302
            set v2[0] = v4
            set v2[1] = 12
            v5 = apply v0 [v2]
            v6 = make block tag:0 vars:[v0]
            set g303 = v6
            v7 = g45
            v8 = v7[102]
            v9 = apply v8 [0]
            set g304 = <atom:0>
            Exit: stop <unit>

            Final acc: <unit>
            End stack: ..., <prev:0> | 
            Stack delta: -0/+0
        "#]],
    );

    // Check function calls are ok
    check(
        vec![
            Closure(88, 0),
            Push,
            Closure(89, 0),
            Push,
            Closure(90, 0),
            Push,
            Closure(91, 0),
            Push,
            Closure(92, 0),
            Push,
            Const(0),
            Push,
            Acc(5),
            Apply1,
            Const(1),
            Push,
            Const(0),
            Push,
            Acc(5),
            Apply2,
            Const(2),
            Push,
            Const(1),
            Push,
            Const(0),
            Push,
            Acc(5),
            Apply3,
            PushRetAddr(4),
            Const(3),
            Push,
            Const(2),
            Push,
            Const(1),
            Push,
            Const(0),
            Push,
            Acc(8),
            Apply(4),
        ],
        BlockExit::UnconditionalJump(4),
        expect![[r#"
            v0 = make closure code:88 vars:[]
            v1 = make closure code:89 vars:[]
            v2 = make closure code:90 vars:[]
            v3 = make closure code:91 vars:[]
            v4 = make closure code:92 vars:[]
            v5 = apply v0 [0]
            v6 = apply v1 [0, 1]
            v7 = apply v2 [0, 1, 2]
            v8 = apply v3 [0, 1, 2, 3]
            Exit: jump 4

            Final acc: v8
            End stack: ..., <prev:0> | v0, v1, v2, v3, v4
            Stack delta: -0/+5
        "#]],
    );
    check(
        vec![
            PushRetAddr(5),
            Const(4),
            Push,
            Const(3),
            Push,
            Const(2),
            Push,
            Const(1),
            Push,
            Const(0),
            Push,
            Acc(8),
            Apply(5),
        ],
        BlockExit::UnconditionalJump(5),
        expect![[r#"
            v0 = apply <prev:0> [0, 1, 2, 3, 4]
            Exit: jump 5

            Final acc: v0
            End stack: ..., <prev:0> | 
            Stack delta: -0/+0
        "#]],
    );
    check(
        vec![
            Push,
            Acc(1),
            Push,
            Acc(3),
            Push,
            Acc(5),
            Push,
            Acc(7),
            Push,
            Acc(9),
            MakeBlock(5, 0),
            Pop(6),
            SetGlobal(46),
            Const(0),
            Push,
            GetGlobal(45),
            GetField(102),
            Apply1,
            MakeBlock(0, 0),
            SetGlobal(47),
            Stop,
        ],
        BlockExit::Stop,
        expect![[r#"
            v0 = make block tag:0 vars:[<prev:4>, <prev:3>, <prev:2>, <prev:1>, <prev:0>]
            set g46 = v0
            v1 = g45
            v2 = v1[102]
            v3 = apply v2 [0]
            set g47 = <atom:0>
            Exit: stop <unit>

            Final acc: <unit>
            End stack: ..., <prev:5> | 
            Stack delta: -5/+0
        "#]],
    );

    // Integer ops
    check(
        vec![
            Const(2),
            Push,
            Const(1),
            ArithInt(ArithOp::Mul),
            Const(2),
            Push,
            Const(1),
            ArithInt(ArithOp::Div),
            Const(2),
            Push,
            Const(1),
            ArithInt(ArithOp::Mod),
            Const(2),
            Push,
            Const(1),
            ArithInt(ArithOp::Or),
            Const(2),
            Push,
            Const(1),
            ArithInt(ArithOp::And),
            Const(2),
            Push,
            Const(1),
            ArithInt(ArithOp::Xor),
            Const(2),
            Push,
            Const(1),
            ArithInt(ArithOp::Lsl),
            Const(2),
            Push,
            Const(1),
            ArithInt(ArithOp::Lsr),
            Const(2),
            Push,
            Const(1),
            ArithInt(ArithOp::Asr),
            Const(1),
            Push,
            Const(1),
            IntCmp(Comp::Eq),
            Const(2),
            Push,
            Const(1),
            IntCmp(Comp::Ne),
            Const(2),
            Push,
            Const(1),
            IntCmp(Comp::Lt),
            Const(2),
            Push,
            Const(1),
            IntCmp(Comp::Gt),
            Const(2),
            Push,
            Const(1),
            IntCmp(Comp::Le),
            Const(1),
            Push,
            Const(2),
            IntCmp(Comp::Ge),
            Const(1),
            Push,
            Const(2),
            IntCmp(Comp::ULt),
            Push,
            Const(1),
            IntCmp(Comp::UGe),
            Const(1),
            Push,
            Acc(0),
            NegInt,
        ],
        BlockExit::UnconditionalJump(3),
        expect![[r#"
            v0 = 1 * 2
            v1 = 1 / 2
            v2 = 1 % 2
            v3 = 1 | 2
            v4 = 1 & 2
            v5 = 1 ^ 2
            v6 = 1 << 2
            v7 = 1 l>> 2
            v8 = 1 a>> 2
            v9 = 1 == 1
            v10 = 1 != 2
            v11 = 1 < 2
            v12 = 1 > 2
            v13 = 1 <= 2
            v14 = 2 >= 1
            v15 = 2 u< 1
            v16 = 1 u>= v15
            v17 = - 1
            Exit: jump 3

            Final acc: v17
            End stack: ..., <prev:0> | 1
            Stack delta: -0/+1
        "#]],
    );

    // Switches - start of CamlinternalFormatBasics.concat_fmtty
    check_advanced(
        vec![Grab(1), Acc(0), Switch(vec![1], (2..=16).collect())],
        true,
        BlockExit::Switch {
            ints: vec![1],
            blocks: (2..=16).collect(),
        },
        expect![[r#"
            grab 1
            Exit: switch a0 ints:[1] blocks:[
                2, 3, 4, 5, 6, 7, 8, 9,
                10, 11, 12, 13, 14, 15, 16,
            ]

            Final acc: a0
            End stack: ..., <prev:0> | a1, a0
            Stack delta: -0/+2
        "#]],
    );
}
