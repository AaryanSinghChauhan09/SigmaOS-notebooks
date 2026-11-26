//! sigmaos-notebooks CLI — summarize a .ipynb file.

use std::env;
use std::fs;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = env::args()
        .nth(1)
        .ok_or("usage: sigmaos-notebooks <file.ipynb>")?;
    let raw = fs::read_to_string(path)?;
    let nb = sigmaos_notebooks::parse_notebook(&raw)?;
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
    Ok(())
}
