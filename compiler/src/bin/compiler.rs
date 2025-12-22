use std::{ ffi::OsStr, io::{ Read, Write }, fs::File, path::Path };
use log;
use simple_logger;
use clap::Parser;
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
    
    #[arg(short, long, help = "Optional binary output file")]
    output: Option<String>,

    #[arg(short, long, default_value_t = 1, help = "Verbosity level")]
    verbose: u32    
}

fn main() -> Result<(), ()> {
    let args = Args::parse();

    match args.verbose {
        0 => {}
        _ => simple_logger::init_with_level(match args.verbose {
            1 => log::Level::Info,
            2 => log::Level::Debug,
            _ => log::Level::Trace
        }).unwrap()
    }
    
    let mut external_interrupts = Vec::<(String, String)>::new();
    for int_name in args.interrupts.unwrap_or(vec![]) {        
        let filename = format!("{}.csv", int_name);
        match File::open(&filename) {
            Ok(mut file) => {
                let mut content = String::new();
                external_interrupts.push((int_name, match file.read_to_string(&mut content) { Ok(_) => { content }, _ => { String::new() }  }));
            }
            Err(_) => { eprintln!(); eprintln!("[ERROR] could not load {}", &filename); eprintln!(); }
        }        
    }  

    let out_file = args.output.unwrap_or(format!("{}.bin", Path::new(&args.source).file_stem().unwrap_or(OsStr::new("out")).to_str().unwrap_or("out")));

    match File::open(&args.source) {
        Ok(mut file) => {
            let name = String::from(Path::new(&args.source).file_stem().unwrap_or(OsStr::new("n/a")).to_str().unwrap_or("n/a"));
            let mut content = String::new();
            match file.read_to_string(&mut content) {
                Ok(_) => {
                    match VirtMach::compile(&name, &content, external_interrupts) {
                        Ok(res) => {                    
                            let program = res.0;
                            if args.verbose > 0 {
                                disassemble(&program);   
                            }

                            let mut file = File::create(out_file);
                            if file.is_ok() {
                                file.unwrap().write(&program.data);
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
                                ListingError::IllegalDefineValue(l, e) => { line = Some(l); eprintln!(); eprintln!("[ERROR] illegal value in define: {}", e); },
                                ListingError::UnknownLabel(l, e) => { line = Some(l); eprintln!(); eprintln!("[ERROR] unknown label: {}", e); },
                                ListingError::UnknownInterrupt(l, e) => { line = Some(l); eprintln!(); eprintln!("[ERROR] unknown interrupt: {}", e); },
                                ListingError::UnknownFunction(l, e) => { line = Some(l); eprintln!(); eprintln!("[ERROR] unknown function: {}", e); },                                
                                ListingError::MalformedFunction(l, e) => { line = Some(l); eprintln!(); eprintln!("[ERROR] malformed function: {}", e); },    
                                ListingError::IllegalInterrupt(l, e) => { line = Some(l); eprintln!(); eprintln!("[ERROR] illegal interrupt: {}", e); },
                                ListingError::NoError => {},                                
                            } }
                            if line.is_some() {
                                let line = line.unwrap();
                                eprintln!(); eprintln!("\tline #{}: {:?}", line, content.lines().nth(line - 1).unwrap_or("")); eprintln!();
                            }
                            Err(())
                        }
                    }                                                                       
                }
                Err(err) => { if args.verbose > 0 { eprintln!(); eprintln!("[ERROR] could not read from file: {}", err); eprintln!(); } Err(()) }
            }
        }
        Err(err) => { if args.verbose > 0 { eprintln!(); eprintln!("[ERROR] could not open file: {}", err); eprintln!(); } Err(()) }
    }
}

pub fn disassemble(program: &Program) {
    println!();
    println!("Program \"{}\" ({}b):", program.id, program.data.len());
    println!();
    let mut pos = 0usize;
    loop {
        let mut op = String::new();
        let addr = pos;                                        
        pos = VirtMach::decompile(&program, pos, &mut op);
        let slice = &program.data[addr..pos];
        let hex_wid = (1 + size_of::<VMAtom>()) * 3 - 1;
        println!("\t{:04x} | {:hex_wid$} | {}", addr, slice.iter().map(|b| format!("{:02X}", b)).collect::<Vec<_>>().join(" "), op.as_str());        

        if pos >= program.data.len() { break }
    }       
    println!();
}