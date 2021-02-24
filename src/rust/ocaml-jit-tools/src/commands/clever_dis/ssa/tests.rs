use std::fmt::Debug;

use expect_test::{expect, Expect};

use ocaml_jit_shared::{Comp, Primitive, RaiseKind};
use Instruction::*;

use crate::commands::clever_dis::data::BlockExit;

use super::*;

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
        state.push(SSAVar::Computed(0, 0));
        check_debug(&state.stack, expect![[r#"[Computed(0, 0)]"#]]);
        check_debug(&state.pick(0), expect![[r#"Computed(0, 0)"#]]);
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

        state.push(SSAVar::Computed(0, 0));
        state.push(SSAVar::Computed(0, 1));
        state.push(SSAVar::Computed(0, 2));
        state.push(SSAVar::Computed(0, 3));
        check_debug_pretty(
            &state,
            expect![[r#"
                State {
                    stack: [
                        Computed(
                            0,
                            0,
                        ),
                        Computed(
                            0,
                            1,
                        ),
                        Computed(
                            0,
                            2,
                        ),
                        Computed(
                            0,
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
            expect![[r#"State { stack: [Computed(0, 0), Computed(0, 1)], acc: PrevAcc, stack_start: 0 }"#]],
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
        state.assign(0, SSAVar::Computed(0, 12));
        check_debug(
            &state,
            expect![[r#"State { stack: [Computed(0, 12)], acc: PrevAcc, stack_start: 1 }"#]],
        );
    }

    // Assignments 2
    {
        let mut state = start_state;
        state.push(SSAVar::Computed(0, 12));
        check_debug(
            &state,
            expect![[r#"State { stack: [Computed(0, 12)], acc: PrevAcc, stack_start: 0 }"#]],
        );

        state.assign(0, SSAVar::Computed(0, 23));
        check_debug(
            &state,
            expect![[r#"State { stack: [Computed(0, 23)], acc: PrevAcc, stack_start: 0 }"#]],
        );

        state.assign(1, SSAVar::Computed(0, 24));
        check_debug(
            &state,
            expect![[r#"State { stack: [Computed(0, 24), Computed(0, 23)], acc: PrevAcc, stack_start: 1 }"#]],
        );

        state.assign(5, SSAVar::Computed(0, 25));
        check_debug_pretty(
            &state,
            expect![[r#"
                State {
                    stack: [
                        Computed(
                            0,
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
                            0,
                            24,
                        ),
                        Computed(
                            0,
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
        };

        let (ssa_block, final_state) = translate_block(&block, 0, is_entry_block).unwrap();
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
            <0_0> = global 310
            <0_1> = global 308
            <0_2> = <0_1>[1]
            <0_3> = apply <0_2> [<0_0>, <prev:0>]
            <0_4> = <prev:0> + 1
            Exit: jump 1

            Final acc: <0_4>
            End stack: ..., <prev:1> | <0_4>
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
            <0_0> = <prev:0>[0]
            <0_1> = apply <closure:0> [<0_0>]
            <0_2> = make block tag:2 vars:[<0_1>]
            Exit: return <0_2>

            Final acc: <0_2>
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
            <0_0> = make rec closure codes:[1] vars:[]
            <0_1> = make rec closure codes:[2] vars:[]
            <0_2> = make rec closure codes:[3] vars:[]
            <0_3> = make block tag:0 vars:[<0_1>, <0_0>, <0_2>]
            set global 12 = <0_3>
            Exit: jump 2

            Final acc: <unit>
            End stack: ..., <prev:0> | 
            Stack delta: -0/+0
        "#]],
    );

    check(
        vec![ClosureRec(vec![95, 96], 0)],
        BlockExit::UnconditionalJump(1),
        expect![[r#"
            <0_0> = make rec closure codes:[95, 96] vars:[]
            <0_1> = rec closure infix <0_0>[1]
            Exit: jump 1

            Final acc: <0_0>
            End stack: ..., <prev:0> | <0_0>, <0_1>
            Stack delta: -0/+2
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
            <0_0> = <prev:0>[1]
            <0_1> = <prev:0>[0]
            <0_2> = mul.f <0_0> <0_0>
            <0_3> = mul.f <0_1> <0_1>
            <0_4> = add.f <0_3> <0_2>
            <0_5> = sqrt.f <0_4>
            Exit: return <0_5>

            Final acc: <0_5>
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
            <0_0> = global 49
            <0_1> = global 50
            <0_2> = make float block [<0_1>, <0_0>]
            <0_3> = global 51
            set float <0_2>[1] = <0_3>
            <0_4> = global 52
            <0_5> = float <0_2>[1]
            <0_6> = ccall 304 [<0_5>, <0_4>]
            Exit: jump_if <0_6> t:6 f:5

            Final acc: <0_6>
            End stack: ..., <prev:0> | <prev:acc>, <0_2>, <unit>
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
            <0_0> = global 2
            <0_1> = make block tag:0 vars:[<0_0>, <prev:0>]
            Exit: raise <0_1>

            Final acc: <0_1>
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
            <0_0> = <prev:1>[1]
            <0_1> = <prev:1>[0]
            <0_2> = apply <env:1> [<0_1>]
            <0_3> = make block tag:0 vars:[<0_2>, <prev:0>]
            Exit: tail_apply <closure:0> [<0_3>, <0_0>]

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
            <0_0> = 2 >= <prev:0>
            Exit: jump_if <0_0> t:1 f:2

            Final acc: <prev:0>
            End stack: ..., <prev:0> | 
            Stack delta: -0/+0
        "#]],
    );

    // Test traps
    check(
        vec![Acc(0), PushTrap(2)],
        BlockExit::PushTrap { normal: 1, trap: 2 },
        expect![[r#"
            Exit: push trap normal:1 trap:2

            Final acc: <prev:0>
            End stack: ..., <prev:0> | <special>, <special>, <special>, <special>
            Stack delta: -0/+4
        "#]],
    );

    // Test traps
    check(
        vec![PopTrap],
        BlockExit::UnconditionalJump(1),
        expect![[r#"
            pop trap
            Exit: jump 1

            Final acc: <prev:acc>
            End stack: ..., <prev:4> | 
            Stack delta: -4/+0
        "#]],
    );

    // BoolNot
    check(
        vec![BoolNot],
        BlockExit::UnconditionalJump(1),
        expect![[r#"
            <0_0> = not <prev:acc>
            Exit: jump 1

            Final acc: <0_0>
            End stack: ..., <prev:0> | 
            Stack delta: -0/+0
        "#]],
    );

    // IsInt
    check(
        vec![IsInt],
        BlockExit::UnconditionalJump(1),
        expect![[r#"
            <0_0> = is_int <prev:acc>
            Exit: jump 1

            Final acc: <0_0>
            End stack: ..., <prev:0> | 
            Stack delta: -0/+0
        "#]],
    );

    // SetBytesChar
    check(
        vec![
            Const(255),
            Push,
            Acc(2),
            ArithInt(ArithOp::And),
            Push,
            Acc(1),
            Push,
            Acc(5),
            GetField(0),
            SetBytesChar,
        ],
        BlockExit::UnconditionalJump(1),
        expect![[r#"
            <0_0> = <prev:1> & 255
            <0_1> = <prev:3>[0]
            set bytes <0_1>[<prev:0>] = <0_0>
            Exit: jump 1

            Final acc: <unit>
            End stack: ..., <prev:0> | 
            Stack delta: -0/+0
        "#]],
    );

    // GetBytesChar
    check(
        vec![
            Acc(2),
            Push,
            Acc(2),
            Push,
            Acc(2),
            GetBytesChar,
            IntCmp(Comp::Eq),
            BranchIfNot(3),
        ],
        BlockExit::ConditionalJump(3, 4),
        expect![[r#"
            <0_0> = bytes <prev:0>[<prev:1>]
            <0_1> = <0_0> == <prev:2>
            Exit: jump_if <0_1> t:4 f:3

            Final acc: <0_1>
            End stack: ..., <prev:0> | 
            Stack delta: -0/+0
        "#]],
    );

    // OffsetRef
    check(
        vec![
            Const(92),
            Push,
            Acc(5),
            GetField(0),
            Push,
            Acc(5),
            SetBytesChar,
            Acc(4),
            OffsetRef(1),
            Const(98),
            Push,
            Acc(5),
            GetField(0),
            Push,
            Acc(5),
            SetBytesChar,
            Branch(31),
        ],
        BlockExit::UnconditionalJump(31),
        expect![[r#"
            <0_0> = <prev:4>[0]
            set bytes <prev:3>[<0_0>] = 92
            <0_1> = <prev:4>[0]
            <0_2> = <0_1> + 1
            set <prev:4>[0] = <0_2>
            <0_3> = <prev:4>[0]
            set bytes <prev:3>[<0_3>] = 98
            Exit: jump 31

            Final acc: <unit>
            End stack: ..., <prev:0> | 
            Stack delta: -0/+0
        "#]],
    );

    // GetDynMet
    check_advanced(
        vec![
            EnvAcc(3),
            Push,
            EnvAcc(2),
            Push,
            EnvAcc(1),
            GetDynMet,
            ApplyTerm(2, 3),
        ],
        true,
        BlockExit::TailCall,
        expect![[r#"
            <0_0> = get dynmet tag:<env:1> object:<env:2> 
            Exit: tail_apply <0_0> [<env:3>, <arg:0>]

            Final acc: <0_0>
            End stack: ..., <prev:1> | 
            Stack delta: -1/+0
        "#]],
    );

    // PubMet
    check(
        vec![
            Pop(2),
            Const(0),
            Push,
            Acc(1),
            GetField(0),
            Apply1,
            SetupForPubMet(109),
            GetDynMet,
            Apply1,
            Push,
        ],
        BlockExit::UnconditionalJump(1),
        expect![[r#"
            <0_0> = <prev:2>[0]
            <0_1> = apply <0_0> [0]
            <0_2> = get dynmet tag:109 object:<0_1> 
            <0_3> = apply <0_2> [<prev:2>]
            Exit: jump 1

            Final acc: <0_3>
            End stack: ..., <prev:3> | <0_3>
            Stack delta: -3/+1
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
            <0_0> = global 89
            <0_1> = global 89
            <0_2> = <0_1>[32]
            <0_3> = make block tag:0 vars:[<0_2>]
            <0_4> = global 100
            <0_5> = <0_4>[0]
            <0_6> = global 100
            <0_7> = <0_6>[0]
            <0_8> = apply <0_7> [<0_3>]
            <0_9> = make closure code:407 vars:[]
            <0_10> = make block tag:0 vars:[<0_9>]
            <0_11> = global 100
            <0_12> = <0_11>[0]
            <0_13> = global 100
            <0_14> = <0_13>[0]
            <0_15> = apply <0_14> [<0_10>]
            <0_16> = make closure code:408 vars:[]
            <0_17> = make closure code:409 vars:[]
            <0_18> = apply <0_16> [0]
            <0_19> = apply <0_17> [0]
            <0_20> = make block tag:0 vars:[<0_8>, <0_15>, <0_16>, <0_17>]
            set global 311 = <0_20>
            <0_21> = global 45
            <0_22> = <0_21>[102]
            <0_23> = apply <0_22> [0]
            set global 312 = <atom:0>
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
            Exit: stop <arg:0>

            Final acc: <arg:0>
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
            Exit: jump_if <arg:1> t:2 f:1

            Final acc: <arg:1>
            End stack: ..., <prev:0> | <arg:1>, <arg:0>
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
            <0_0> = make closure code:405 vars:[]
            <0_1> = global 301
            <0_2> = make block tag:0 vars:[<0_1>, 71]
            <0_3> = apply <0_0> [<0_2>]
            <0_4> = global 302
            set <0_2>[0] = <0_4>
            set <0_2>[1] = 12
            <0_5> = apply <0_0> [<0_2>]
            <0_6> = make block tag:0 vars:[<0_0>]
            set global 303 = <0_6>
            <0_7> = global 45
            <0_8> = <0_7>[102]
            <0_9> = apply <0_8> [0]
            set global 304 = <atom:0>
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
            <0_0> = make closure code:88 vars:[]
            <0_1> = make closure code:89 vars:[]
            <0_2> = make closure code:90 vars:[]
            <0_3> = make closure code:91 vars:[]
            <0_4> = make closure code:92 vars:[]
            <0_5> = apply <0_0> [0]
            <0_6> = apply <0_1> [0, 1]
            <0_7> = apply <0_2> [0, 1, 2]
            <0_8> = apply <0_3> [0, 1, 2, 3]
            Exit: jump 4

            Final acc: <0_8>
            End stack: ..., <prev:0> | <0_0>, <0_1>, <0_2>, <0_3>, <0_4>
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
            <0_0> = apply <prev:0> [0, 1, 2, 3, 4]
            Exit: jump 5

            Final acc: <0_0>
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
            <0_0> = make block tag:0 vars:[<prev:4>, <prev:3>, <prev:2>, <prev:1>, <prev:0>]
            set global 46 = <0_0>
            <0_1> = global 45
            <0_2> = <0_1>[102]
            <0_3> = apply <0_2> [0]
            set global 47 = <atom:0>
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
            <0_0> = 1 * 2
            <0_1> = 1 / 2
            <0_2> = 1 % 2
            <0_3> = 1 | 2
            <0_4> = 1 & 2
            <0_5> = 1 ^ 2
            <0_6> = 1 << 2
            <0_7> = 1 l>> 2
            <0_8> = 1 a>> 2
            <0_9> = 1 == 1
            <0_10> = 1 != 2
            <0_11> = 1 < 2
            <0_12> = 1 > 2
            <0_13> = 1 <= 2
            <0_14> = 2 >= 1
            <0_15> = 2 u< 1
            <0_16> = 1 u>= <0_15>
            <0_17> = neg int 1
            Exit: jump 3

            Final acc: <0_17>
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
            Exit: switch <arg:0> ints:[1] blocks:[
                2, 3, 4, 5, 6, 7, 8, 9,
                10, 11, 12, 13, 14, 15, 16,
            ]

            Final acc: <arg:0>
            End stack: ..., <prev:0> | <arg:1>, <arg:0>
            Stack delta: -0/+2
        "#]],
    );
}
