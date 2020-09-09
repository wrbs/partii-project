use crate::utils::{die, die_dyn};
use colored::Colorize;
use ocaml_bytecode::trailer::{parse_trailer, SectionEntry, TRAILER_LENGTH};
use pretty_hex::PrettyHex;
use prettytable::format::consts::FORMAT_NO_LINESEP_WITH_TITLE;
use prettytable::Table;
use std::error::Error;
use std::fs::File;
use std::io;
use std::io::{Read, Seek};
use std::path::PathBuf;
use std::process::exit;
use structopt::StructOpt;

#[derive(StructOpt)]
#[structopt(about = "show a hexdump with the different sections")]
pub struct Options {
    #[structopt(parse(from_os_str))]
    input: PathBuf,
}

pub fn run(options: Options) {
    let () = run_exn(options).unwrap_or_else(die_dyn);
}

struct ParsedSection {
    name: String,
    length: usize,
}

impl ParsedSection {
    fn new<S: Into<String>>(name: S, length: usize) -> ParsedSection {
        ParsedSection {
            name: name.into(),
            length,
        }
    }
}

fn run_exn(options: Options) -> Result<(), Box<dyn Error>> {
    let mut f = File::open(&options.input)?;
    let file_size = options.input.metadata()?.len() as usize;
    let trailer = parse_trailer(&mut f).unwrap_or_else(die);

    // Check that the size adds up
    let sum_of_lengths: usize = trailer.sections.iter().map(|entry| entry.length).sum();
    let toc_size = 8 * trailer.sections.len();

    let content_size = sum_of_lengths + toc_size + TRAILER_LENGTH;

    if content_size > file_size {
        eprintln!(
            "File too small: expected at least {} from the trailer but the file is only {}",
            content_size, file_size
        );

        exit(1);
    }

    let pre_content_size = file_size - content_size;

    let mut parsed_sections = Vec::new();
    parsed_sections.push(ParsedSection::new("start", pre_content_size));
    for section in trailer.sections.iter() {
        let parsed_name = String::from_utf8_lossy(&section.name);
        parsed_sections.push(ParsedSection::new(parsed_name, section.length));
    }
    parsed_sections.push(ParsedSection::new("toc", toc_size));
    parsed_sections.push(ParsedSection::new("trailer", TRAILER_LENGTH));

    // Hexdumps of everything
    f.seek(io::SeekFrom::Start(0))?;
    print_hexdumps(&mut f, &parsed_sections)?;

    // Overview table
    print_overview_table(&parsed_sections);

    Ok(())
}

fn print_overview_table(sections: &Vec<ParsedSection>) {
    let mut table = Table::new();
    table.set_format(*FORMAT_NO_LINESEP_WITH_TITLE);
    table.set_titles(row![b => "Section", "Offset", "Length"]);
    let mut current = 0;

    for ParsedSection { name, length } in sections {
        table.add_row(row![r -> name, format!("0x{:X}", current), format!("{}", length)]);
        current += length;
    }

    table.printstd();
}

fn print_hexdumps(f: &mut File, sections: &Vec<ParsedSection>) -> io::Result<()> {
    for (idx, section) in sections.iter().enumerate() {
        if idx != 0 {
            println!();
        }

        let mut contents = vec![0; section.length];
        f.read_exact(&mut contents)?;

        println!("{}:", section.name.red());
        println!("{:?}", contents.hex_dump());
    }

    Ok(())
}
