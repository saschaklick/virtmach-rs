use std::{ ffi::OsStr, io::{ Read, Write }, fs::File, path::Path, format };
use clap::Parser;
use simple_logger;
use virtmach::{ VirtMach, VMAtom, Program, ListingError };

#[derive(Parser, Debug)]
#[command(name = "virtmach-rs Compiler")]
#[command(version = "0.1")]
#[command(about = "Compile virtmach-rs listings into binary", long_about = None)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(help = "Listing file to compile")]
    source: String,

    #[arg(value_delimiter = ' ', num_args = 1.., help = "Additional interrupts to include. Reads from a .csv file by the same name")]
    interrupts: Option<Vec<String>>,

    #[arg(short = 'I', long = "include", help = "Additional directory to search for interrupt .csv files, after the source file's directory and ./include/, before the repository's include/ (repeatable)")]
    include: Vec<String>,

    #[arg(short, long, help = "Optional binary output file")]
    output: Option<String>,

    #[arg(short, long, default_value_t = 1, help = "Verbosity level")]
    verbose: u32    
}

fn main() -> Result<(), String> {
    let args = Args::parse();

    match args.verbose {
        0 => {}
        _ => simple_logger::init_with_level(match args.verbose {
            1 => log::Level::Error,
            2 => log::Level::Info,
            3 => log::Level::Debug,
            _ => log::Level::Trace
        }).unwrap()
    }
    
    let dirs = interrupt_csv::search_path(Path::new(&args.source), &args.include);
    let (interrupts, functions) = match interrupt_csv::load_interrupts(&dirs, &args.interrupts.unwrap_or(vec![])) {
        Ok(res) => res,
        Err(err) => { eprintln!(); eprintln!("[ERROR] {}", err); eprintln!(); return Err(format!("")); }
    };

    let out_file = args.output.unwrap_or(format!("{}.bin", Path::new(&args.source).file_stem().unwrap_or(OsStr::new("out")).to_str().unwrap_or("out")));

    match File::open(&args.source) {
        Ok(mut file) => {
            let name = String::from(Path::new(&args.source).file_stem().unwrap_or(OsStr::new("n/a")).to_str().unwrap_or("n/a"));
            let mut content = String::new();
            match file.read_to_string(&mut content) {
                Ok(_) => {                    
                    match VirtMach::compile_owned(&name, &content, &interrupts, functions) {
                        Ok(res) => {                    
                            let program = res.0;
                            if args.verbose > 0 {
                                disassemble(&program);   
                            }

                            let file = File::create(&out_file);
                            if file.is_ok() {
                                match file.unwrap().write(&program.data) {
                                    Ok(_) => { println!("[OK] wrote binary to {}", &out_file); }
                                    Err(err) => { eprintln!("failed to write binary file {}: {}", &out_file, err); return Err(format!("")); }
                                }
                            }

                            Ok(())                                                      
                        }
                        Err(err) => {
                            let mut line: Option<usize> = None;
                            if args.verbose > 0 { match err {                            
                                ListingError::IllegalOp(l, e) => { line = Some(l); eprintln!(); eprintln!("[ERROR] illegal op code: {}", e); },
                                ListingError::IllegalArgument(l, e) => { line = Some(l); eprintln!(); eprintln!("[ERROR] illegal argument: {}", e); },
                                ListingError::IllegalRegister(l, e) => { line = Some(l); eprintln!(); eprintln!("[ERROR] illegal register: {}", e); },
                                ListingError::MalformedDefine(l, e) => { line = Some(l); eprintln!(); eprintln!("[ERROR] malformed define: {}", e); },
                                ListingError::MalformedLiteral(l, e) => { line = Some(l); eprintln!(); eprintln!("[ERROR] malformed literal: {}", e); },
                                ListingError::IllegalDefineValue(l, e) => { line = Some(l); eprintln!(); eprintln!("[ERROR] illegal value in define: {}", e); },
                                ListingError::UnknownLabel(l, e) => { line = Some(l); eprintln!(); eprintln!("[ERROR] unknown label: {}", e); },
                                ListingError::UnknownInterrupt(l, e) => { line = Some(l); eprintln!(); eprintln!("[ERROR] unknown interrupt: {}", e); },
                                ListingError::UnknownFunction(l, e) => { line = Some(l); eprintln!(); eprintln!("[ERROR] unknown function: {}", e); },                                
                                ListingError::MalformedFunction(l, e) => { line = Some(l); eprintln!(); eprintln!("[ERROR] malformed function: {}", e); },    
                                ListingError::IllegalInterrupt(l, e) => { line = Some(l); eprintln!(); eprintln!("[ERROR] illegal interrupt: {}", e); },
                                #[cfg(feature = "basic")]
                                ListingError::Basic(l, e) => { line = if l > 0 { Some(l) } else { None }; eprintln!(); eprintln!("[ERROR] BASIC: {}", e); },
                                ListingError::NoError => {},                                
                            } }
                            if line.is_some() {
                                let line = line.unwrap();
                                eprintln!(); eprintln!("\tline #{}: {:?}", line, content.lines().nth(line - 1).unwrap_or("")); eprintln!();
                            }
                            Err(format!(""))
                        }
                    }                                                                       
                }
                Err(err) => { if args.verbose > 0 { eprintln!(); eprintln!("[ERROR] could not read from file: {}", err); eprintln!(); } Err(format!("")) }
            }
        }
        Err(err) => { if args.verbose > 0 { eprintln!(); eprintln!("[ERROR] could not open file: {}", err); eprintln!(); } Err(format!("")) }
    }
}

pub fn disassemble(program: &Program) {
    println!();
    println!("Program \"{}\" ({}b):", program.id, program.data.len());
    println!();
    let mut pos = 0usize;
    let instructions = program.get_instructions();
    for index in 0 .. program.get_fixed_bin_count() {
        let bin = program.get_bin(index);
        println!("\t{:4} = [{:3}] {} \"{}\"", index, bin.len(), bin.iter().map(|b| format!("{:02X}", b)).collect::<Vec<_>>().join(" "), str::from_utf8(&bin).unwrap_or("<binary>"));
    }
    if program.get_fixed_bin_count() > 0 {
        println!();
    }
    loop {
        let mut op = String::new();
        let addr = pos;                                        
        pos = VirtMach::decompile(&program, pos, &mut op);
        let slice = &instructions[addr..pos];
        let hex_wid = (1 + size_of::<VMAtom>()) * 3 - 1;
        println!("\t{:04x} | {:hex_wid$} | {}", addr, slice.iter().map(|b| format!("{:02X}", b)).collect::<Vec<_>>().join(" "), op.as_str());        

        if pos >= instructions.len() { break }
    }       
    println!();
}

#[path = "../lib/interrupt_csv.rs"]
mod interrupt_csv;
