use std::{
    ffi::OsString,
    fs::{create_dir_all, File},
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
};

use anyhow::{bail, Context, Result};
use indicatif::{ProgressBar, ProgressStyle};

use ocaml_jit_shared::basic_blocks::{BasicBlockExit, BasicBlockInstruction, BasicClosure};
use std::collections::HashMap;

#[derive(Debug)]
pub struct Options {
    pub use_links: bool,
    pub verbose: bool,
    pub output_path: PathBuf,
    pub output_closure_json: bool,
}

#[derive(Debug)]
struct VisContext {
    options: Options,
}

pub fn write_dot_graphs(closures: &HashMap<usize, BasicClosure>, options: Options) -> Result<()> {
    let ctx = VisContext { options };

    create_dir_all(&ctx.options.output_path).context("Could not create output directory")?;

    let progress = if ctx.options.verbose {
        None
    } else {
        let bar = ProgressBar::new(closures.len() as u64);

        bar.set_style(ProgressStyle::default_bar().template(
            "[{elapsed_precise}/{eta_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}",
        ));

        Some(bar)
    };

    for (closure_id, closure) in closures.iter() {
        let closure_id = *closure_id;
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

        ctx.output_closure_dot(closure_id, closure, &mut dot_file)
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

impl VisContext {
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
        closure: &BasicClosure,
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
                r#"<TR><TD BORDER="1"><B>Block {}</B></TD></TR>"#,
                block_no
            )?;

            let mut bytecode_instrs = vec![];
            for instr in &block.instructions {
                bytecode_instrs.append(&mut self.format_instruction(closure, instr));
            }
            bytecode_instrs.push(self.format_simple_instruction(&format!("{:?}", block.exit)));

            for s in bytecode_instrs.into_iter() {
                writeln!(f, "<TR>{}</TR>", s)?;
            }

            writeln!(f, "</TABLE>>];")?;

            match &block.exit {
                BasicBlockExit::Branch(other) => {
                    writeln!(f, "n{} -> n{};", block_no, other)?;
                }
                BasicBlockExit::BranchIf {
                    then_block,
                    else_block,
                } => {
                    writeln!(
                        f,
                        r#"n{} -> n{} [headlabel=" then"];"#,
                        block_no, then_block
                    )?;
                    writeln!(
                        f,
                        r#"n{} -> n{} [headlabel=" else"];"#,
                        block_no, else_block
                    )?;
                }
                BasicBlockExit::BranchCmp {
                    then_block,
                    else_block,
                    ..
                } => {
                    writeln!(
                        f,
                        r#"n{} -> n{} [headlabel=" then"];"#,
                        block_no, then_block
                    )?;
                    writeln!(
                        f,
                        r#"n{} -> n{} [headlable=" else"];"#,
                        block_no, else_block
                    )?;
                }
                BasicBlockExit::Switch { ints, tags } => {
                    for (i, other) in ints.iter().enumerate() {
                        writeln!(
                            f,
                            r#"n{} -> n{} [headlabel=" int {}"];"#,
                            block_no, other, i
                        )?;
                    }
                    for (tag, other) in tags.iter().enumerate() {
                        writeln!(
                            f,
                            r#"n{} -> n{} [headlabel=" tag {}"];"#,
                            block_no, other, tag
                        )?;
                    }
                }
                BasicBlockExit::TailCall { .. } => {
                    emit_tailcall = true;
                    writeln!(f, "n{} -> tailcall;", block_no)?;
                }
                BasicBlockExit::Return { .. } => {
                    emit_return = true;
                    writeln!(f, "n{} -> return;", block_no)?;
                }
                BasicBlockExit::Stop => {
                    emit_stop = true;
                    writeln!(f, "n{} -> stop;", block_no)?;
                }

                BasicBlockExit::Raise(_) => {
                    writeln!(f, "n{} -> raise{};", block_no, block_no)?;
                    writeln!(f, r#"raise{} [shape=box label="Raise"];"#, block_no)?;
                }

                BasicBlockExit::PushTrap { normal, trap } => {
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

    fn format_instruction(
        &self,
        _closure: &BasicClosure,
        instruction: &BasicBlockInstruction,
    ) -> Vec<String> {
        let mut extra = Vec::new();
        let first = match instruction {
            BasicBlockInstruction::Closure(to, _) => self.format_linked_instruction(
                format!("{:?}", instruction).as_str(),
                format!("./{}", self.closure_filename(*to, Extension::SVG)).as_str(),
            ),
            BasicBlockInstruction::ClosureRec(funcs, nvars) => {
                let first = self.format_simple_instruction("ClosureRec([");

                for func in funcs {
                    extra.push(format!(
                        "{}\n",
                        self.format_linked_instruction(
                            &format!("    {},", func),
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
}
