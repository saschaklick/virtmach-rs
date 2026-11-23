//! Command line front end of the BASIC compiler in virtmach::basic.

use std::{ ffi::OsStr, fs::File, io::{ Read, Write }, path::Path };
use clap::Parser as ClapParser;
use simple_logger;
use virtmach::{ VirtMach, VMAtom, Program, ListingError, basic::{ compile, Loc } };
use interrupt_csv::load_interrupts;

#[derive(clap::Parser, Debug)]
#[command(name = "virtmach-rs BASIC Compiler")]
#[command(version = "0.1")]
#[command(about = "Compile BASIC programs into virtmach-rs binaries", long_about = None)]
struct Args {
    #[arg(required_unless_present = "help_interrupt", help = "BASIC file to compile")]
    source: Option<String>,

    #[arg(value_delimiter = ' ', num_args = 1.., help = "Interrupts to include, in the order of the runtime. Reads from a .csv file by the same name")]
    interrupts: Option<Vec<String>>,

    #[arg(short = 'I', long = "include", help = "Additional directory to search for interrupt .csv files, after the source file's directory and ./include/, before the repository's include/ (repeatable)")]
    include: Vec<String>,

    #[arg(short, long, help = "Optional binary output file")]
    output: Option<String>,

    #[arg(short, long, help = "Optional file to write the generated virtmach listing to")]
    listing: Option<String>,

    #[arg(long = "help-interrupt", value_name = "INTERRUPT[.FUNCTION]", num_args = 0..=1, default_missing_value = "",
        help = "Print the help of interrupts and their functions as csv and exit, by name or number, e.g. math, math.sqr, 0.10 or .sqr. Without a value all of them. With this, the arguments are interrupts, no program; use --help-interrupt=math")]
    help_interrupt: Option<String>,

    #[arg(short, long, default_value_t = 1, help = "Verbosity level")]
    verbose: u32
}


// ---------------------------------------------------------------------------------------------
// Command line

/// Lists the instructions, addresses match the vm's program counter (the leading atom-size byte is skipped).
fn disassemble(program: &Program) {
    println!();
    println!("\tatom size id {:02X}", program.data[0]);
    println!();
    let code = Program { source: program.source, id: program.id, data: program.data };
    let hex_wid = (1 + size_of::<VMAtom>()) * 3 - 1;
    let mut pos = 0usize;
    let instructions = code.get_instructions();
    while pos < instructions.len() {
        let mut op = String::new();
        let addr = pos;
        pos = VirtMach::decompile(&code, pos, &mut op);
        let bytes = instructions[addr..pos].iter().map(|b| format!("{:02X}", b)).collect::<Vec<_>>().join(" ");
        let target = match (op.split_whitespace().next(), op.split('#').nth(1).map(|v| v.parse::<i64>())) {
            (Some("cal" | "jmp" | "jpz" | "jpc" | "jps"), Some(Ok(offset))) => format!("  -> {:04x}", pos as i64 + offset),
            _ => String::new()
        };
        println!("\t{:04x} | {:hex_wid$} | {}{}", addr, bytes, op, target);
    }
    println!();
}

fn listing_error_line(err: &ListingError) -> Option<usize> {
    match err {
        ListingError::IllegalOp(l, _) | ListingError::IllegalArgument(l, _) | ListingError::IllegalRegister(l, _)
        | ListingError::IllegalInterrupt(l, _) | ListingError::MalformedDefine(l, _) | ListingError::IllegalDefineValue(l, _)
        | ListingError::UnknownLabel(l, _) | ListingError::UnknownInterrupt(l, _) | ListingError::UnknownFunction(l, _)
        | ListingError::MalformedFunction(l, _) | ListingError::MalformedLiteral(l, _) => Some(*l),
        ListingError::Basic(l, _) => Some(*l).filter(|l| *l > 0),
        ListingError::NoError => None
    }
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

    if let Some(filter) = &args.help_interrupt {
        // the arguments are all interrupts, there is no program
        let names: Vec<String> = args.source.iter().cloned().chain(args.interrupts.iter().flatten().cloned()).collect();
        let dirs = interrupt_csv::search_path(Path::new(""), &args.include);
        let mut out = String::new();
        return match interrupt_csv::help(&dirs, &names, filter, &mut out) {
            Ok(0) => { eprintln!("[ERROR] no interrupt or function matches {:?}", filter); Err(String::new()) }
            Ok(_) => { print!("{}", out); Ok(()) }
            Err(err) => { eprintln!(); eprintln!("[ERROR] {}", err); eprintln!(); Err(String::new()) }
        };
    }
    let source_arg = args.source.clone().unwrap();

    let source = Path::new(&source_arg);
    let name = source.file_stem().unwrap_or(OsStr::new("n/a")).to_string_lossy().to_string();
    let out_file = args.output.unwrap_or(format!("{}.bin", source.file_stem().unwrap_or(OsStr::new("out")).to_string_lossy()));

    let (interrupts, functions) = load_interrupts(&interrupt_csv::search_path(source, &args.include), &args.interrupts.unwrap_or(vec![]))
        .map_err(|e| { eprintln!(); eprintln!("[ERROR] {}", e); eprintln!(); String::new() })?;

    let mut content = String::new();
    File::open(source).and_then(|mut f| f.read_to_string(&mut content))
        .map_err(|e| { eprintln!(); eprintln!("[ERROR] could not read {}: {}", source_arg, e); eprintln!(); String::new() })?;

    let output = match compile(&name, &content, &interrupts, &functions) {
        Ok(output) => output,
        Err(e) => {
            eprintln!();
            eprintln!("[ERROR] line {}: {}", e.line, e.msg);
            if e.line > 0 { eprintln!(); eprintln!("\t{}: {}", e.line, content.lines().nth(e.line - 1).unwrap_or("").trim()); }
            eprintln!();
            return Err(String::new());
        }
    };

    if let Some(listing_file) = &args.listing {
        File::create(listing_file).and_then(|mut f| f.write_all(output.listing.as_bytes()))
            .map_err(|e| { eprintln!("[ERROR] failed to write listing {}: {}", listing_file, e); String::new() })?;
        println!("[OK] wrote listing to {}", listing_file);
    }

    if args.verbose >= 2 {
        println!();
        println!("{}", output.listing);
    }

    match VirtMach::compile_owned(&name, &output.listing, &interrupts, functions.clone()) {
        Ok((program, _)) => {
            if args.verbose > 0 {
                println!();
                println!("Program \"{}\" ({}b, {} labels, {} jumps, {} memory cells):", name, program.data.len(), output.labels, output.jumps, output.memory_cells());
                for (var, loc) in &output.variables {
                    if var.starts_with('_') { continue; }
                    match loc { Loc::Reg(r) => println!("\t{:10} r{}", var, r), Loc::Mem(a) => println!("\t{:10} memory #{}", var, a) }
                }
                for (array, base, size) in &output.arrays {
                    println!("\t{:10} memory #{}..#{}", format!("{}()", array), base, base + size - 1);
                }
                disassemble(&program);
            }
            File::create(&out_file).and_then(|mut f| f.write_all(program.data))
                .map_err(|e| { eprintln!("failed to write binary file {}: {}", out_file, e); String::new() })?;
            println!("[OK] wrote binary to {}", &out_file);
            Ok(())
        }
        Err(e) => {
            eprintln!();
            eprintln!("[ERROR] generated listing failed to assemble: {:?}", e);
            if let Some(l) = listing_error_line(&e) {
                eprintln!(); eprintln!("\tlisting line #{}: {:?}", l, output.listing.lines().nth(l - 1).unwrap_or(""));
            }
            eprintln!();
            Err(String::new())
        }
    }
}

