//! SigmaOS component binary: notebooks

use sigmaos_notebooks::Component;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    Component::new().operate()?;
    println!("sigmaos-notebooks: ok");
    Ok(())
}
