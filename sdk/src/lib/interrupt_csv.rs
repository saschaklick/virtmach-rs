//! Loads interrupt function definitions from <interrupt>.csv files.
//!
//! A csv next to the compiled source file has priority, after that the include directories are
//! searched in order, starting with ./include/ and ending with the repository's include/.

use std::{ collections::HashMap, fs::File, path::{ Path, PathBuf } };
use csv::ReaderBuilder;
use virtmach::VMAtom;

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

/// Loads the interrupts in the given order, the order defines the interrupt numbers.
pub fn load_interrupts(dirs: &[PathBuf], names: &[String]) -> Result<(Vec<String>, Functions), String> {
    let mut functions = Functions::new();
    let mut interrupts = vec![];

    for (int_no, int_name) in names.iter().enumerate() {
        let filename = find(dirs, int_name)?;
        log::info!("interrupt {} from {}", int_name, filename.display());
        let file = File::open(&filename).map_err(|e| format!("could not load {}: {}", filename.display(), e))?;
        let mut reader = ReaderBuilder::new().double_quote(false).has_headers(true).from_reader(file);

        interrupts.push(int_name.clone());
        functions.insert(int_name.clone(), (int_no as u8, 0, 0, 0));

        for record in reader.records() {
            let record = record.map_err(|e| format!("malformed {}: {}", filename.display(), e))?;
            let field = |i: usize| record.get(i).unwrap_or("").trim();
            let parsed = (field(1).parse::<VMAtom>(), field(2).parse::<usize>(), field(3).parse::<usize>());
            match parsed {
                (Ok(fcn_no), Ok(args), Ok(rets)) => { functions.insert(format!("{}.{}", int_name, field(0)), (int_no as u8, fcn_no, args, rets)); }
                _ => return Err(format!("malformed entry for {} in {}", field(0), filename.display()))
            }
        }
    }

    Ok((interrupts, functions))
}
