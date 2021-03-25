use colored::Colorize;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::fmt::{self, Write};

use crate::BytecodeLocation;

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq)]
pub enum CallTraceLocation {
    CCall(usize),
    Apply(BytecodeLocation),
}

impl fmt::Display for CallTraceLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CallTraceLocation::CCall(n) => {
                write!(f, "c:{}", n)
            }
            CallTraceLocation::Apply(n) => {
                write!(f, "{}", n)
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq)]
pub enum CallTraceAction {
    Enter { needed: usize, provided: Vec<u64> },
    Return(u64),
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct CallTrace {
    pub location: CallTraceLocation,
    pub action: CallTraceAction,
}

impl fmt::Display for CallTrace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let action = match self.action {
            CallTraceAction::Enter { .. } => format!("enter {}", self.location),
            CallTraceAction::Return(_) => format!("return {}", self.location),
        };

        let mut retargs = [0];

        let (needed, args) = match &self.action {
            CallTraceAction::Enter { needed, provided } => (*needed, &provided[..]),
            CallTraceAction::Return(v) => {
                retargs[0] = *v;
                (1, &retargs[..])
            }
        };

        let mut arg_s = String::new();
        for (i, arg) in args.iter().enumerate() {
            if i != 0 {
                write!(arg_s, ", ").unwrap();
            }

            if i == needed {
                write!(arg_s, "[").unwrap();
            } else {
                write!(arg_s, " ").unwrap();
            }

            write!(arg_s, "{:#018X}", arg).unwrap();
        }

        if needed < args.len() {
            write!(arg_s, "]").unwrap();
        }

        write!(f, "{:<20} {}", action, arg_s)
    }
}

pub fn compare_call_traces(expected: &CallTrace, actual: &CallTrace) {
    let expected_f = format!("{}", expected);
    println!("{}", expected_f.yellow().bold());
    let actual_f = format!("{}", actual);

    for x in expected_f.chars().zip_longest(actual_f.chars()) {
        let (error, c) = match x {
            itertools::EitherOrBoth::Both(c1, c2) => {
                if c1 == c2 {
                    (false, c2)
                } else {
                    (true, c2)
                }
            }
            itertools::EitherOrBoth::Left(_) => (true, '_'),
            itertools::EitherOrBoth::Right(c) => (true, c),
        };

        if error {
            let s = c.to_string();
            print!("{}", s.red().bold());
        } else {
            print!("{}", c);
        }
    }
}
