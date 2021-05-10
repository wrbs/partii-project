use std::{io::Write, process::Command};

use crate::basic_blocks::BasicClosure;

use super::{CompilerOutput, CraneliftCompiler, CraneliftCompilerOptions};
use cranelift_codegen::{
    isa::TargetIsa,
    settings::{self, Configurable},
};
use cranelift_module::default_libcall_names;
use cranelift_object::{ObjectBuilder, ObjectModule};
use expect_test::{expect_file, ExpectFile};
use std::fmt::Write as FmtWrite;
use tempfile::NamedTempFile;

#[allow(clippy::clippy::too_many_arguments)]
fn run_test(
    case_name: &str,
    closure_json: &str,
    expected_codegen: ExpectFile,
    expected_compile: ExpectFile,
    expected_stack_maps: ExpectFile,
    expected_disasm: ExpectFile,
    expected_objdump: ExpectFile,
    expected_closuredump: ExpectFile,
) {
    let _ = env_logger::try_init(); // Cranelift uses rust_log a lot internally and this allows setting RUST_LOG
    let closure: BasicClosure = serde_json::from_str(closure_json).unwrap();

    expected_closuredump.assert_eq(&dump_closure(&closure));

    let isa = get_isa();
    let object_builder = ObjectBuilder::new(isa, case_name, default_libcall_names()).unwrap();
    let module = ObjectModule::new(object_builder);
    let options = CraneliftCompilerOptions {
        use_call_traces: false,
    };
    let mut compiler = CraneliftCompiler::new(module, 0x12121212).unwrap();

    let mut compiler_output = CompilerOutput::default();
    let mut stack_maps = vec![];

    let lookup_closure_code = |_| Some(0xDEADBEEF as *const u8);

    let _ = compiler
        .compile_closure(
            case_name,
            &closure,
            lookup_closure_code,
            &options,
            Some(&mut compiler_output),
            &mut stack_maps,
        )
        .unwrap();

    let op = compiler.module.finish();
    let obj = op.object.write().unwrap();

    let mut tempfile = NamedTempFile::new().unwrap();

    tempfile.write_all(&obj).unwrap();

    let stdout = Command::new("objdump")
        .arg("-M")
        .arg("intel")
        .arg("-D")
        .arg("-r")
        .arg("-g")
        .arg(tempfile.path())
        .output()
        .expect("Failed to run objdump")
        .stdout;

    let objdump_output = String::from_utf8(stdout).unwrap();
    let objdump_output = objdump_output.replace(tempfile.path().to_str().unwrap(), "<input>");

    expected_codegen.assert_eq(&compiler_output.ir_after_codegen);
    expected_compile.assert_eq(&compiler_output.ir_after_compile);

    let actual_stack_maps = {
        let mut s = String::new();
        for (offset, map) in stack_maps {
            write!(s, "{:#x}: {:#?}", offset, map).unwrap();
        }
        s
    };

    expected_stack_maps.assert_eq(&&actual_stack_maps);

    expected_disasm.assert_eq(&compiler_output.disasm);
    expected_objdump.assert_eq(&objdump_output);
}

fn get_isa() -> Box<dyn TargetIsa> {
    let mut shared_builder = settings::builder();
    shared_builder.set("enable_safepoints", "true").unwrap();
    shared_builder.set("opt_level", "speed").unwrap();
    let flags = settings::Flags::new(shared_builder);

    cranelift_codegen::isa::lookup_by_name("x86_64-unknown-linux-gnu")
        .unwrap()
        .finish(flags)
}

macro_rules! test_case {
    ($case: ident) => {
        #[test]
        fn $case() {
            run_test(
                stringify!($case),
                include_str!(concat!("./test_cases/", stringify!($case), "/closure.json")),
                expect_file![concat!(
                    "./test_cases/",
                    stringify!($case),
                    "/ir-after-codegen"
                )],
                expect_file![concat!(
                    "./test_cases/",
                    stringify!($case),
                    "/ir-after-compile"
                )],
                expect_file![concat!("./test_cases/", stringify!($case), "/stack-maps")],
                expect_file![concat!("./test_cases/", stringify!($case), "/disasm")],
                expect_file![concat!("./test_cases/", stringify!($case), "/objdump")],
                expect_file![concat!("./test_cases/", stringify!($case), "/closure-dump")],
            );
        }
    };
}

test_case!(stdlib_min);
test_case!(output_char);
test_case!(format_convert_int);
test_case!(list_iter);
test_case!(min_fun);
test_case!(makeblock_internalformat_make_int_padding_precision_anon);
test_case!(big_makeblock);
test_case!(arith_add);
test_case!(arith_sub);
test_case!(arith_mod);
test_case!(arith_div);
test_case!(arith_le);
test_case!(arith_toplevel_t);
test_case!(trigger_gc_please);
test_case!(arith_not);
test_case!(arith_neg);
test_case!(calls);
test_case!(closure);
test_case!(printf_kprintf);
test_case!(simple_switch);
test_case!(switch_ints);
test_case!(switch_tags);
test_case!(rec_closures);
test_case!(buffer_add_char);

fn dump_closure(closure: &BasicClosure) -> String {
    let mut s = String::new();

    writeln!(s, "Arity: {}", closure.arity).unwrap();
    writeln!(s, "Max stack size: {}", closure.max_stack_size).unwrap();

    writeln!(s).unwrap();

    for (block_num, block) in closure.blocks.iter().enumerate() {
        writeln!(
            s,
            "# Block {} (stack_start = {})",
            block_num, block.start_stack_size
        )
        .unwrap();
        for instr in &block.instructions {
            writeln!(s, "{:?}", instr).unwrap();
        }

        writeln!(s, "Exit: {:?}", block.exit).unwrap();

        writeln!(s).unwrap();
    }

    s
}
test_case!(clamp);
test_case!(intern_segfault);