// ---------------------------------------------------------------------------------------------

#[path = "../lib/interrupt_csv.rs"]
mod interrupt_csv;

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use virtmach::basic::{ Error, Output };
    use crate::interrupt_csv::Functions;
    use virtmach::{ Runtime, RuntimeError, interrupts::{ SoftInterrupt, SoftInterruptFunction, proc, math, surface, string, dummy } };

    /// math.random has one generator for all threads, the tests that depend on its values take turns
    static RANDOM_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    const INTERRUPTS: [&str; 4] = ["proc", "math", "surface", "string"];

    /// Records surface calls instead of drawing.
    struct MockSurface { calls: usize, texts: Vec<String>, draws: Vec<(VMAtom, Vec<VMAtom>)> }

    impl SoftInterrupt for MockSurface {
        fn name(&self) -> &str { surface::NAME }

        fn functions(&self) -> &'static [SoftInterruptFunction<'static>] { &surface::FUNCTIONS }

        fn call(&mut self, vm: &mut VirtMach) {
            let op = vm.stack_pop();
            let function = surface::FUNCTIONS.iter().find(|f| f.no == op).expect("unknown surface function");
            let args: Vec<VMAtom> = (0..function.arguments).map(|_| vm.stack_pop()).collect();
            match op {
                10 => self.texts.push(vm.get_str(args[3] as u8).to_string()),
                15 => { let width = vm.get_bin(args[1] as u8).len() as VMAtom * 4; vm.stack_push(width); vm.stack_push(4); }
                16 => { vm.stack_push(64); vm.stack_push(40); }
                17 => { vm.stack_push(5); vm.stack_push(5); }
                18 => { for v in [0, 0, 64, 40] { vm.stack_push(v); } }
                _ => {}
            }
            self.draws.push((op, args));
            self.calls += 1;
        }
    }

    struct Run {
        output: Output,
        registers: Vec<VMAtom>,
        memory: Vec<VMAtom>,
        state: Runtime,
        error: RuntimeError,
        surface_calls: usize,
        texts: Vec<String>,
        /// every surface call: function number and arguments
        draws: Vec<(VMAtom, Vec<VMAtom>)>,
        /// string entries 0..64 after the run
        strings: Vec<String>
    }

    impl Run {
        fn get(&self, name: &str) -> VMAtom {
            match self.output.variables.iter().find(|v| v.0 == name).unwrap_or_else(|| panic!("no variable {}", name)).1 {
                Loc::Reg(r) => self.registers[r as usize],
                Loc::Mem(a) => self.memory[a]
            }
        }

        fn array(&self, name: &str) -> Vec<VMAtom> {
            let (_, base, size) = self.output.arrays.iter().find(|a| a.0 == name).unwrap();
            self.memory[*base..base + size].to_vec()
        }
    }

    fn include_dirs() -> Vec<PathBuf> {
        vec![PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../include")]
    }

    fn programs_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../examples/programs")
    }

    fn build(src: &str) -> Result<Output, Error> {
        let names: Vec<String> = INTERRUPTS.iter().map(|s| s.to_string()).collect();
        let (interrupts, functions) = load_interrupts(&include_dirs(), &names).unwrap();
        compile("test", src, &interrupts, &functions)
    }

    fn run_src(src: &str, halts: usize) -> Run {
        let names: Vec<String> = INTERRUPTS.iter().map(|s| s.to_string()).collect();
        let (interrupts, functions) = load_interrupts(&include_dirs(), &names).unwrap();
        let output = compile("test", src, &interrupts, &functions).unwrap_or_else(|e| panic!("line {}: {}", e.line, e.msg));
        let (program, _) = VirtMach::compile_owned("test", &output.listing, &interrupts, functions.clone())
            .unwrap_or_else(|e| panic!("{:?}\n{}", e, output.listing));

        let mut vm = VirtMach::new();
        vm.load_program(program);
        let (mut p, mut m, mut s, mut t) = (proc::Interrupt {}, math::Interrupt {}, MockSurface { calls: 0, texts: vec![], draws: vec![] }, string::Interrupt {});
        let [mut time, mut d4, mut d5, mut d6, mut d7, mut periph] = [dummy::Interrupt {}, dummy::Interrupt {}, dummy::Interrupt {}, dummy::Interrupt {}, dummy::Interrupt {}, dummy::Interrupt {}];
        {
            // at their INDEX like the runtime, the numbers come from load_interrupts
            let mut ints: [&mut dyn SoftInterrupt; 10] = [&mut m, &mut p, &mut t, &mut time, &mut d4, &mut d5, &mut d6, &mut d7, &mut periph, &mut s];
            for (slot, int) in ints.iter().enumerate() {
                assert!(virtmach::interrupts::builtin_index(int.name()).is_none_or(|index| index as usize == slot), "{} in slot {}", int.name(), slot);
            }
            for _ in 0..=halts {
                vm.run(1_000_000, &mut ints);
                if vm.state == Runtime::Stp || vm.state == Runtime::Err { break; }
            }
        }
        let error = vm.error.clone();
        let strings = (0..64u8).map(|i| if vm.has_bin(i) { vm.get_str(i).to_string() } else { String::new() }).collect();
        Run { registers: vm.registers.to_vec(), memory: vm.memory.to_vec(), state: vm.state, error, surface_calls: s.calls, texts: s.texts, draws: s.draws, strings, output }
    }

    fn run_file(file: &str, halts: usize) -> Run {
        let src = std::fs::read_to_string(programs_dir().join(file)).unwrap();
        let run = run_src(&src, halts);
        assert_eq!(run.error, RuntimeError::NoError, "{}:\n{}", file, run.output.listing);
        run
    }

    fn assert_finished(run: &Run) {
        assert_eq!(run.state, Runtime::Stp, "program did not reach END\n{}", run.output.listing);
    }

    #[test]
    fn unsupported_types_are_rejected() {
        let cases = [
            ("X = 1.5", "SINGLE literal 1.5"),
            ("X = 1E5", "SINGLE literal 1E5"),
            ("X = 2.5D3", "DOUBLE literal 2.5D3"),
            ("X = 1.5#", "DOUBLE (#) literal 1.5#"),
            ("X = 3!", "SINGLE (!) literal 3!"),
            ("X = 3&", "LONG (&) literal 3&"),
            ("X! = 3", "SINGLE (!) variable X!"),
            ("X# = 3", "DOUBLE (#) variable X#"),
            ("Y& = 3", "LONG (&) variable Y&"),
            ("DIM X AS DOUBLE", "type DOUBLE"),
            ("DIM X AS LONG", "type LONG"),
            ("DIM X AS STRING * 10", "fixed length strings"),
            ("DIM X AS Point", "type POINT"),
            ("SUB f(a AS SINGLE)\nEND SUB", "type SINGLE"),
            ("DEFDBL A-Z", "DEFDBL is not supported"),
            ("DIM X$ AS INTEGER", "does not match its suffix"),
            ("DIM X% AS STRING", "does not match its suffix"),
            ("DEFSTR Z-A", "backwards"),
            ("DIM A(3) AS STRING * 8", "fixed length strings"),
            ("TYPE Point", "user-defined types"),
            ("X = CDBL(1)", "CDBL is not supported"),
        ];
        for (src, msg) in cases {
            match build(src) {
                Ok(_) => panic!("{:?} compiled", src),
                Err(e) => assert!(e.msg.contains(msg), "{:?}: {}", src, e.msg)
            }
        }
        // the supported types still work
        assert!(build("DEFINT A-Z\nDIM X AS INTEGER\nDIM A(3) AS INTEGER\nY% = 5%\nS$ = \"a\"").is_ok());
    }

    #[test]
    fn sum() {
        let run = run_file("ci/sum.bas", 0);
        assert_finished(&run);
        assert_eq!(run.get("SUM"), 55);
        assert_eq!(run.get("FACT"), 5040);
        assert_eq!(run.get("EVENS"), 30);
    }

    #[test]
    fn primes() {
        let run = run_file("ci/primes.bas", 0);
        assert_finished(&run);
        assert_eq!(run.get("COUNT"), 15);
        assert_eq!(run.get("LAST"), 47);
    }

    #[test]
    fn gcd() {
        let run = run_file("ci/gcd.bas", 0);
        assert_finished(&run);
        assert_eq!((run.get("G1"), run.get("G2"), run.get("G3")), (6, 1, 12));
    }

    #[test]
    fn bits() {
        let run = run_file("ci/bits.bas", 0);
        assert_finished(&run);
        let expected = [("DIRECT", 0), ("BAND", 48), ("BOR", 252), ("BXOR", 204), ("BNOT", -1), ("MASK", -1),
            ("SHL", 16), ("SHR", 64), ("QUOT", 14), ("REMD", 2), ("PREC", 12), ("PAREN", -20), ("TRUTH", -2), ("BITS", 5)];
        for (name, value) in expected { assert_eq!(run.get(name), value, "{}", name); }
        assert!(matches!(run.output.variables.iter().find(|v| v.0 == "BITS").unwrap().1, Loc::Mem(_)));
    }

    #[test]
    fn fib() {
        let run = run_file("ci/fib.bas", 0);
        assert_finished(&run);
        assert_eq!(run.array("F"), vec![0, 1, 1, 2, 3, 5, 8, 13, 21, 34, 55, 89]);
        assert_eq!(run.get("TOTAL"), 232);
    }

    #[test]
    fn dice() {
        let _random = RANDOM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let run = run_file("ci/dice.bas", 0);
        assert_finished(&run);
        let (lo, hi, doubles) = (run.get("LO"), run.get("HI"), run.get("DOUBLES"));
        assert!(2 <= lo && lo <= hi && hi <= 12, "lo {} hi {}", lo, hi);
        assert!((0..=100).contains(&doubles));
    }

    #[test]
    fn bounce() {
        let run = run_file("gfx/bounce.bas", 200);
        assert_eq!(run.state, Runtime::Hlt);
        let (x, y) = (run.get("X"), run.get("Y"));
        assert!((1..=62).contains(&x) && (1..=38).contains(&y), "x {} y {}", x, y);
        assert!(run.surface_calls > 600);
    }

    #[test]
    fn ellipse() {
        // the first frame ends at 360 degrees of the unturned ellipse: get_size, set_clip, clear and 36 lines
        let run = run_file("gfx/ellipse.bas", 0);
        assert_eq!(run.state, Runtime::Hlt);
        assert_eq!((run.get("X"), run.get("Y")), (32 + 19, 20));
        assert_eq!(run.surface_calls, 39);
        // turned by 30 * 3 degrees the same point is at the bottom
        let run = run_file("gfx/ellipse.bas", 30);
        assert_eq!((run.get("X"), run.get("Y")), (32, 20 + 19));
    }

    #[test]
    fn pong() {
        let _lock = DICT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let _random = RANDOM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let run = run_file("gfx/pong.bas", 3000);
        assert_eq!(run.state, Runtime::Hlt);
        let (ly, ry, bx, by) = (run.get("LY"), run.get("RY"), run.get("BX"), run.get("BY"));
        assert!((0..=35).contains(&ly) && (0..=35).contains(&ry), "paddles {} {}", ly, ry);
        assert!((-1..=64).contains(&bx) && (0..=39).contains(&by), "ball {} {}", bx, by);
        assert!((0..=9).contains(&run.get("SL")) && (0..=9).contains(&run.get("SR")));
        assert_eq!(run.output.strings, [" "], "only the blank STR$ puts in front of numbers");
        let last: Vec<String> = run.texts[run.texts.len() - 2..].to_vec();
        assert_eq!(last, [run.get("SL").to_string(), run.get("SR").to_string()]);
    }

    #[test]
    fn strings() {
        let run = run_file("ci/strings.bas", 0);
        assert_finished(&run);
        let expected = [("HELLO$", 0), ("SAME$", 0), ("OTHER$", 1), ("LEN1", 13), ("LEN2", 12), ("LEN3", 0)];
        for (name, value) in expected { assert_eq!(run.get(name), value, "{}", name); }
        assert_eq!(run.output.strings, ["Hello, World!", "Hi; \"quoted\"", "", "same entry"]);
        assert_eq!(run.surface_calls, 2);
        assert!(run.output.listing.contains("#db \"Hi\", 0x3b, 0x20, 0x22"), "{}", run.output.listing);

        let names: Vec<String> = INTERRUPTS.iter().map(|s| s.to_string()).collect();
        let (interrupts, functions) = load_interrupts(&include_dirs(), &names).unwrap();
        let (program, _) = VirtMach::compile_owned("strings", &run.output.listing, &interrupts, functions).unwrap();
        assert_eq!(program.get_fixed_bin_count(), 4);
        for (i, text) in run.output.strings.iter().enumerate() { assert_eq!(program.get_bin(i as u8), text.as_bytes()); }

        let many: String = (0..256).map(|i| format!("S = \"{}\"\n", i)).collect();
        assert!(build(&many).err().unwrap().msg.contains("out of string slots"));
    }

    /// DICT with the dynamic strings is one global, tests writing to it must not run at the same time
    static DICT_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// Counts the bytes allocated and not yet freed by the current thread
    struct CountingAllocator;

    thread_local! { static LIVE_BYTES: std::cell::Cell<isize> = const { std::cell::Cell::new(0) }; }

    unsafe impl std::alloc::GlobalAlloc for CountingAllocator {
        unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
            let _ = LIVE_BYTES.try_with(|b| b.set(b.get() + layout.size() as isize));
            unsafe { std::alloc::System.alloc(layout) }
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: std::alloc::Layout) {
            let _ = LIVE_BYTES.try_with(|b| b.set(b.get() - layout.size() as isize));
            unsafe { std::alloc::System.dealloc(ptr, layout) }
        }
    }

    #[global_allocator]
    static ALLOCATOR: CountingAllocator = CountingAllocator;

    #[test]
    fn dynamic_strings_are_freed() {
        let _lock = DICT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let live_after = |n: usize| {
            let before = LIVE_BYTES.with(|b| b.get());
            let run = run_src(&format!("REM\nFOR I = 1 TO {}\n  S$ = STR$(I) + \"x\"\nNEXT I\nEND", n), 0);
            assert_eq!(run.error, RuntimeError::NoError);
            drop(run);
            LIVE_BYTES.with(|b| b.get()) - before
        };
        live_after(100);
        let (small, large) = (live_after(100), live_after(5000));
        assert!(large - small < 256, "{} bytes more still allocated after 5000 instead of 100 string operations", large - small);

        let run = run_src("REM\nA$ = STR$(12345)\nstring.substr(A$, A$, 1, 3)\nEND", 0);
        assert_eq!(run.strings[run.get("A$") as usize], "12", "STR$(12345) is \" 12345\"");
    }

    #[test]
    fn dictionary_out_of_bound() {
        let _lock = DICT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let cases = [
            ("reading an entry that was never set", "REM\nL = string.get_length(200)\nEND"),
            ("writing a fixed #db entry", "REM\nA = \"fixed\"\nstring.format(A, 5)\nEND"),
            ("joining an entry that was never set", "REM\nA$ = STR$(1) + \"x\"\nstring.concat(A$, A$, 201)\nEND"),
        ];
        for (what, src) in cases {
            let run = run_src(src, 0);
            assert_eq!(run.error, RuntimeError::DictionaryOutOfBound, "{}", what);
        }
        let run = run_src("REM\nA$ = STR$(1)\nL = LEN(A$) + LEN(\"ab\")\nEND", 0);
        assert_eq!((run.error.clone(), run.get("L")), (RuntimeError::NoError, 4), "STR$(1) is \" 1\"");
    }

    #[test]
    fn string_functions() {
        let _lock = DICT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let run = run_file("ci/strfuncs.bas", 0);
        assert_finished(&run);
        let text = |name: &str| run.strings[run.get(name) as usize].clone();
        let expected = [("A$", " 1234"), ("B$", "-56"), ("M$", "World"), ("R$", "World!"), ("F$", "He"), ("T$", "bc"), ("X$", "abc"), ("D$", " 2"),
            ("MIN$", "-32768"), ("ZERO$", " 0"),
            ("J$", "Hello, World!"), ("S$", "123")];
        for (name, value) in expected { assert_eq!(text(name), value, "{}", name); }
        assert_eq!(run.get("L"), 5);
        assert!(run.get("A$") as usize >= run.output.strings.len(), "results are dynamic entries after the #db entries");

        // the argument is evaluated once even though STR$ needs it twice, and only numbers that are not negative get the blank
        let run = run_src("REM once\nDEFINT A-Z\nREQ string\nFUNCTION Bump\n  SHARED N\n  N = N + 1\n  Bump = N * 2 - 5\nEND FUNCTION\nA$ = STR$(Bump)\nB$ = STR$(Bump)\nC$ = STR$(3 - 10)\nD$ = STR$(2 * 3)\n", 0);
        let text = |name: &str| run.strings[run.get(name) as usize].clone();
        assert_eq!(run.get("N"), 2);
        assert_eq!((text("A$").as_str(), text("B$").as_str(), text("C$").as_str(), text("D$").as_str()), ("-3", "-1", "-7", " 6"));

        let e = compile("test", "A$ = STR$(1)", &[], &Functions::new()).err().unwrap();
        assert!(e.msg.contains("add string to the interrupts"), "{}", e.msg);
        let many: String = (0..256).map(|i| format!("A$ = STR$({})\n", i)).collect();
        assert!(build(&many).err().unwrap().msg.contains("at most 255 fit the string indices"));
        assert!(build("A$ = MID$(\"x\")").err().unwrap().msg.contains("MID$ takes"));
        assert!(build("A$ = \"a\" + 1").err().unwrap().msg.contains("cannot join a string and a number"));
        assert!(build("N = 1 : A$ = N + \"a\"").err().unwrap().msg.contains("cannot join a string and a number"));
        assert!(!build("A$ = \"a\" : A$ = A$ + \"b\"").unwrap().listing.contains("add #"), "string join must not become a native add");
    }

    #[test]
    fn declared_types() {
        let _lock = DICT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let run = run_file("ci/types.bas", 0);
        assert_finished(&run);
        let text = |name: &str| run.strings[run.get(name) as usize].clone();
        assert_eq!(text("S$"), "w5");
        assert_eq!(text("T$"), "abab");
        assert_eq!(text("Q1$"), "dq");
        assert_eq!(text("Q2$"), "dq!");
        assert_eq!(run.get("N"), 5);
        assert_eq!(run.get("M"), 6);
        assert_eq!(run.get("R"), 0, "DEFINT turns Q back into an integer");
        assert_eq!(run.array("A$").len(), 3);
        // the same name is a string in one SUB and an integer in another
        let run = run_src("REM scopes\nSUB a(X AS STRING)\n  Y = LEN(X)\nEND SUB\nSUB b(X AS INTEGER)\n  Z = X + 1\nEND SUB\na \"abc\"\nb 4\n", 0);
        let names: Vec<&str> = run.output.variables.iter().map(|v| v.0.as_str()).collect();
        assert!(names.contains(&"A.X$") && names.contains(&"B.X"), "{:?}", names);
    }

    #[test]
    fn control_flow() {
        let run = run_file("ci/control.bas", 0);
        assert_finished(&run);
        let expected = [("SIZES", 1234), ("KINDS", 11234), ("CS", 2), ("DOWHILE", 10), ("DOUNTIL", 12), ("DU", 5),
            ("LOOPWHILE", 101), ("FOUND", 7), ("EXITDO", 4), ("EXITWHILE", 6), ("ONSUM", 111), ("STEPS", 18), ("BACK", 5)];
        for (name, value) in expected { assert_eq!(run.get(name), value, "{}", name); }
    }

    #[test]
    fn subroutines() {
        let _lock = DICT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let run = run_file("ci/subs.bas", 0);
        assert_finished(&run);
        let expected = [("G", 12), ("F", 120), ("TOTAL", 24), ("C1", 2), ("SQ", 13), ("A", 2), ("B", 1), ("S", -99)];
        for (name, value) in expected { assert_eq!(run.get(name), value, "{}", name); }
        assert_eq!(run.strings[run.get("H$") as usize], "Hello, World!");
        // GCD's parameters are its own, not the main program's A and B
        assert!(run.output.variables.iter().any(|v| v.0 == "GCD.A"));
    }

    #[test]
    fn robust_comparisons() {
        let _lock = DICT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let run = run_src("REM
            A = 30000 : B = -30000 : R = 0
            IF A > B THEN R = R + 1
            IF B < A THEN R = R + 2
            IF NOT (A < B) THEN R = R + 4
            IF A >= B AND B <= A THEN R = R + 8
            LOW = -32767 - 1 : HIGH = 32767
            IF LOW < HIGH AND HIGH > LOW THEN R = R + 16
            V = (A > B) + (B > A)
            LO = MIN(A, B) : HI = MAX(A, B)
            S = 0
            IF \"abc\" < \"abd\" THEN S = S + 1
            IF \"b\" > \"abc\" THEN S = S + 2
            IF STR$(12) = \" 12\" THEN S = S + 4
            IF \"ab\" < \"abc\" THEN S = S + 8
            IF \"x\" <> \"x\" THEN S = S + 100
            X$ = \"hi\" : Y$ = STR$(5)
            IF X$ = \"hi\" AND Y$ = \" 5\" AND X$ <> Y$ THEN S = S + 16
            END", 0);
        assert_finished(&run);
        assert_eq!((run.get("R"), run.get("V"), run.get("S")), (31, -1, 31));
        assert_eq!((run.get("LO"), run.get("HI")), (-30000, 30000));

        let mut src = String::from("REM\nN = 0\n");
        for i in 0..300 { src.push_str(&format!("IF N = {} THEN N = N + 1\n", i)); }
        src.push_str("END\n");
        let run = run_src(&src, 0);
        assert!(run.output.jumps > 128, "{} jumps", run.output.jumps);
        assert_finished(&run);
        assert_eq!(run.get("N"), 300);
    }

    #[test]
    fn subroutine_and_control_errors() {
        let cases = [
            ("FUNCTION F(N)\nF = F(N - 1)\nEND FUNCTION", "recursion is not supported"),
            ("SUB A\nB\nEND SUB\nSUB B\nA\nEND SUB", "recursion is not supported"),
            ("SUB S\nEND SUB\nX = S", "is a SUB and has no value"),
            ("SUB S\nEND SUB\nX = S(1)", "only a FUNCTION can be used"),
            ("FUNCTION F(A)\nEND FUNCTION\nX = F", "call it with its arguments"),
            ("FUNCTION F(A, B)\nEND FUNCTION\nX = F(1)", "expects 2 argument(s)"),
            ("SUB S\nSUB T\nEND SUB", "cannot be nested"),
            ("FOR I = 1 TO 2\nSUB S\nEND SUB\nNEXT", "cannot be inside a block"),
            ("SUB MIN\nEND SUB", "built-in function"),
            ("SUB S\nEND SUB\nSUB S\nEND SUB", "defined twice"),
            ("FOO 1", "unknown SUB FOO"),
            ("EXIT FOR", "EXIT FOR outside of FOR"),
            ("WHILE 1\nSUB S\nEXIT WHILE\nEND SUB\nWEND", "cannot be inside a block"),
            ("CASE 1", "CASE without SELECT CASE"),
            ("SELECT CASE 1\nCASE ELSE\nCASE 2\nEND SELECT", "CASE after CASE ELSE"),
            ("DO\nX = 1", "DO without LOOP"),
            ("GOTO NOWHERE", "label NOWHERE does not exist"),
            ("A:\nA:", "duplicate label A"),
            ("FOR I = 1 TO 2 STEP 0\nNEXT", "STEP must not be 0"),
            ("IF \"a\" = 1 THEN X = 1", "cannot compare a string with a number"),
            ("X = RND(1)", "RND takes"),
            ("SHARED X", "SHARED outside"),
            ("REQ math, joystick", "requires the joystick interrupt, which is not available (available: proc, math, surface, string)"),
        ];
        for (src, msg) in cases {
            match build(src) {
                Ok(_) => panic!("{:?} compiled", src),
                Err(e) => assert!(e.msg.contains(msg), "{:?}: {}", src, e.msg)
            }
        }
        assert!(build("X = 1 : STOP").unwrap().listing.contains("brk"));
        assert!(build("REQ math, surface\nREQ STRING\nX = 1").is_ok());
        let e = compile("test", "X = 1\nREQ math", &[], &Functions::new()).err().unwrap();
        assert_eq!(e.line, 2);
        assert!(e.msg.contains("(available: none)"), "{}", e.msg);
    }

    #[test]
    fn detection() {
        use virtmach::basic::is_basic;
        for src in ["REM x\nX = 1", "\n\n  rem lower case", "10 REM numbered", "REM", "'x\nREM"] {
            assert_eq!(is_basic(src), !src.starts_with('\''), "{:?}", src);
        }
        for src in ["; listing\nREM", "reg r0", "REMARK = 1", "", "#req math", "X = 1"] {
            assert!(!is_basic(src), "{:?}", src);
        }
    }

    #[test]
    fn integrated_compile() {
        let names: Vec<String> = INTERRUPTS.iter().map(|s| s.to_string()).collect();
        let (interrupts, functions) = load_interrupts(&include_dirs(), &names).unwrap();
        let basic = std::fs::read_to_string(programs_dir().join("ci/gcd.bas")).unwrap();
        let listing = compile("gcd", &basic, &interrupts, &functions).unwrap().listing;

        let from_basic = VirtMach::compile_owned("gcd", &basic, &interrupts, functions.clone()).unwrap().0;
        let from_listing = VirtMach::compile_owned("gcd", &listing, &interrupts, functions.clone()).unwrap().0;
        assert_eq!(from_basic.data, from_listing.data);

        match VirtMach::compile_owned("bad", "REM broken\nX = math.and(1)", &interrupts, functions.clone()) {
            Err(ListingError::Basic(2, msg)) => assert!(msg.contains("expects 2 argument(s)"), "{}", msg),
            other => panic!("{:?}", other.map(|_| ()))
        }
        assert!(matches!(VirtMach::compile_owned("listing", "X = 1", &interrupts, functions), Err(ListingError::IllegalOp(1, _))));
    }

    #[test]
    fn conditions() {
        let run = run_src("
            A = 5 : B = 7 : R = 0
            IF A < B THEN R = R + 1
            IF A > B THEN R = R + 100
            IF A <= 5 AND B >= 7 THEN R = R + 2
            IF A = 4 OR B <> 7 THEN R = R + 100 ELSE R = R + 4
            IF NOT A = B THEN R = R + 8
            IF A - 5 THEN R = R + 100
            IF B THEN
              R = R + 16
            ELSE
              R = R + 100
            END IF
            V = ABS(-9) + ABS(3)
            N = -A
            END", 0);
        assert_finished(&run);
        assert_eq!(run.get("R"), 31);
        assert_eq!(run.get("V"), 12);
        assert_eq!(run.get("N"), -5);
    }

    #[test]
    fn peripherals() {
        // only compiled and assembled, running them needs the simulators of examples/helpers.rs
        let names: Vec<String> = ["proc", "math", "string", "time", "periph"].iter().map(|s| s.to_string()).collect();
        let (interrupts, functions) = load_interrupts(&include_dirs(), &names).unwrap();
        let files: Vec<PathBuf> = std::fs::read_dir(programs_dir().join("peripherals")).unwrap()
            .map(|entry| entry.unwrap().path()).filter(|path| path.extension().is_some_and(|e| e == "bas")).collect();
        assert!(files.len() >= 5, "{:?}", files);
        for file in files {
            let src = std::fs::read_to_string(&file).unwrap();
            let output = compile("test", &src, &interrupts, &functions).unwrap_or_else(|e| panic!("{}: line {}: {}", file.display(), e.line, e.msg));
            VirtMach::compile_owned("test", &output.listing, &interrupts, functions.clone())
                .unwrap_or_else(|e| panic!("{}: {:?}\n{}", file.display(), e, output.listing));
        }
    }

    #[test]
    fn ci_programs_compile() {
        // every program of examples/programs/ci, so a new one is picked up without touching this test:
        // BASIC sources are compiled to a listing and assembled, listings are assembled directly
        let names: Vec<String> = INTERRUPTS.iter().chain(&["time"]).map(|s| s.to_string()).collect();
        let (interrupts, functions) = load_interrupts(&include_dirs(), &names).unwrap();
        let mut files: Vec<PathBuf> = std::fs::read_dir(programs_dir().join("ci")).unwrap()
            .map(|entry| entry.unwrap().path()).filter(|path| path.extension().is_some_and(|e| e == "bas" || e == "txt")).collect();
        files.sort();
        assert!(files.len() >= 12, "ci programs are missing: {:?}", files);
        for file in files {
            let src = std::fs::read_to_string(&file).unwrap();
            let name = file.file_stem().unwrap().to_string_lossy().to_string();
            let direct = VirtMach::compile_owned(&name, &src, &interrupts, functions.clone())
                .unwrap_or_else(|e| panic!("{}: {:?}", file.display(), e)).0;
            if file.extension().is_some_and(|e| e == "bas") {
                let output = compile(&name, &src, &interrupts, &functions).unwrap_or_else(|e| panic!("{}: line {}: {}", file.display(), e.line, e.msg));
                let assembled = VirtMach::compile_owned(&name, &output.listing, &interrupts, functions.clone())
                    .unwrap_or_else(|e| panic!("{}: {:?}\n{}", file.display(), e, output.listing)).0;
                assert_eq!(direct.data, assembled.data, "{}: compiling the source and its listing differ", file.display());
            }
            assert!(!direct.data.is_empty(), "{} compiled to nothing", file.display());
        }
    }

    #[test]
    fn syntax() {
        let run = run_file("ci/syntax.bas", 0);
        assert_finished(&run);
        for (name, value) in [("A", 1), ("B", 16), ("C", 2), ("D", 17), ("E", 1), ("F", 2), ("G", 2), ("H", 2)] {
            assert_eq!(run.get(name), value, "{}", name);
        }
        assert!(run.output.strings.contains(&String::from("a;b")) && run.output.strings.contains(&String::from("say \"hi\"")), "{:?}", run.output.strings);
    }

    #[test]
    fn extensions() {
        let _lock = DICT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let run = run_file("ci/extensions.bas", 1);
        for (name, value) in [("A", 16), ("SAMECOL", -1), ("M", -11), ("R", 5), ("SHL", 16), ("SHR", 64), ("DIRECT", 0)] {
            assert_eq!(run.get(name), value, "{}", name);
        }
        assert_eq!(run.strings[run.get("C$") as usize], "Hello, World!");
        assert_eq!(run.texts, ["Hello, World!"]);
        assert_eq!(run.output.strings[run.get("HELLO") as usize], "Hello");
    }

    #[test]
    fn data_read_restore() {
        let _lock = DICT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let run = run_file("ci/data.bas", 0);
        assert_finished(&run);
        for (name, value) in [("A", 10), ("B", -20), ("C", 255), ("AGE", 31), ("TOTAL", 10), ("FIRST", 7), ("AGAIN", 8), ("X", 10)] {
            assert_eq!(run.get(name), value, "{}", name);
        }
        let text = |name: &str| run.strings[run.get(name) as usize].clone();
        assert_eq!((text("N$"), text("W$"), text("Q$")), (String::from("Ann"), String::from("word"), String::from("a, b")));
        assert_eq!(run.array("V"), [0, 1, 2, 3, 4]);
        assert_eq!(run.array("_DATA").len(), 13);

        // reading past the last item stops the vm with an error
        let run = run_src("REM past\nDEFINT A-Z\nREQ math\nREAD A, B\nDATA 1\n", 0);
        assert_eq!(run.error, RuntimeError::InterruptError);
        assert_eq!(run.get("A"), 1);
        // DATA in a SUB belongs to the same list, READ in a SUB works
        let run = run_src("REM sub\nDEFINT A-Z\nREQ math\nSUB r\n  READ X\n  SHARED Y\n  Y = X\nEND SUB\nr\nr\nDATA 5, 6\n", 0);
        assert_eq!(run.get("Y"), 6);
        // READ needs the math interrupt for the end of the DATA
        let e = compile("test", "DATA 1\nREAD A", &[], &Functions::new()).err().unwrap();
        assert!(e.msg.contains("READ needs the math interrupt"), "{}", e.msg);
    }

    #[test]
    fn randomize() {
        let _lock = RANDOM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let run = run_file("ci/random.bas", 0);
        assert_finished(&run);
        assert_eq!((run.get("SAME"), run.get("DIFFERENT")), (-1, -1));
    }

    #[test]
    fn input_and_unsupported_statements_fail_with_any_console() {
        // the programs of ci/invalid cover them, this checks that they also fail inside a PRINT or a single line IF
        for src in ["IF 1 THEN INPUT A", "X = INKEY$ + \"a\"", "IF 1 THEN CIRCLE (1, 1), 2", "A$ = INPUT$(1)"] {
            assert!(build(src).is_err(), "{} compiled", src);
        }
    }

    /// Without --cfg basic_console="surface" every console and drawing statement is an error
    #[cfg(not(basic_console = "surface"))]
    #[test]
    fn console_statements_are_rejected() {
        let cases = [("PRINT \"x\"", "PRINT"), ("? 1", "PRINT"), ("PRINT USING \"##\"; 1", "PRINT"), ("LOCATE 1, 1", "LOCATE"),
            ("WIDTH 80", "WIDTH"), ("VIEW PRINT 1 TO 5", "VIEW PRINT"), ("IF 1 THEN CLS ELSE PRINT \"a\"", "PRINT")];
        for (src, word) in cases {
            let e = build(src).err().unwrap_or_else(|| panic!("{} compiled", src));
            assert!(e.msg.contains(&format!("{} is not supported", word)) && !e.msg.contains("RUSTFLAGS"), "{}: {}", src, e.msg);
        }
    }

    #[cfg(basic_console = "surface")]
    fn text_draws(run: &Run) -> Vec<(VMAtom, VMAtom, String)> {
        let positions = run.draws.iter().filter(|d| d.0 == 10).map(|d| (d.1[0], d.1[1]));
        positions.zip(run.texts.iter()).map(|((x, y), t)| (x, y, t.clone())).collect()
    }

    #[cfg(basic_console = "surface")]
    #[test]
    fn print_on_the_surface() {
        let _lock = DICT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let run = run_file("console/print.bas", 0);
        assert_finished(&run);
        // the font is 3 pixels wide with 1 pixel between the characters, 4 high with 1 pixel between the lines
        let expected = [(0, 0, "Hello"), (0, 5, "a"), (4, 5, "b"), (0, 10, " 42 "), (0, 15, " 1 "), (56, 15, " 2 "), (0, 20, "x"), (4, 20, "y"),
            (0, 25, "q"), (0, 35, "end")];
        let draws = text_draws(&run);
        assert_eq!(draws, expected.iter().map(|(x, y, t)| (*x, *y, t.to_string())).collect::<Vec<_>>());
        assert_eq!(run.draws[0], (0, vec![virtmach::color::Color::vga(0)]), "CLS clears with the background, black to begin with");
        assert_eq!((run.get("_PX"), run.get("_PY")), (0, 40));
        // the cursor moves by the font's metrics, the surface is not asked for the size of a text
        assert_eq!(run.draws.iter().filter(|d| d.0 == 15).count(), 0);
    }

    #[test]
    fn drawing_on_the_surface() {
        let run = run_file("ci/drawing.bas", 0);
        assert_finished(&run);
        let calls: Vec<(VMAtom, Vec<VMAtom>)> = run.draws.iter().filter(|d| !(15..=18).contains(&d.0)).cloned().collect();
        // COLOR #000000, #ffffff: black on white. Colors are atoms, a number is an index of the VGA palette
        use virtmach::color::Color;
        let (black, white) = (Color::parse("#000000"), Color::parse("#ffffff"));
        let expected: Vec<(VMAtom, Vec<VMAtom>)> = vec![
            (20, vec![1, black]),                 // COLOR sets the palette entries of text 1 and 0
            (20, vec![0, white]),
            (0, vec![white]),                     // CLS clears with the background
            (1, vec![10, 20, black]),             // PSET in the foreground
            (1, vec![11, 22, Color::vga(5)]),     // PSET STEP, relative to the last point, color index 5
            (1, vec![3, 4, white]),               // PRESET in the background
            (4, vec![0, 0, 10, 5, black]),        // LINE
            (4, vec![10, 5, 20, 8, Color::vga(7)]), // LINE from the last point
            (2, vec![5, 5, 11, 11, black]),       // B, STEP is relative to the first corner
            (3, vec![25, 20, 6, 6, Color::vga(1)]), // BF with the corners the other way round
            (19, vec![2, 3, 10, 6]),              // VIEW
            (19, vec![0, 0, 64, 40])              // VIEW without arguments, SCREEN draws nothing
        ];
        assert_eq!(calls, expected);
    }

    #[test]
    fn colors_of_drawing_statements() {
        use virtmach::color::Color;
        let run = run_file("ci/colors.bas", 0);
        assert_finished(&run);
        let calls: Vec<(VMAtom, Vec<VMAtom>)> = run.draws.iter().filter(|d| !(15..=18).contains(&d.0)).cloned().collect();
        let (blue, brown) = (Color::parse("#123456"), Color::parse("#654321"));
        let expected: Vec<(VMAtom, Vec<VMAtom>)> = vec![
            (1, vec![1, 1, Color::parse("#ff0000")]), // a color literal
            (1, vec![2, 1, Color::parse("#00ff00")]), // a constant that was defined as one
            (1, vec![3, 1, Color::vga(4)]),           // a constant index
            (1, vec![4, 1, Color::vga(5)]),           // folded: RED + 1
            (4, vec![0, 0, 5, 5, Color::vga(15)]),
            (1, vec![5, 1, Color::vga(15)]),          // white on black to begin with
            (1, vec![6, 1, Color::vga(0)]),
            (20, vec![1, blue]),                      // COLOR with a foreground only
            (1, vec![7, 1, blue]),
            (20, vec![0, brown]),                     // COLOR with a background only
            (1, vec![8, 1, brown]),
            (0, vec![brown])
        ];
        assert_eq!(calls, expected);
    }

    #[test]
    fn drawing_errors() {
        let cases = [("LINE (0, 0)-(5, 5), 1, B, 3", "line styles"), ("VIEW (0, 0)-(5, 5), 1", "VIEW colors and borders"),
            ("VIEW (0, 0)-STEP(5, 5)", "absolute coordinates"), ("COLOR 1, 2, 3", "COLOR takes")];
        for (src, msg) in cases {
            let e = build(&format!("REQ surface, math\n{}", src)).err().unwrap_or_else(|| panic!("{} compiled", src));
            assert!(e.msg.contains(msg), "{}: {}", src, e.msg);
        }
        // the statements need the surface functions they are built on
        for src in ["PSET (1, 2)", "LINE (0, 0)-(1, 1)", "VIEW", "COLOR 1", "CLS"] {
            let e = compile("test", src, &[], &Functions::new()).err().unwrap();
            assert!(e.msg.contains("add the surface interrupt"), "{}: {}", src, e.msg);
        }
        // SCREEN compiles to nothing
        let code = |src: &str| build(src).unwrap().listing.lines().filter(|l| !l.trim_start().starts_with(';')).map(String::from).collect::<Vec<_>>();
        assert_eq!(code("SCREEN 12\nX = 1"), code("X = 1"));
    }

    #[cfg(basic_console = "surface")]
    #[test]
    fn console_errors_on_the_surface() {
        let cases = [("PRINT USING \"##\"; 1", "PRINT USING is not supported"), ("PRINT TAB(5)", "TAB is not supported")];
        for (src, msg) in cases {
            let e = build(&format!("REQ surface, math\n{}", src)).err().unwrap_or_else(|| panic!("{} compiled", src));
            assert!(e.msg.contains(msg), "{}: {}", src, e.msg);
        }
        let e = compile("test", "PRINT 1", &[], &Functions::new()).err().unwrap();
        assert!(e.msg.contains("add the surface interrupt"), "{}", e.msg);
        // without the string interrupt there is no LEN to measure a text with
        let names: Vec<String> = ["proc", "math", "surface"].iter().map(|s| s.to_string()).collect();
        let (interrupts, functions) = load_interrupts(&include_dirs(), &names).unwrap();
        let e = compile("test", "PRINT \"a\"", &interrupts, &functions).err().unwrap();
        assert!(e.msg.contains("PRINT needs string.get_length, add the string interrupt"), "{}", e.msg);
        // LOCATE, WIDTH and VIEW PRINT compile to nothing
        let code = |src: &str| build(src).unwrap().listing.lines().filter(|l| !l.trim_start().starts_with(';')).map(String::from).collect::<Vec<_>>();
        assert_eq!(code("LOCATE 1, 1\nWIDTH 80\nVIEW PRINT 1 TO 5\nX = 1"), code("X = 1"));
    }

    #[test]
    fn peek_and_poke() {
        let run = run_file("ci/peekpoke.bas", 0);
        assert_finished(&run);
        assert_eq!(run.output.arrays[0].1, 0, "the array starts at address 0");
        for (name, value) in [("X", 77), ("Y", 5), ("Z", 9), ("W", 77)] { assert_eq!(run.get(name), value, "{}", name); }
        assert_eq!(&run.array("A")[2..], [77, 5, 9]);
        // outside of the memory only the vm complains
        for src in ["X = PEEK(-1)", "POKE -1, 5", "X = PEEK(32000)", "POKE 32000, 5", "I = 32000 : X = PEEK(I)"] {
            let run = run_src(&format!("REM\nDEFINT A-Z\n{}\nEND", src), 0);
            assert_eq!(run.error, RuntimeError::MemoryOutOfBounds, "{}", src);
        }
    }

    #[test]
    fn semicolon_is_no_comment() {
        // it separates the items of PRINT, which only exists on the surface, so it must not hide the rest of a line
        assert!(build("X = 1 ; Y = 2").is_err());
        assert!(build("X = 1 ' Y = 2").is_ok());
    }

    #[test]
    fn declarations() {
        let _lock = DICT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let run = run_file("ci/decl.bas", 0);
        assert_finished(&run);
        for (name, value) in [("R", 63), ("S", 6), ("M", 63)] { assert_eq!(run.get(name), value, "{}", name); }
        assert_eq!(run.strings[run.get("TXT$") as usize], "Hi!");
        assert_eq!(&run.array("ARR")[1..], [3, 6, 9, 12, 15, 18], "OPTION BASE 1: ARR(1..6)");
        // module variables of DIM SHARED are not prefixed with the SUB they are used in
        assert!(run.output.variables.iter().all(|v| !v.0.ends_with(".USED") && !v.0.ends_with(".TXT$")), "{:?}", run.output.variables);
    }

    #[test]
    fn operators() {
        let _lock = DICT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let run = run_file("ci/operators.bas", 0);
        assert_finished(&run);
        let expected = [("A", 3), ("B", -3), ("C", 3), ("D", 1), ("F", 1), ("Y", 5), ("G", -1), ("H", 0), ("I", 0), ("J", -1), ("K", -1),
            ("L", -7), ("M", -5), ("P", 2), ("T", 5), ("U", 31), ("V", 15), ("W", 10), ("X", 15),
            ("SC1", 1), ("SC2", 1), ("SC3", 1), ("SC4", 1), ("LOOPS", 3), ("SC5", 2)];
        for (name, value) in expected { assert_eq!(run.get(name), value, "{}", name); }
    }

    #[test]
    fn constants() {
        let folded = "CONST A = 2 + 3 * 4, B = A \\ 3, C = -A, D = A MOD 4 << 1, E = NOT 0, F = 2 ^ 5, G = (A > 3), H = A = 14, I = A < 3
CONST S$ = \"a\" + \"b\", L = &HFF AND 15
CONST SZ = 2 * 2
DIM X(SZ), Y(SZ + 1) AS INTEGER
X(SZ) = 7
R = A + B + SZ
X1 = B : X2 = C : X3 = D : X4 = E : X5 = F : X6 = G : X7 = H : X8 = I : X9 = L : T$ = S$
SUB t
  Q = A + SZ
END SUB
t
";
        let _lock = DICT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let run = run_src(&format!("REM consts\n{}", folded), 0);
        for (name, value) in [("R", 22), ("T.Q", 18), ("X1", 4), ("X2", -14), ("X3", 4), ("X4", -1), ("X5", 32), ("X6", -1), ("X7", -1), ("X8", 0), ("X9", 15)] {
            assert_eq!(run.get(name), value, "{}", name);
        }
        assert_eq!(run.strings[run.get("T$") as usize], "ab");
        assert_eq!(run.array("X").len(), 5);
        assert_eq!(run.array("Y").len(), 6);
    }

    #[test]
    fn invalid_programs_fail() {
        // every file of ci/invalid names the error it must fail with in its first line: REM expect: <part of the message>
        let mut files: Vec<PathBuf> = std::fs::read_dir(programs_dir().join("ci/invalid")).unwrap()
            .map(|entry| entry.unwrap().path()).filter(|path| path.extension().is_some_and(|e| e == "bas")).collect();
        files.sort();
        assert!(files.len() >= 10, "{:?}", files);
        for file in files {
            let src = std::fs::read_to_string(&file).unwrap();
            let expected = src.lines().next().and_then(|l| l.strip_prefix("REM expect:")).unwrap_or_else(|| panic!("{} has no REM expect: line", file.display())).trim();
            match build(&src) {
                Ok(_) => panic!("{} compiled, expected {:?}", file.display(), expected),
                Err(e) => assert!(e.msg.contains(expected), "{}: expected {:?}, got {}", file.display(), expected, e.msg)
            }
        }
    }

    #[test]
    fn errors() {
        let cases = [
            ("X = foo.bar(1)", "unknown interrupt function"),
            ("X = math.foo(1)", "has no function"),
            ("X = math.and(1)", "expects 2 argument(s)"),
            ("surface.clear()", "expects 1 argument(s)"),
            ("X = surface.get_size()", "cannot be used in an expression"),
            ("FOR I = 1 TO 3\nNEXT J", "does not match"),
            ("GOTO 100", "does not exist"),
            ("WHILE 1", "without WEND"),
            ("X = Y(1)", "unknown function"),
            ("DIM A(3)\nA(4) = 1", "out of bounds"),
            ("X = 40000", "out of range"),
            ("X = = 1", "syntax error"),
        ];
        for (src, msg) in cases {
            match build(src) {
                Ok(_) => panic!("{:?} compiled", src),
                Err(e) => assert!(e.msg.contains(msg), "{:?}: {}", src, e.msg)
            }
        }
        let e = compile("test", "X = 2 * 3", &[], &Functions::new()).err().unwrap();
        assert!(e.msg.contains("add math to the interrupts"), "{}", e.msg);
    }
}
