use colored::Colorize;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::fmt::{self, Write};

use crate::BytecodeLocation;

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq)]
pub enum CallTrace {
    Enter {
        closure: BytecodeLocation,
        needed: usize,
        provided: Vec<u64>,
    },
    CCall {
        id: usize,
        args: Vec<u64>,
    },
    Return(u64),
    Custom(String),
}

fn format_args(args: &[u64], needed: usize) -> String {
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

    arg_s
}

impl fmt::Display for CallTrace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (loc, args) = match self {
            CallTrace::Enter {
                closure,
                needed,
                provided,
            } => {
                let loc = format!("apply {}", closure);
                let args = format_args(provided, *needed);
                (loc, args)
            }
            CallTrace::CCall { id, args } => {
                let loc = format!("c_call {}", id);
                let args = format_args(args, args.len());
                (loc, args)
            }
            CallTrace::Return(val) => {
                let loc = "return".to_string();
                let args = format_args(&[*val], 1);
                (loc, args)
            }
            CallTrace::Custom(message) => ("custom".to_string(), message.clone()),
        };
        write!(f, "{:<20} {}", loc, args)
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
