use std::{ffi::OsStr, fs::File, io::Read, path::Path};
use virtmach::{VirtMach, VMAtom, Program};

#[allow(dead_code)]
pub fn disassemble(program: Program) {
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

#[allow(dead_code)]
pub fn load_file(filename: &str) -> Result<(String, String), std::io::Error> {
    match File::open(filename) {
        Ok(mut file) => {
            let name = String::from(Path::new(filename).file_stem().unwrap_or(OsStr::new("-na-")).to_str().unwrap_or("-na-"));
            let mut content = String::new();
            match file.read_to_string(&mut content) {
                Ok(_) => Ok((name, content)),                
                Err(err) => Err(err)
            }
        }
        Err(err) => Err(err)
    }
}