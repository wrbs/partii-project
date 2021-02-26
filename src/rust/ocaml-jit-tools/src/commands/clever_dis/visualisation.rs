use std::ffi::OsString;
use std::fs::{create_dir_all, File};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use itertools::{EitherOrBoth, Itertools};

use ocaml_jit_shared::Instruction;

use crate::commands::clever_dis::ssa::translate_block;

use super::data::*;
use super::DotShow;
use crate::commands::clever_dis::ssa::data::SSAClosure;

#[derive(Debug)]
pub struct Options {
    pub use_links: bool,
    pub verbose: bool,
    pub output_path: PathBuf,
    pub show: DotShow,
    pub output_closure_json: bool,
}

#[derive(Debug)]
struct VisContext<'a> {
    program: &'a Program,
    options: Options,
}

pub fn write_dot_graphs(
    program: &Program,
    ssa_closures: &[SSAClosure],
    options: Options,
) -> Result<()> {
    let ctx = VisContext { program, options };

    create_dir_all(&ctx.options.output_path).context("Could not create output directory")?;

    let progress = if ctx.options.verbose {
        None
    } else {
        let bar = ProgressBar::new(ctx.program.closures.len() as u64);

        bar.set_style(ProgressStyle::default_bar().template(
            "[{elapsed_precise}/{eta_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}",
        ));

        Some(bar)
    };

    for (closure_id, closure) in ctx.program.closures.iter().enumerate() {
        let ssa_closure = &ssa_closures[closure_id];
        let dot_path = ctx
            .options
            .output_path
            .join(ctx.closure_filename(closure_id, Extension::Dot));

        let svg_path = ctx
            .options
            .output_path
            .join(ctx.closure_filename(closure_id, Extension::SVG));

        if ctx.options.verbose {
            eprintln!("Writing dot file: {}", dot_path.to_string_lossy());
        } else if let Some(p) = &progress {
            p.set_message(format!("Creating {}", svg_path.to_string_lossy()).as_str());
        }

        let mut dot_file = File::create(&dot_path).with_context(|| {
            format!(
                "Could not create output dot file {}",
                dot_path.to_string_lossy()
            )
        })?;

        ctx.output_closure_dot(closure_id, closure, ssa_closure, &mut dot_file)
            .with_context(|| format!("Problem writing closure file for closure {}", closure_id))?;

        if ctx.options.output_closure_json {
            let json_path = ctx
                .options
                .output_path
                .join(ctx.closure_filename(closure_id, Extension::JSON));
            let f = File::create(json_path).context("Cannot create output json file")?;
            serde_json::to_writer(f, &closure).context("Problem serializing output JSON")?;
        }

        let args = &[
            &OsString::from("-Tsvg"),
            &OsString::from("-Nfontname=monospace"),
            &OsString::from("-Efontname=monospace"),
            &OsString::from("-o"),
            svg_path.as_os_str(),
            dot_path.as_os_str(),
        ];

        if ctx.options.verbose {
            eprintln!("Running command dot with args {:?}", args);
        }

        let cmd_output = Command::new("dot")
            .stdin(Stdio::null())
            .args(args)
            .output()
            .context("Problem running dot to create output file")?;

        if !cmd_output.status.success() {
            std::io::stderr().write_all(&cmd_output.stderr)?;
            bail!("Problem running dot to create output file");
        }

        if let Some(p) = &progress {
            p.inc(1);
        }
    }

    Ok(())
}

fn html_escape(s: &str) -> String {
    s.replace("&", "&amp;")
        .replace("<", "&lt;")
        .replace(">", "&gt;")
}

enum Extension {
    SVG,
    Dot,
    JSON,
}

impl Extension {
    fn as_str(&self) -> &'static str {
        match self {
            Extension::SVG => "svg",
            Extension::Dot => "dot",
            Extension::JSON => "json",
        }
    }
}

impl<'a> VisContext<'a> {
    pub fn closure_filename(&self, closure_id: usize, extension: Extension) -> String {
        if closure_id == 0 {
            format!("root.{}", extension.as_str())
        } else {
            format!("closure_{}.{}", closure_id, extension.as_str())
        }
    }

