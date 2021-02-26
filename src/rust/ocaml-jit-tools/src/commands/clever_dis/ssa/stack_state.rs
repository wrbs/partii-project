use crate::commands::clever_dis::ssa::data::{ModifySSAVars, SSASubstitutionTarget, SSAVar};
use itertools::Itertools;
use std::cmp::max;
use std::collections::HashSet;
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, PartialEq)]
pub struct SSAStackState {
    stack: Vec<SSAVar>,
    accu: SSAVar,
    stack_start: usize,
    pub used_prev: HashSet<SSASubstitutionTarget>,
}

impl SSAStackState {
    pub fn new() -> SSAStackState {
        SSAStackState {
            stack: vec![],
            accu: SSAVar::Prev(SSASubstitutionTarget::Acc),
            stack_start: 0,
            used_prev: HashSet::new(),
        }
    }

    pub fn get_subst(&mut self, t: SSASubstitutionTarget) -> SSAVar {
        let v = match t {
            SSASubstitutionTarget::Stack(i) => {
                self.ensure_capacity_for(i);
                self.stack[self.stack.len() - 1 - i]
            }
            SSASubstitutionTarget::Acc => self.accu,
        };

        if let SSAVar::Prev(t) = v {
            self.used_prev.insert(t);
        }

        v
    }

    pub fn pick(&mut self, n: usize) -> SSAVar {
        self.get_subst(SSASubstitutionTarget::Stack(n))
    }

    pub fn accu(&mut self) -> SSAVar {
        self.get_subst(SSASubstitutionTarget::Acc)
    }

    #[inline(always)]
    pub fn set_accu(&mut self, value: SSAVar) {
        self.accu = value;
    }

    pub fn pop(&mut self, count: usize) {
        let to_keep_in_stack = max(self.stack.len() as isize - count as isize, 0) as usize;
        let to_remove_from_stack = self.stack.len() - to_keep_in_stack;
        self.stack.truncate(to_keep_in_stack);

        let remaining = count - to_remove_from_stack;
        self.stack_start += remaining;
    }

    pub fn push(&mut self, entry: SSAVar) {
        self.stack.push(entry);
    }

    pub fn assign(&mut self, index: usize, entry: SSAVar) {
        self.ensure_capacity_for(index);

        let length = self.stack.len();
        self.stack[length - 1 - index] = entry;
    }

    fn ensure_capacity_for(&mut self, index: usize) {
        if index >= self.stack.len() {
            let todo = index - self.stack.len() + 1;
            self.stack_start += todo;
            let mut tmp_stack = vec![];
            for i in 0..todo {
                tmp_stack.push(SSAVar::Prev(SSASubstitutionTarget::Stack(
                    self.stack_start - i - 1,
                )));
            }

            std::mem::swap(&mut self.stack, &mut tmp_stack);
            self.stack.extend(tmp_stack);
        }
    }

    pub fn delta(&self) -> isize {
        return self.stack.len() as isize - self.stack_start as isize;
    }
}

impl ModifySSAVars for SSAStackState {
    fn modify_ssa_vars<F>(&mut self, f: &mut F)
    where
        F: FnMut(&mut SSAVar),
    {
        f(&mut self.accu);
        self.stack.iter_mut().for_each(f);
    }
}

impl Display for SSAStackState {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        writeln!(f, "Final accu: {}", self.accu)?;

        write!(f, "End stack: ..., <prev:{}> | ", self.stack_start)?;

        let mut first = true;

        for entry in &self.stack {
            if first {
                first = false
            } else {
                write!(f, ", ")?;
            }
            write!(f, "{}", entry)?;
        }
        writeln!(f)?;

        writeln!(
            f,
            "Used prev: {:?}",
            self.used_prev.iter().sorted().collect::<Vec<_>>()
        )?;

        writeln!(
            f,
            "Stack delta: -{}/+{}",
            self.stack_start,
            self.stack.len()
        )?;

        Ok(())
    }
}

