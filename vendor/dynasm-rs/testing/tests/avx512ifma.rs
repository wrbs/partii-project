#![allow(unused_imports)]

use dynasmrt::dynasm;
use dynasmrt::DynasmApi;

include!("gen_x64/avx512ifma.rs.gen");