    pub fn output_closure_dot<W: Write>(
        &self,
        closure_no: usize,
        closure: &Closure,
        ssa_closure: &SSAClosure,
        f: &mut W,
    ) -> Result<()> {
        writeln!(f, "digraph G {{")?;

        // Write basic metadata
        writeln!(
            f,
            r#"info [shape=plain label=<<TABLE BORDER="1" CELLBORDER="0" ALIGN="left">"#
        )?;
        writeln!(
            f,
            r#"<TR><TD BORDER="1"><B>Closure {}</B></TD></TR>"#,
            closure_no
        )?;

        if let Some(PositionInfo {
            module,
            def_name,
            filename,
            heap_env,
            rec_env,
        }) = &closure.position
        {
            writeln!(
                f,
                "<TR>{}</TR>\n",
                self.format_simple_instruction(&format!("Module: {}", module))
            )?;
            writeln!(
                f,
                "<TR>{}</TR>\n",
                self.format_simple_instruction(&format!("Def name: {}", def_name))
            )?;
            writeln!(
                f,
                "<TR>{}</TR>\n",
                self.format_simple_instruction(&format!("File: {}", filename))
            )?;

            for (id, ident) in heap_env {
                writeln!(
                    f,
                    "<TR>{}</TR>\n",
                    self.format_simple_instruction(&format!("Heap {}: {:?}", id, ident))
                )?;
            }

            for (id, ident) in rec_env {
                writeln!(
                    f,
                    "<TR>{}</TR>\n",
                    self.format_simple_instruction(&format!("Rec {}: {:?}", id, ident))
                )?;
            }
        }

        writeln!(f, "</TABLE>>];")?;

        writeln!(f, "info -> n0;")?;

        let mut emit_return = false;
        let mut emit_stop = false;
        let mut emit_tailcall = false;

        for (block_no, block) in closure.blocks.iter().enumerate() {
            writeln!(
                f,
                r#"n{} [shape=plain label=<<TABLE BORDER="1" CELLBORDER="0" ALIGN="left" COLUMNS="*">"#,
                block_no
            )?;
            writeln!(
                f,
                r#"<TR><TD BORDER="1"{}><B>Block {}</B></TD></TR>"#,
                if self.options.show == DotShow::Both {
                    r#" COLSPAN="2""#
                } else {
                    ""
                },
                block_no
            )?;

            let bytecode_instrs = if self.options.show.show_bytecode() {
                let mut bytecode_instrs = vec![];
                for instr in &block.instructions {
                    bytecode_instrs.append(&mut self.format_instruction(closure, instr));
                }
                Some(bytecode_instrs)
            } else {
                None
            };

            let ssa_instrs = if self.options.show.show_ssa() {
                let mut ssa_instrs: Vec<_> = format!("{}", ssa_closure.blocks[block_no])
                    .lines()
                    .map(|l| format!(r#"<TD ALIGN="left">{}   </TD>"#, html_escape(l)))
                    .collect();

                ssa_instrs.extend(
                    format!("{}", ssa_closure.blocks[block_no].final_state)
                        .lines()
                        .map(|l| {
                            let s = html_escape(l);
                            let sections: Vec<_> = s.split(':').collect();
                            format!(
                                r#"<TD ALIGN="left"><B>{:>13}:</B>{}   </TD>"#,
                                sections[0],
                                &sections[1..sections.len()].join(":")
                            )
                        }),
                );

                Some(ssa_instrs)
            } else {
                None
            };

            match self.options.show {
                DotShow::Both => {
                    for i in bytecode_instrs
                        .unwrap()
                        .into_iter()
                        .zip_longest(ssa_instrs.unwrap())
                    {
                        match i {
                            EitherOrBoth::Both(a, b) => {
                                writeln!(f, "<TR>{}{}</TR>", a, b)?;
                            }
                            EitherOrBoth::Left(a) => {
                                writeln!(f, r#"<TR>{}<TD style="invis"></TD></TR>"#, a)?;
                            }
                            EitherOrBoth::Right(b) => {
                                writeln!(f, r#"<TR><TD style="invis"></TD>{}</TR>"#, b)?;
                            }
                        }
                    }
                }
                DotShow::Bytecode => {
                    for s in bytecode_instrs.unwrap().into_iter() {
                        writeln!(f, "<TR>{}</TR>", s)?;
                    }
                }
                DotShow::SSA => {
                    for s in ssa_instrs.unwrap().into_iter() {
                        writeln!(f, "<TR>{}</TR>", s)?;
                    }
                }
            }

            writeln!(f, "</TABLE>>];")?;

            match &block.exit {
                BlockExit::UnconditionalJump(other) => {
                    writeln!(f, "n{} -> n{};", block_no, other)?;
                }
                BlockExit::ConditionalJump(a, b) => {
                    writeln!(f, "n{} -> n{};", block_no, a)?;
                    writeln!(f, "n{} -> n{};", block_no, b)?;
                }
                BlockExit::Switch { ints, blocks } => {
                    for (i, other) in ints.iter().enumerate() {
                        writeln!(
                            f,
                            r#"n{} -> n{} [headlabel=" int {}"];"#,
                            block_no, other, i
                        )?;
                    }
                    for (tag, other) in blocks.iter().enumerate() {
                        writeln!(
                            f,
                            r#"n{} -> n{} [headlabel=" tag {}"];"#,
                            block_no, other, tag
                        )?;
                    }
                }
                BlockExit::TailCall => {
                    emit_tailcall = true;
                    writeln!(f, "n{} -> tailcall;", block_no)?;
                }
                BlockExit::Return => {
                    emit_return = true;
                    writeln!(f, "n{} -> return;", block_no)?;
                }
                BlockExit::Stop => {
                    emit_stop = true;
                    writeln!(f, "n{} -> stop;", block_no)?;
                }

                BlockExit::Raise => {
                    writeln!(f, "n{} -> raise{};", block_no, block_no)?;
                    writeln!(f, r#"raise{} [shape=box label="Raise"];"#, block_no)?;
                }

                BlockExit::PushTrap { normal, trap } => {
                    writeln!(f, "n{} -> n{};", block_no, normal)?;
                    writeln!(f, "n{} -> n{} [color=green]", block_no, trap)?;
                }
            }
        }

        if emit_tailcall {
            writeln!(f, r#"tailcall [shape=box label="Tail Call"];"#)?;
        }

        if emit_stop {
            writeln!(f, r#"stop [shape=box label="Stop"];"#)?;
        }

        if emit_return {
            writeln!(f, r#"return [shape=box label="Return"];"#)?;
        }

        writeln!(f, "}}")?;
        Ok(())
    }

    fn format_closure_name(&self, closure_id: usize) -> String {
        match &self.program.closures[closure_id].position {
            Some(PositionInfo { def_name, .. }) => {
                format!(" # {}", def_name)
            }
            None => {
                format!("")
            }
        }
    }

    fn format_instruction(
        &self,
        closure: &Closure,
        instruction: &Instruction<usize>,
    ) -> Vec<String> {
        let mut extra = Vec::new();
        let first = match instruction {
            Instruction::Closure(to, _) => self.format_linked_instruction(
                format!("{:?}{}", instruction, self.format_closure_name(*to)).as_str(),
                format!("./{}", self.closure_filename(*to, Extension::SVG)).as_str(),
            ),
            Instruction::ClosureRec(funcs, nvars) => {
                let first = self.format_simple_instruction("ClosureRec([");

                for func in funcs {
                    extra.push(format!(
                        "{}\n",
                        self.format_linked_instruction(
                            &format!("    {},{}", func, self.format_closure_name(*func)),
                            &format!("./{}", self.closure_filename(*func, Extension::SVG)),
                        )
                    ));
                }
                extra.push(format!(
                    "{}\n",
                    self.format_simple_instruction(&format!("], {})", nvars))
                ));

                first
            }
            Instruction::EnvAcc(id) => {
                self.format_simple_instruction(&match closure.lookup_heap_ident(*id as usize) {
                    Some(ident) => format!("EnvAcc({}) # {}", id, ident),
                    None => format!("EnvAcc({})", id),
                })
            }
            Instruction::OffsetClosure(offset) => self.format_simple_instruction(&match closure
                .lookup_closure_ident(*offset as i64)
            {
                Some(ident) => format!("OffsetClosure({}) # {}", offset, ident),
                None => format!("OffsetClosure({})", offset),
            }),

            Instruction::GetGlobal(id) => self.format_simple_instruction(
                format!("GetGlobal({}) # {}", id, self.format_global(id)).as_str(),
            ),
            Instruction::SetGlobal(id) => self.format_simple_instruction(
                format!("SetGlobal({}) # {}", id, self.format_global(id)).as_str(),
            ),
            Instruction::CCall1(id) => self.c_call(id, 1),
            Instruction::CCall2(id) => self.c_call(id, 2),
            Instruction::CCall3(id) => self.c_call(id, 3),
            Instruction::CCall4(id) => self.c_call(id, 4),
            Instruction::CCall5(id) => self.c_call(id, 5),
            Instruction::CCallN(id, nargs) => self.c_call(id, *nargs),
            _ => self.format_simple_instruction(format!("{:?}", instruction).as_str()),
        };

        let mut instructions = vec![first];
        instructions.append(&mut extra);

        instructions
    }

    fn format_simple_instruction(&self, contents: &str) -> String {
        format!(r#"<TD ALIGN="left">{}  </TD>"#, html_escape(contents))
    }

    fn format_linked_instruction(&self, contents: &str, href: &str) -> String {
        if self.options.use_links {
            format!(
                r#"<TD ALIGN="left" HREF="{}"><U>{}</U>   </TD>"#,
                href,
                html_escape(contents)
            )
        } else {
            format!(r#"<TD ALIGN="left">{}   </TD>"#, html_escape(contents))
        }
    }

    fn c_call(&self, id: &u32, nargs: u32) -> String {
        match self.program.primitives.get(*id as usize) {
            Some(p) => {
                self.format_simple_instruction(format!("CCall({} = {}, {})", id, p, nargs).as_str())
            }
            None => self.format_simple_instruction(format!("CCall({}, {})", id, nargs).as_str()),
        }
    }

    fn format_global(&self, id: &u32) -> String {
        match self.program.globals.get(*id as usize) {
            Some(GlobalTableEntry::Global(g)) => g.clone(),
            Some(GlobalTableEntry::Constant(g)) => {
                format!("{}", self.program.global_data_blocks.format_value(g))
            }
            None => "<empty>".into(),
        }
    }
}