#[cfg(test)]
mod test {

    use super::SSAStackState;
    use crate::commands::clever_dis::ssa::data::SSAVar;
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
        let start_state = SSAStackState::new();

        // Basic pick behaviour
        {
            let mut state = start_state.clone();

            // Make sure picking has the correct behaviour
            check_debug(&state.stack, expect![[r#"[]"#]]);
            check_debug(&state.pick(0), expect![[r#"Prev(Stack(0))"#]]);
            check_debug(&state.pick(1), expect![[r#"Prev(Stack(1))"#]]);
            check_debug(&state.pick(2), expect![[r#"Prev(Stack(2))"#]]);

            // Push something
            state.push(SSAVar::Computed(0, 0));
            check_debug(
                &state.stack,
                expect![[r#"[Prev(Stack(2)), Prev(Stack(1)), Prev(Stack(0)), Computed(0, 0)]"#]],
            );
            check_debug(&state.pick(0), expect![[r#"Computed(0, 0)"#]]);
            check_debug(&state.pick(1), expect![[r#"Prev(Stack(0))"#]]);
            check_debug(&state.pick(2), expect![[r#"Prev(Stack(1))"#]]);
        }

        // Popping with a completely empty stack
        {
            let mut state = start_state.clone();

            check_debug(
                &state,
                expect![[
                    r#"SSAStackState { stack: [], accu: Prev(Acc), stack_start: 0, used_prev: {} }"#
                ]],
            );

            state.pop(3);
            check_debug(
                &state,
                expect![[
                    r#"SSAStackState { stack: [], accu: Prev(Acc), stack_start: 3, used_prev: {} }"#
                ]],
            );
            check_debug(&state.pick(0), expect![[r#"Prev(Stack(3))"#]]);
            check_debug(&state.pick(1), expect![[r#"Prev(Stack(4))"#]]);
            check_debug(&state.pick(2), expect![[r#"Prev(Stack(5))"#]]);
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
                    accu: Prev(
                        Acc,
                    ),
                    stack_start: 0,
                    used_prev: {},
                }"#]],
            );

            state.pop(2);
            check_debug(
                &state,
                expect![[
                    r#"SSAStackState { stack: [Computed(0, 0), Computed(0, 1)], accu: Prev(Acc), stack_start: 0, used_prev: {} }"#
                ]],
            );

            state.pop(3);
            check_debug(
                &state,
                expect![[
                    r#"SSAStackState { stack: [], accu: Prev(Acc), stack_start: 1, used_prev: {} }"#
                ]],
            );
        }
        // Assignments 1
        {
            let mut state = start_state.clone();
            state.assign(0, SSAVar::Computed(0, 12));
            check_debug(
                &state,
                expect![[
                    r#"SSAStackState { stack: [Computed(0, 12)], accu: Prev(Acc), stack_start: 1, used_prev: {} }"#
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
                    r#"SSAStackState { stack: [Computed(0, 12)], accu: Prev(Acc), stack_start: 0, used_prev: {} }"#
                ]],
            );

            state.assign(0, SSAVar::Computed(0, 23));
            check_debug(
                &state,
                expect![[
                    r#"SSAStackState { stack: [Computed(0, 23)], accu: Prev(Acc), stack_start: 0, used_prev: {} }"#
                ]],
            );

            state.assign(1, SSAVar::Computed(0, 24));
            check_debug(
                &state,
                expect![[
                    r#"SSAStackState { stack: [Computed(0, 24), Computed(0, 23)], accu: Prev(Acc), stack_start: 1, used_prev: {} }"#
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
                        Prev(
                            Stack(
                                3,
                            ),
                        ),
                        Prev(
                            Stack(
                                2,
                            ),
                        ),
                        Prev(
                            Stack(
                                1,
                            ),
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
                    accu: Prev(
                        Acc,
                    ),
                    stack_start: 5,
                    used_prev: {},
                }"#]],
            );
        }
    }
}
