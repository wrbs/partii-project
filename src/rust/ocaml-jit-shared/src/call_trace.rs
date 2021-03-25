use serde::{Deserialize, Serialize};
use std::fmt::{self, Write};

use crate::BytecodeLocation;

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug, Serialize, Deserialize)]
pub enum CallTraceAction {
    Enter { needed: usize, provided: Vec<u64> },
    Return(u64),
}

#[derive(Debug, Serialize, Deserialize)]
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
