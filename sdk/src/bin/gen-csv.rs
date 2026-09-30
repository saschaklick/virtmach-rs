//! Writes the interrupt .csv files from the FUNCTIONS definitions in src/interrupts/.

use std::{ fs::File, io::Write, path::{ Path, PathBuf } };
use clap::Parser;
use virtmach::interrupts::{ self, SoftInterruptFunction };

#[derive(Parser, Debug)]
#[command(name = "virtmach-rs CSV Generator")]
#[command(about = "Generate the interrupt .csv files from the built-in interrupt definitions", long_about = None)]
struct Args {
    #[arg(help = "Output directory, defaults to the repository's include/")]
    output: Option<String>
}

fn write(dir: &Path, name: &str, functions: &[SoftInterruptFunction]) -> Result<(), String> {
    let path = dir.join(format!("{}.csv", name));
    let mut out = String::from("name,index,arguments,returns,description\n");
    for function in functions {
        out += &format!("{},{},{},{},\"{}\"\n", function.name, function.no, function.arguments, function.returns, function.help.replace('"', "\"\""));
    }
    File::create(&path).and_then(|mut file| file.write_all(out.as_bytes())).map_err(|e| format!("{}: {}", path.display(), e))?;
    println!("{}", path.display());
    Ok(())
}

fn main() -> Result<(), String> {
    let args = Args::parse();
    let dir = args.output.map(PathBuf::from).unwrap_or(PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../include")));

    write(&dir, interrupts::math::NAME, &interrupts::math::FUNCTIONS)?;
    write(&dir, interrupts::proc::NAME, &interrupts::proc::FUNCTIONS)?;
    write(&dir, interrupts::random::NAME, &interrupts::random::FUNCTIONS)?;
    write(&dir, interrupts::string::NAME, &interrupts::string::FUNCTIONS)?;
    write(&dir, interrupts::surface::NAME, &interrupts::surface::FUNCTIONS)?;
    write(&dir, interrupts::trig::NAME, &interrupts::trig::FUNCTIONS)?;
    Ok(())
}
