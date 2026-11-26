//! sigmaos-notebooks CLI — inspect a .ipynb file.
//!
//! usage: sigmaos-notebooks <file.ipynb> [--code|--markdown|--stats]

use std::env;
use std::fs;
use std::process::ExitCode;

use sigmaos_notebooks::CellType;

fn usage() -> ExitCode {
    eprintln!("usage: sigmaos-notebooks <file.ipynb> [--code|--markdown|--stats]");
    ExitCode::from(2)
}

fn main() -> Result<ExitCode, Box<dyn std::error::Error>> {
    let mut path = None;
    let mut mode = "stats";
    for arg in env::args().skip(1) {
        match arg.as_str() {
            "--code" => mode = "code",
            "--markdown" => mode = "markdown",
            "--stats" => mode = "stats",
            "-h" | "--help" => return Ok(usage()),
            _ if path.is_none() => path = Some(arg),
            _ => return Ok(usage()),
        }
    }
    let Some(path) = path else {
        return Ok(usage());
    };
    let raw = fs::read_to_string(path)?;
    let nb = sigmaos_notebooks::parse_notebook(&raw)?;
    match mode {
        "code" => print!("{}", nb.source_of(CellType::Code)),
        "markdown" => print!("{}", nb.source_of(CellType::Markdown)),
        _ => {
            let (code, md, rawc) = nb.counts();
            println!(
                "nbformat {}.{} — {} cells ({} code, {} markdown, {} raw), {} lines",
                nb.nbformat,
                nb.nbformat_minor,
                nb.cells.len(),
                code,
                md,
                rawc,
                nb.line_count()
            );
        }
    }
    Ok(ExitCode::SUCCESS)
}
