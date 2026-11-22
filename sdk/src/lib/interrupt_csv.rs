//! Loads interrupt function definitions from <interrupt>.csv files.
//!
//! A csv next to the compiled source file has priority, after that the include directories are
//! searched in order, starting with ./include/ and ending with the repository's include/.

use std::{ collections::HashMap, fs::File, path::{ Path, PathBuf } };
use csv::ReaderBuilder;
use virtmach::{ VMAtom, interrupts };

/// Interrupt functions by "interrupt.function" name: (interrupt no, function no, arguments, returns).
/// The bare interrupt name maps to (interrupt no, 0, 0, 0) for `int <interrupt>` in listings.
pub type Functions = HashMap<String, (u8, VMAtom, usize, usize)>;

pub const DEFAULT_INCLUDE: &str = "./include";

/// The repository's include/ at build time, so the tools find its csv files from any directory
pub const REPO_INCLUDE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../include");

/// The directories to look for csv files in: the source file's directory, ./include/, the given
/// ones and the repository's include/.
pub fn search_path(source: &Path, includes: &[String]) -> Vec<PathBuf> {
    let source_dir = source.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let mut dirs = vec![source_dir.to_path_buf(), PathBuf::from(DEFAULT_INCLUDE)];
    dirs.extend(includes.iter().map(PathBuf::from));
    dirs.push(PathBuf::from(REPO_INCLUDE));
    dirs
}

/// Finds the first <name>.csv in the directories.
pub fn find(dirs: &[PathBuf], name: &str) -> Result<PathBuf, String> {
    let file = format!("{}.csv", name);
    dirs.iter().map(|dir| dir.join(&file)).find(|path| path.is_file()).ok_or_else(|| {
        format!("could not find {} in {}", file, dirs.iter().map(|d| d.display().to_string()).collect::<Vec<_>>().join(", "))
    })
}

/// A function of an interrupt csv: (name, function no, arguments, returns, description)
type Row = (String, VMAtom, usize, usize, String);

fn read_rows(filename: &Path) -> Result<Vec<Row>, String> {
    let file = File::open(filename).map_err(|e| format!("could not load {}: {}", filename.display(), e))?;
    let mut reader = ReaderBuilder::new().double_quote(false).has_headers(true).from_reader(file);
    let mut rows = vec![];
    for record in reader.records() {
        let record = record.map_err(|e| format!("malformed {}: {}", filename.display(), e))?;
        let field = |i: usize| record.get(i).unwrap_or("").trim();
        match (field(1).parse::<VMAtom>(), field(2).parse::<usize>(), field(3).parse::<usize>()) {
            (Ok(fcn_no), Ok(args), Ok(rets)) => rows.push((field(0).to_string(), fcn_no, args, rets, field(4).to_string())),
            _ => return Err(format!("malformed entry for {} in {}", field(0), filename.display()))
        }
    }
    Ok(rows)
}

/// Loads the interrupts, see virtmach::interrupts::interrupt_numbers for their numbers. The order only matters for
/// interrupts that are not built in.
pub fn load_interrupts(dirs: &[PathBuf], names: &[String]) -> Result<(Vec<String>, Functions), String> {
    let mut functions = Functions::new();
    let mut interrupts = vec![];
    let numbers = interrupts::interrupt_numbers(names)?;

    for (int_name, int_no) in names.iter().zip(numbers) {
        let filename = find(dirs, int_name)?;
        log::info!("interrupt {} from {}", int_name, filename.display());
        let rows = read_rows(&filename)?;

        interrupts.push(int_name.clone());
        functions.insert(int_name.clone(), (int_no as u8, 0, 0, 0));

        for (name, fcn_no, args, rets, _) in rows {
            functions.insert(format!("{}.{}", int_name, name), (int_no as u8, fcn_no, args, rets));
        }
    }

    Ok((interrupts, functions))
}

/// The built-in interrupts that have a csv in the repository's include/
pub const BUILTIN: [&str; 6] = [interrupts::math::NAME, interrupts::proc::NAME, interrupts::string::NAME,
    interrupts::time::NAME, interrupts::surface::NAME, interrupts::periph::NAME];

/// Writes the help of the interrupts as csv, under one header, see virtmach::interrupts::help. The filter is
/// INTERRUPT, INTERRUPT.FUNCTION or .FUNCTION, each a name or a number; empty is everything. No names
/// are all built-in interrupts. Returns the number of rows.
pub fn help(dirs: &[PathBuf], names: &[String], filter: &str, writer: &mut dyn std::fmt::Write) -> Result<usize, String> {
    let names: Vec<String> = if names.is_empty() { BUILTIN.iter().map(|n| n.to_string()).collect() } else { names.to_vec() };
    let numbers = interrupts::interrupt_numbers(&names)?;

    let mut loaded = vec![];
    for name in &names {
        loaded.push(read_rows(&find(dirs, name)?)?);
    }
    let functions: Vec<Vec<interrupts::SoftInterruptFunction>> = loaded.iter().map(|rows| rows.iter().map(|(name, no, arguments, returns, help)| {
        interrupts::SoftInterruptFunction { no: *no, name, arguments: *arguments, returns: *returns, help }
    }).collect()).collect();
    let tables: Vec<interrupts::InterruptTable> = names.iter().zip(&numbers).zip(&functions)
        .map(|((name, number), functions)| interrupts::InterruptTable { name, number: *number, functions }).collect();

    let (interrupt, function) = match filter.trim().split_once('.') {
        Some((i, f)) => (i, f),
        None => (filter.trim(), "")
    };
    fn some(s: &str) -> Option<&str> { Some(s.trim()).filter(|s| !s.is_empty()) }
    interrupts::help(&tables, some(interrupt), some(function), writer).map_err(|e| e.to_string())
}
