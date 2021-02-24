use crate::commands::clever_dis::ssa::data::SSAVar;
use crate::commands::clever_dis::ssa::SSAStackState;
use expect_test::{expect, Expect};
use std::fmt::Debug;

fn check_debug<T: Debug>(thing: &T, expected: Expect) {
    expected.assert_eq(&format!("{:?}", thing));
}

fn check_debug_pretty<T: Debug>(thing: &T, expected: Expect) {
    expected.assert_eq(&format!("{:#?}", thing));
}

#[test]
fn test_state() {
    let start_state = SSAStackState {
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
            expect![[r#"SSAStackState { stack: [], acc: PrevAcc, stack_start: 0 }"#]],
        );

        state.pop(3);
        check_debug(
            &state,
            expect![[r#"SSAStackState { stack: [], acc: PrevAcc, stack_start: 3 }"#]],
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
                SSAStackState {
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
            expect![[
                r#"SSAStackState { stack: [Computed(0, 0), Computed(0, 1)], acc: PrevAcc, stack_start: 0 }"#
            ]],
        );

        state.pop(3);
        check_debug(
            &state,
            expect![[r#"SSAStackState { stack: [], acc: PrevAcc, stack_start: 1 }"#]],
        );
    }
    // Assignments 1
    {
        let mut state = start_state.clone();
        state.assign(0, SSAVar::Computed(0, 12));
        check_debug(
            &state,
            expect![[
                r#"SSAStackState { stack: [Computed(0, 12)], acc: PrevAcc, stack_start: 1 }"#
            ]],
        );
    }

    // Assignments 2
    {
        let mut state = start_state;
        state.push(SSAVar::Computed(0, 12));
        check_debug(
            &state,
            expect![[
                r#"SSAStackState { stack: [Computed(0, 12)], acc: PrevAcc, stack_start: 0 }"#
            ]],
        );

        state.assign(0, SSAVar::Computed(0, 23));
        check_debug(
            &state,
            expect![[
                r#"SSAStackState { stack: [Computed(0, 23)], acc: PrevAcc, stack_start: 0 }"#
            ]],
        );

        state.assign(1, SSAVar::Computed(0, 24));
        check_debug(
            &state,
            expect![[
                r#"SSAStackState { stack: [Computed(0, 24), Computed(0, 23)], acc: PrevAcc, stack_start: 1 }"#
            ]],
        );

        state.assign(5, SSAVar::Computed(0, 25));
        check_debug_pretty(
            &state,
            expect![[r#"
                SSAStackState {
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
