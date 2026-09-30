use std::{ env, ffi::OsStr, fs::File, io::{ Read, Write }, path::Path };
use virtmach::{ VirtMach, VMAtom, Program, interrupts::SoftInterruptFunction };

#[allow(dead_code)]
pub fn disassemble(program: Program) {
    println!();
    println!("Program \"{}\" ({}b):", program.id, program.data.len());
    println!();
    let mut pos = 0usize;
    let instructions = program.get_instructions();
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

#[allow(dead_code)]
pub fn load_file(filename: &str) -> Result<(String, String), std::io::Error> {
    match File::open(filename) {
        Ok(mut file) => {
            let name = String::from(Path::new(filename).file_stem().unwrap_or(OsStr::new("n/a")).to_str().unwrap_or("n/a"));
            let mut content = String::new();
            match file.read_to_string(&mut content) {
                Ok(_) => Ok((name, content)),                
                Err(err) => Err(err)
            }
        }
        Err(err) => Err(err)
    }
}

/// Where a BASIC variable is kept in the vm
#[allow(dead_code)]
pub enum Slot { Reg(usize), Mem(usize) }

/// Source for VirtMach::compile, a listing or (with the basic feature) a BASIC program starting with REM,
/// plus the BASIC variables for displaying their values
#[allow(dead_code)]
pub struct Source {
    pub filename: String,
    pub name: String,
    pub code: String,
    pub variables: Vec<(String, Slot)>
}

/// Loads the file given as first command line argument, or `default`. Errors are printed and return None.
#[allow(dead_code)]
pub fn load_source(default: &str, tables: &[(&str, &[SoftInterruptFunction])]) -> Option<Source> {
    let filename = env::args().nth(1).unwrap_or(String::from(default));
    match load_file(&filename) {
        Ok((name, code)) => {
            let variables = basic_variables(&name, &code, tables)?;
            Some(Source { filename, name, code, variables })
        }
        Err(err) => { println!("file read error: {}: {:?}", filename, err); None }
    }
}

/// Variables of a BASIC program, empty for listings. BASIC errors are printed and return None.
#[cfg(feature = "basic")]
fn basic_variables(name: &str, code: &str, tables: &[(&str, &[SoftInterruptFunction])]) -> Option<Vec<(String, Slot)>> {
    use virtmach::basic::{ self, Loc };

    if !basic::is_basic(code) { return Some(vec![]); }
    let (interrupts, functions) = basic::functions(tables);
    match basic::compile(name, code, &interrupts, &functions) {
        Ok(output) => {
            let mut variables: Vec<(String, Slot)> = output.variables.iter()
                .filter(|(name, _)| !name.starts_with('_'))
                .map(|(name, loc)| (name.clone(), match loc { Loc::Reg(r) => Slot::Reg(*r as usize), Loc::Mem(a) => Slot::Mem(*a) }))
                .collect();
            for (array, base, size) in &output.arrays {
                variables.extend((0..*size).map(|i| (format!("{}({})", array, i), Slot::Mem(base + i))));
            }
            Some(variables)
        }
        Err(err) => { println!("BASIC error in line {}: {}", err.line, err.msg); None }
    }
}

#[cfg(not(feature = "basic"))]
fn basic_variables(_name: &str, code: &str, _tables: &[(&str, &[SoftInterruptFunction])]) -> Option<Vec<(String, Slot)>> {
    let first = code.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    if first.trim_start_matches(|c: char| c.is_ascii_digit()).trim_start().to_ascii_uppercase().starts_with("REM") {
        println!("this looks like a BASIC program, compiling it needs the basic feature: cargo run --example <example> --features basic -- <file>");
        return None;
    }
    Some(vec![])
}

/// Where each BASIC variable is kept, e.g. "SUM        r0"
#[allow(dead_code)]
pub fn variable_locations(variables: &[(String, Slot)]) -> Vec<String> {
    variables.iter().map(|(name, slot)| match slot {
        Slot::Reg(r) => format!("{:>10} r{}", name, r),
        Slot::Mem(a) => format!("{:>10} memory #{}", name, a)
    }).collect()
}

/// Current value of each BASIC variable, e.g. "SUM = 55"
#[allow(dead_code)]
pub fn variable_values(vm: &VirtMach, variables: &[(String, Slot)]) -> Vec<String> {
    variables.iter().map(|(name, slot)| {
        let value = match slot { Slot::Reg(r) => vm.registers[*r], Slot::Mem(a) => vm.memory[*a] };
        format!("{:>10} = {}", name, value)
    }).collect()
}

/// Prints the BASIC variable values with ANSI cursor positioning in a column to the right of the
/// dashboard, which was printed at the given 1-based row and column. Leaves the cursor at the start
/// of the screen line below the dashboard.
#[allow(dead_code)]
pub fn print_variables_beside(vm: &VirtMach, variables: &[(String, Slot)], dashboard: &str, row: usize, column: usize) {
    let column = column + dashboard.lines().map(|l| l.chars().count()).max().unwrap_or(0) + 1;
    let values: Vec<String> = variable_values(vm, variables).iter().map(|l| format!("|{}", l)).collect();
    if !values.is_empty() {
        let width = values.iter().map(|l| l.chars().count()).max().unwrap_or(0).max(16);
        let header = [String::from("BASIC VARIABLES"), "-".repeat(width)];
        for (i, line) in header.iter().chain(values.iter()).enumerate() { print!("\x1b[{};{}H{}\x1b[K", row + i, column, line); }
    }
    print!("\x1b[{};1H", row + dashboard.lines().count());
    let _ = std::io::stdout().flush();
}

#[warn(dead_code)]
pub fn main() {
}