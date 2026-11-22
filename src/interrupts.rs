use crate::{ VirtMach, VMAtom, Storage, Ram };

pub mod dummy;
pub mod proc;
pub mod math;
#[cfg(feature = "int_math_random")]
pub mod math_random;
#[cfg(feature = "int_math_trig")]
pub mod math_trig;
pub mod string;
pub mod surface;
pub mod periph;
pub mod time;

/// Built-in interrupts have fixed numbers, their INDEX, so programs compiled with any selection of
/// them run on every runtime that places them there. Unused numbers are filled with dummy.
pub const BUILTIN_SLOTS: u8 = 10;

/// Interrupt numbers are 4 bits in the INT instruction
pub const MAX_SLOTS: u8 = 16;

/// The INDEX of a built-in interrupt by its NAME
pub fn builtin_index(name: &str) -> Option<u8> {
    match name {
        n if n == math::NAME => Some(math::INDEX),
        n if n == proc::NAME => Some(proc::INDEX),
        n if n == string::NAME => Some(string::INDEX),
        n if n == time::NAME => Some(time::INDEX),
        n if n == surface::NAME => Some(surface::INDEX),
        n if n == periph::NAME => Some(periph::INDEX),
        _ => None
    }
}

#[cfg(feature = "compile")]
extern crate std;
#[cfg(feature = "compile")]
use std::{ format, string::String, vec::Vec };

/// The number of each interrupt: built-in ones use their INDEX, others the numbers after the
/// built-in ones in the given order. Fails if a number is used twice or is too high.
#[cfg(feature = "compile")]
pub fn interrupt_numbers<N: AsRef<str>>(names: &[N]) -> Result<Vec<u8>, String> {
    let mut custom = BUILTIN_SLOTS;
    let mut numbers: Vec<u8> = Vec::new();
    for name in names.iter().map(AsRef::as_ref) {
        let int_no = match builtin_index(name) {
            Some(index) => index,
            None => { custom += 1; custom - 1 }
        };
        if int_no >= MAX_SLOTS {
            return Err(format!("too many interrupts, {} would be number {} but the last is {}", name, int_no, MAX_SLOTS - 1));
        }
        if let Some(other) = numbers.iter().position(|n| *n == int_no) {
            return Err(format!("{} and {} are both interrupt number {}", names[other].as_ref(), name, int_no));
        }
        numbers.push(int_no);
    }
    Ok(numbers)
}

#[derive(Clone, Copy)]
pub struct SoftInterruptFunction <'a> {
    pub no: VMAtom,
    pub name: &'a str,    
    pub arguments: usize,
    pub returns: usize,
    pub help: &'a str
}

#[cfg(feature = "compile")]
pub struct SoftInterruptFunctionOwned {
    pub no: VMAtom,
    pub name: String,    
    pub arguments: usize,
    pub returns: usize,
    pub help: String
}

/// An interrupt for vms with programs in the storage S, Ram unless given. Interrupts that do not
/// read bins with get_bin or get_str can implement it for every storage with
/// impl<S: Storage> SoftInterrupt<S> for ..., copy_bin reads bins in any storage.
#[diagnostic::on_unimplemented(message = "`{Self}` is not a SoftInterrupt", label = "this has to implement SoftInterrupt to be used as an interrupt")]
pub trait SoftInterrupt<S: Storage = Ram> {        
    fn name(&self) -> &str;    

    #[cfg(feature = "compile")]
    fn functions(&self) -> &'static [SoftInterruptFunction<'static>];
    
    fn call(&mut self, vm: &mut VirtMach<'_, S>);

    /// Decodes a color atom to [r, g, b, a], see color::Color::to_rgba
    fn atom_to_rgba(atom: VMAtom) -> [u8; 4] where Self: Sized { crate::color::Color::to_rgba(atom) }
}

/// An interrupt with its number and functions, what interrupt_tables! builds from the same list
/// as interrupt_list!. Compile with VirtMach::compile_tables or document with its functions.
#[derive(Clone, Copy)]
pub struct InterruptTable<'a> {
    pub name: &'a str,
    pub number: u8,
    pub functions: &'a [SoftInterruptFunction<'a>]
}

/// The interrupt names and the functions by "interrupt.function" name, as the compilers take
/// them: (interrupt no, function no, arguments, returns). The bare interrupt name maps to
/// (interrupt no, 0, 0, 0). Fails if a number is used twice or too high.
#[cfg(feature = "compile")]
pub fn function_map(tables: &[InterruptTable]) -> Result<(Vec<String>, std::collections::HashMap<String, (u8, VMAtom, usize, usize)>), String> {
    let mut map = std::collections::HashMap::new();
    for (i, table) in tables.iter().enumerate() {
        if table.number >= MAX_SLOTS {
            return Err(format!("{} is interrupt number {} but the last is {}", table.name, table.number, MAX_SLOTS - 1));
        }
        if let Some(other) = tables[..i].iter().find(|t| t.number == table.number) {
            return Err(format!("{} and {} are both interrupt number {}", other.name, table.name, table.number));
        }
        map.insert(String::from(table.name), (table.number, 0, 0, 0));
        for function in table.functions {
            map.insert(format!("{}.{}", table.name, function.name), (table.number, function.no, function.arguments, function.returns));
        }
    }
    Ok((tables.iter().map(|t| String::from(t.name)).collect(), map))
}

/// Header of the csv that help writes, the columns of the interrupt .csv files
pub const HELP_HEADER: &str = "name,index,arguments,returns,description\n";

fn selected(filter: Option<&str>, name: &str, no: i32) -> bool {
    match filter {
        None => true,
        Some(f) => match f.trim().parse::<i32>() {
            Ok(n) => n == no,
            Err(_) => f.trim() == name
        }
    }
}

/// Writes the help of the interrupts as csv, with the columns of the interrupt .csv files under
/// one header: name, index, arguments, returns, description. A row with only the name and number
/// of an interrupt comes before its functions. The description is quoted, a quote in it doubled.
///
/// ```text
/// name,index,arguments,returns,description
/// math,0
/// and,0,2,1,"Logical AND (num0,num1)->(res)"
/// or,1,2,1,"Logical OR (num0,num1)->(res)"
/// time,3
/// wait_for,0,2,0,"Wait the interval from now (seconds,milliseconds)->()"
/// ```
///
/// Both filters are optional, None shows everything. A filter is a name or a number, text that
/// parses as a number is the number. With only an interrupt the help covers its functions, with
/// only a function it covers the functions of that name or number in every interrupt that has
/// one, with both the one function of the interrupt. An interrupt without functions is only its
/// row, unless a function is asked for.
///
/// Returns the number of functions written, plus one for each interrupt without functions, so 0
/// if nothing matched the filters and nothing was written, not even the header.
///
/// ```ignore
/// help(&tables, None, None, &mut out)?;                     // everything
/// help(&tables, Some("math"), None, &mut out)?;             // all functions of math
/// help(&tables, Some("0"), Some("sqr"), &mut out)?;         // math.sqr, 0 being math
/// help(&tables, Some("math"), Some("10"), &mut out)?;       // function number 10 of math
/// ```
pub fn help(tables: &[InterruptTable], interrupt: Option<&str>, function: Option<&str>, writer: &mut dyn core::fmt::Write) -> Result<usize, core::fmt::Error> {
    let mut rows = 0;
    for table in tables.iter().filter(|t| selected(interrupt, t.name, t.number as i32)) {
        let mut functions = table.functions.iter().filter(|f| selected(function, f.name, f.no as i32)).peekable();
        if functions.peek().is_none() && (function.is_some() || !table.functions.is_empty()) {
            continue;
        }
        if rows == 0 { writer.write_str(HELP_HEADER)?; }
        write!(writer, "{},{}\n", table.name, table.number)?;
        rows += functions.peek().is_none() as usize;
        for f in functions {
            write!(writer, "{},{},{},{},\"", f.name, f.no, f.arguments, f.returns)?;
            for c in f.help.chars() {
                if c == '"' { writer.write_char('"')?; }
                writer.write_char(c)?;
            }
            writer.write_str("\"\n")?;
            rows += 1;
        }
    }
    Ok(rows)
}

/// The length of the interrupt array for these numbers, the highest number plus one. Fails to
/// compile if a number is used twice or too high. Used by interrupt_list!.
#[doc(hidden)]
pub const fn slot_count<const K: usize>(slots: [u8; K]) -> usize {
    let (mut len, mut i) = (0, 0);
    while i < K {
        assert!(slots[i] < MAX_SLOTS, "interrupt number too high, the last is 15");
        let mut j = 0;
        while j < i {
            assert!(slots[j] != slots[i], "interrupt number used twice");
            j += 1;
        }
        if slots[i] as usize + 1 > len { len = slots[i] as usize + 1; }
        i += 1;
    }
    len
}

/// Puts a dummy in every slot of interrupt_list!
#[doc(hidden)]
#[inline(always)]
pub fn dummy_slots<'a, S: Storage, const N: usize>(slots: &mut [core::mem::MaybeUninit<&'a mut dyn SoftInterrupt<S>>; N]) {
    for slot in slots.iter_mut() {
        slot.write(dummy::Interrupt::slot());
    }
}

/// The slots of interrupt_list! once every one of them has an interrupt
#[doc(hidden)]
#[inline(always)]
pub fn slots_ready<'a, 'b, S: Storage, const N: usize>(slots: &'b mut [core::mem::MaybeUninit<&'a mut dyn SoftInterrupt<S>>; N]) -> &'b mut [&'a mut dyn SoftInterrupt<S>; N] {
    // SAFETY: dummy_slots wrote all of them and MaybeUninit<T> has the layout of T
    unsafe { &mut *(slots as *mut [core::mem::MaybeUninit<&'a mut dyn SoftInterrupt<S>>; N] as *mut [&'a mut dyn SoftInterrupt<S>; N]) }
}

/// Builds the array of interrupts to pass to VirtMach::run and binds it to a name. Each interrupt
/// goes to the slot of its number, slots without one are filled with dummy. The items are:
///
/// - `math`: the built-in interrupt of that module, at its INDEX
/// - `math @ 5`: the same at another number
/// - `time => my_time`: your variable with the implementation of a built-in interrupt, at its
///   INDEX (`time @ 5 => my_time` at another number). It is borrowed mutably, and it is a
///   compile error if it is not a SoftInterrupt.
/// - `time => &mut IntTime::new()`: the same with an expression of a reference
/// - `time => _`: the same with the reference given after the list, see below
/// - `ext("joystick", 11, &mut joystick)`: an interrupt that is not built in, with its name and
///   number, and optionally its functions for the documentation and the compiler:
///   `ext("joystick", 11, &mut joystick, &JOYSTICK_FUNCTIONS)`
///
/// ```ignore
/// interrupt_list!(let ints = [math, proc, time => sim_time, ext("joystick", 11, &mut joy)]);
/// vm.run(4096, ints);
///
/// // the references of the `_` items follow the list in their order
/// interrupt_list!(let ints = [math, time => _, periph => _]; &mut sim_time, &mut hw);
/// ```
///
/// Using a number twice or above 15 fails to compile. In debug builds the name() of each
/// implementation must match the module or the given name, or be dummy for an interrupt that is
/// not available.
///
/// interrupt_tables! takes the same list and builds the InterruptTable of each item instead, an
/// array to compile and document the program for the same numbers. It does not use the
/// implementations and variables, they do not have to exist where it is used. To write the list
/// once for both see interrupt_set!.
#[macro_export]
macro_rules! interrupt_list {
    (let $list:ident = [ $($items:tt)+ ]) => {
        $crate::interrupt_list!(@item {list $list} [] [] [] [] () $($items)+ ,)
    };
    (let $list:ident = [ $($items:tt)+ ] ; $($args:expr),* $(,)?) => {
        $crate::interrupt_list!(@item {list $list} [] [] [] [] ($($args,)*) $($items)+ ,)
    };
    // an external interrupt
    (@item $out:tt [$($s:expr,)*] [$($i:expr,)*] [$($n:expr,)*] [$($f:expr,)*] ($($q:expr,)*)
        ext($name:expr, $no:expr, $inst:expr, $funcs:expr $(,)?) , $($rest:tt)*) => {
        $crate::interrupt_list!(@item $out [$($s,)* $no,] [$($i,)* $inst,] [$($n,)* $name,] [$($f,)* $funcs,] ($($q,)*) $($rest)*)
    };
    (@item $out:tt [$($s:expr,)*] [$($i:expr,)*] [$($n:expr,)*] [$($f:expr,)*] ($($q:expr,)*)
        ext($name:expr, $no:expr, $inst:expr $(,)?) , $($rest:tt)*) => {
        $crate::interrupt_list!(@item $out [$($s,)* $no,] [$($i,)* $inst,] [$($n,)* $name,] [$($f,)* &[],] ($($q,)*) $($rest)*)
    };
    // a built-in interrupt with the next reference after the list, at another number
    (@item $out:tt [$($s:expr,)*] [$($i:expr,)*] [$($n:expr,)*] [$($f:expr,)*] ($next:expr, $($q:expr,)*)
        $m:ident @ $no:expr => _ , $($rest:tt)*) => {
        $crate::interrupt_list!(@item $out [$($s,)* $no,] [$($i,)* $next,] [$($n,)* $crate::interrupts::$m::NAME,]
            [$($f,)* $crate::interrupts::$m::FUNCTIONS.as_slice(),] ($($q,)*) $($rest)*)
    };
    // a built-in interrupt with the next reference after the list
    (@item $out:tt [$($s:expr,)*] [$($i:expr,)*] [$($n:expr,)*] [$($f:expr,)*] ($next:expr, $($q:expr,)*)
        $m:ident => _ , $($rest:tt)*) => {
        $crate::interrupt_list!(@item $out [$($s,)* $crate::interrupts::$m::INDEX,] [$($i,)* $next,] [$($n,)* $crate::interrupts::$m::NAME,]
            [$($f,)* $crate::interrupts::$m::FUNCTIONS.as_slice(),] ($($q,)*) $($rest)*)
    };
    // the same without one, the interrupt_tables! do not need them. In a list this is an error
    // as () is not a SoftInterrupt, because the list after it is too short.
    (@item $out:tt [$($s:expr,)*] [$($i:expr,)*] [$($n:expr,)*] [$($f:expr,)*] ()
        $m:ident @ $no:expr => _ , $($rest:tt)*) => {
        $crate::interrupt_list!(@item $out [$($s,)* $no,] [$($i,)* (),] [$($n,)* $crate::interrupts::$m::NAME,]
            [$($f,)* $crate::interrupts::$m::FUNCTIONS.as_slice(),] () $($rest)*)
    };
    (@item $out:tt [$($s:expr,)*] [$($i:expr,)*] [$($n:expr,)*] [$($f:expr,)*] ()
        $m:ident => _ , $($rest:tt)*) => {
        $crate::interrupt_list!(@item $out [$($s,)* $crate::interrupts::$m::INDEX,] [$($i,)* (),] [$($n,)* $crate::interrupts::$m::NAME,]
            [$($f,)* $crate::interrupts::$m::FUNCTIONS.as_slice(),] () $($rest)*)
    };
    // a built-in interrupt in a variable of the caller, at another number
    (@item $out:tt [$($s:expr,)*] [$($i:expr,)*] [$($n:expr,)*] [$($f:expr,)*] ($($q:expr,)*)
        $m:ident @ $no:expr => $var:ident , $($rest:tt)*) => {
        $crate::interrupt_list!(@item $out [$($s,)* $no,] [$($i,)* &mut $var,] [$($n,)* $crate::interrupts::$m::NAME,]
            [$($f,)* $crate::interrupts::$m::FUNCTIONS.as_slice(),] ($($q,)*) $($rest)*)
    };
    // a built-in interrupt in a variable of the caller
    (@item $out:tt [$($s:expr,)*] [$($i:expr,)*] [$($n:expr,)*] [$($f:expr,)*] ($($q:expr,)*)
        $m:ident => $var:ident , $($rest:tt)*) => {
        $crate::interrupt_list!(@item $out [$($s,)* $crate::interrupts::$m::INDEX,] [$($i,)* &mut $var,] [$($n,)* $crate::interrupts::$m::NAME,]
            [$($f,)* $crate::interrupts::$m::FUNCTIONS.as_slice(),] ($($q,)*) $($rest)*)
    };
    // a built-in interrupt with an implementation of the caller, at another number
    (@item $out:tt [$($s:expr,)*] [$($i:expr,)*] [$($n:expr,)*] [$($f:expr,)*] ($($q:expr,)*)
        $m:ident @ $no:expr => $inst:expr , $($rest:tt)*) => {
        $crate::interrupt_list!(@item $out [$($s,)* $no,] [$($i,)* $inst,] [$($n,)* $crate::interrupts::$m::NAME,]
            [$($f,)* $crate::interrupts::$m::FUNCTIONS.as_slice(),] ($($q,)*) $($rest)*)
    };
    // a built-in interrupt with an implementation of the caller
    (@item $out:tt [$($s:expr,)*] [$($i:expr,)*] [$($n:expr,)*] [$($f:expr,)*] ($($q:expr,)*)
        $m:ident => $inst:expr , $($rest:tt)*) => {
        $crate::interrupt_list!(@item $out [$($s,)* $crate::interrupts::$m::INDEX,] [$($i,)* $inst,] [$($n,)* $crate::interrupts::$m::NAME,]
            [$($f,)* $crate::interrupts::$m::FUNCTIONS.as_slice(),] ($($q,)*) $($rest)*)
    };
    // a built-in interrupt at another number
    (@item $out:tt [$($s:expr,)*] [$($i:expr,)*] [$($n:expr,)*] [$($f:expr,)*] ($($q:expr,)*)
        $m:ident @ $no:expr , $($rest:tt)*) => {
        $crate::interrupt_list!(@item $out [$($s,)* $no,] [$($i,)* &mut $crate::interrupts::$m::Interrupt {},] [$($n,)* $crate::interrupts::$m::NAME,]
            [$($f,)* $crate::interrupts::$m::FUNCTIONS.as_slice(),] ($($q,)*) $($rest)*)
    };
    // a built-in interrupt
    (@item $out:tt [$($s:expr,)*] [$($i:expr,)*] [$($n:expr,)*] [$($f:expr,)*] ($($q:expr,)*)
        $m:ident , $($rest:tt)*) => {
        $crate::interrupt_list!(@item $out [$($s,)* $crate::interrupts::$m::INDEX,] [$($i,)* &mut $crate::interrupts::$m::Interrupt {},] [$($n,)* $crate::interrupts::$m::NAME,]
            [$($f,)* $crate::interrupts::$m::FUNCTIONS.as_slice(),] ($($q,)*) $($rest)*)
    };
    // trailing commas
    (@item $out:tt [$($s:expr,)*] [$($i:expr,)*] [$($n:expr,)*] [$($f:expr,)*] ($($q:expr,)*) , $($rest:tt)*) => {
        $crate::interrupt_list!(@item $out [$($s,)*] [$($i,)*] [$($n,)*] [$($f,)*] ($($q,)*) $($rest)*)
    };
    // the end of the list, the array of interrupts, with all the references used
    (@item {list $list:ident} [$($s:expr,)*] [$($i:expr,)*] [$($n:expr,)*] [$($f:expr,)*] ()) => {
        // every slot is a dummy, then each interrupt is put in its own. The array is filled in
        // place, no arrays that are moved around: that is small enough for a microcontroller
        let mut __slotted = [ const { ::core::mem::MaybeUninit::uninit() }; $crate::interrupts::slot_count([ $($s),* ]) ];
        $crate::interrupts::dummy_slots(&mut __slotted);
        $(
            // the temporaries of the item live as long as the bindings
            let __int = $i as &mut dyn $crate::interrupts::SoftInterrupt<_>;
            // a dummy stands in for an interrupt that is not available
            debug_assert!(__int.name() == $n || __int.name() == $crate::interrupts::dummy::NAME);
            __slotted[$s as usize].write(__int);
        )*
        let $list = $crate::interrupts::slots_ready(&mut __slotted);
    };
    // the end of the list, the tables
    (@item {tables} [$($s:expr,)*] [$($i:expr,)*] [$($n:expr,)*] [$($f:expr,)*] ($($q:expr,)*)) => {{
        const _: usize = $crate::interrupts::slot_count([ $($s),* ]);
        [ $( $crate::interrupts::InterruptTable { name: $n, number: $s, functions: $f } ),* ]
    }};
}

/// The InterruptTable array for a list of interrupts, see interrupt_list! for the items. Compile
/// with VirtMach::compile_tables(name, listing, &tables).
#[macro_export]
macro_rules! interrupt_tables {
    ([ $($items:tt)+ ]) => {
        $crate::interrupt_list!(@item {tables} [] [] [] [] () $($items)+ ,)
    };
}

/// Writes a list of interrupts once for both interrupt_tables! and interrupt_list!. It defines a
/// macro of the given name, valid from there to the end of the block or module, so it can be at
/// the top of a file, before any variable exists. A macro cannot see variables defined after it,
/// so the items for variables are `_` and the references are given where the list is made:
///
/// ```ignore
/// interrupt_set!(my_interrupts = [math, proc, time => _, periph => _]);
///
/// fn main() {
///     let tables = my_interrupts!(tables);           // like interrupt_tables!([...])
///     // ...
///     loop {
///         my_interrupts!(let ints, &mut sim_time, &mut hw);   // like interrupt_list!(let ints = [...]; ...)
///         vm.run(4096, ints);
///     }
/// }
/// ```
#[macro_export]
macro_rules! interrupt_set {
    ($name:ident = [ $($items:tt)+ ]) => {
        $crate::interrupt_set!(@define ($) $name [ $($items)+ ]);
    };
    // the $ is passed in, a macro cannot write one for the macro it defines
    (@define ($d:tt) $name:ident [ $($items:tt)+ ]) => {
        macro_rules! $name {
            (tables) => { $crate::interrupt_tables!([ $($items)+ ]) };
            (let $d list:ident $d(, $d arg:expr)* $d(,)?) => { $crate::interrupt_list!(let $d list = [ $($items)+ ] ; $d($d arg),*); };
        }
    };
}

#[cfg(all(test, feature = "compile"))]
mod tests {
    use super::*;

    #[test]
    fn numbers() {
        assert_eq!(interrupt_numbers(&["proc", "math", "surface"]), Ok(std::vec![1, 0, 9]));
        assert_eq!(interrupt_numbers(&["math", "proc", "surface"]), Ok(std::vec![0, 1, 9]));
        // not built in: after the built-in numbers in the given order
        assert_eq!(interrupt_numbers(&["joystick", "math", "sound"]), Ok(std::vec![10, 0, 11]));
        assert!(interrupt_numbers(&["math", "math"]).is_err());
        assert!(interrupt_numbers(&["a", "b", "c", "d", "e", "f", "g"]).is_err());
        assert_eq!(builtin_index(dummy::NAME), None);
    }
}

#[cfg(test)]
mod list_tests {
    use crate::{ VirtMach, VMAtom, Storage, interrupts::{ self, SoftInterrupt, SoftInterruptFunction } };

    struct Joystick { calls: u32 }
    impl <S: Storage> SoftInterrupt<S> for Joystick {
        fn name(&self) -> &str { "joystick" }
        #[cfg(feature = "compile")]
        fn functions(&self) -> &'static [SoftInterruptFunction<'static>] { &[] }
        fn call(&mut self, vm: &mut VirtMach<'_, S>) { self.calls += 1; vm.stack_push(7 as VMAtom); }
    }

    #[test]
    fn list() {
        let mut joy = Joystick { calls: 0 };
        interrupt_list!(let ints = [proc, math @ 4, ext("joystick", 11, &mut joy), time => &mut interrupts::time::Interrupt {},]);
        let mut vm = VirtMach::new();
        vm.run(0, ints);
        assert_eq!(ints.len(), 12);
        assert_eq!(ints[0].name(), "dummy");
        assert_eq!(ints[1].name(), "proc");
        assert_eq!(ints[3].name(), "time");
        assert_eq!(ints[4].name(), "math");
        assert_eq!(ints[11].name(), "joystick");
    }
}

#[cfg(all(test, feature = "compile"))]
mod table_tests {
    use crate::{ VirtMach, interrupts::SoftInterruptFunction };
    use std::format;

    const JOY: [SoftInterruptFunction; 1] = [SoftInterruptFunction { no: 3, name: "read", arguments: 0, returns: 2, help: "Read the stick ()->(x,y)" }];

    #[test]
    fn tables() {
        let tables = interrupt_tables!([proc, math @ 4, time => &mut (), ext("joystick", 11, &mut (), &JOY), ext("sound", 12, &mut ())]);
        assert_eq!(tables.map(|t| (t.name, t.number)), [("proc", 1), ("math", 4), ("time", 3), ("joystick", 11), ("sound", 12)]);
        assert_eq!(tables[3].functions.len(), 1);
        assert_eq!(tables[4].functions.len(), 0);

        let (names, map) = super::function_map(&tables).unwrap();
        assert_eq!(names, ["proc", "math", "time", "joystick", "sound"]);
        assert_eq!(map["math.and"], (4, 0, 2, 1));
        helps(&tables);
        assert_eq!(map["joystick.read"], (11, 3, 0, 2));

        // a remapped interrupt compiles to its new number
        let (program, _) = VirtMach::compile_tables("t", "int math\n", &tables).unwrap();
        assert_eq!(program.get_instructions().first().map(|i| i >> 4), Some(4));
    }

    fn helps(tables: &[super::InterruptTable]) {
        use super::{ help, HELP_HEADER };
        let run = |i: Option<&str>, f: Option<&str>| {
            let mut out = std::string::String::new();
            let rows = help(tables, i, f, &mut out).unwrap();
            (rows, out)
        };
        // everything: one header, then the name and number of each interrupt above its functions
        let (_, all) = run(None, None);
        assert!(all.starts_with(HELP_HEADER));
        assert_eq!(all.matches(HELP_HEADER).count(), 1);
        assert!(all.starts_with(&format!("{}proc,1\nversion,0,0,3,", HELP_HEADER)));
        assert!(all.contains("\nmath,4\nand,0,2,1,\"Logical AND (num0,num1)->(res)\"\n"));
        assert!(all.contains("\njoystick,11\nread,3,0,2,\"Read the stick ()->(x,y)\"\nsound,12\n"));
        assert!(all.ends_with("sound,12\n") && !all.contains("\n\n"));
        // an interrupt by name or number, the same
        let (rows, math) = run(Some("math"), None);
        assert_eq!(run(Some("4"), None), (rows, math.clone()));
        assert!(rows > 10 && math.starts_with(&format!("{}math,4\nand,", HELP_HEADER)));
        // one function by name or number
        let (rows, one) = run(Some("math"), Some("sqr"));
        assert_eq!(rows, 1);
        assert_eq!(one, format!("{}math,4\nsqr,10,1,1,\"Squareroot (num)->(res)\"\n", HELP_HEADER));
        assert_eq!(run(Some("math"), Some("10")).1, one);
        // a function in every interrupt that has it, the others are left out
        let (rows, read) = run(None, Some("read"));
        assert_eq!((rows, read), (1, format!("{}joystick,11\nread,3,0,2,\"Read the stick ()->(x,y)\"\n", HELP_HEADER)));
        // nothing found, not even the header
        assert_eq!(run(Some("nothing"), None), (0, std::string::String::new()));
        assert_eq!(run(Some("sound"), Some("read")).0, 0);
        // quotes in the description are doubled
        let quoted = [super::InterruptTable { name: "q", number: 1, functions: &[super::SoftInterruptFunction { no: 0, name: "f", arguments: 0, returns: 0, help: "say \"hi\", twice" }] }];
        let mut out = std::string::String::new();
        help(&quoted, None, None, &mut out).unwrap();
        assert!(out.ends_with("f,0,0,0,\"say \"\"hi\"\", twice\"\n"));
    }
}

// the list once, at module level before the variables exist
#[cfg(test)]
interrupt_set!(module_set = [proc, time => _, periph @ 5 => _]);

#[cfg(test)]
mod set_tests {
    use crate::{ VirtMach, interrupts::{ self, SoftInterrupt } };

    #[test]
    fn variables_and_sets() {
        // the tables do not need the references
        let tables = module_set!(tables);
        assert_eq!(tables.map(|t| (t.name, t.number)), [("proc", 1), ("time", 3), ("periph", 5)]);

        let mut my_time = interrupts::time::Interrupt {};
        let mut my_periph = interrupts::periph::Interrupt {};
        let mut vm = VirtMach::new();
        module_set!(let ints, &mut my_time, &mut my_periph);
        vm.run(0, ints);
        assert_eq!((ints.len(), ints[1].name(), ints[3].name(), ints[5].name()), (6, "proc", "time", "periph"));

        // variables by name in the list, and the references after it
        interrupt_list!(let ints = [proc, time => my_time]);
        assert_eq!((ints.len(), ints[1].name(), ints[3].name()), (4, "proc", "time"));
        vm.run(0, ints);
        interrupt_list!(let ints = [math, time => _, periph => _]; &mut my_time, &mut my_periph);
        vm.run(0, ints);
        assert_eq!((ints.len(), ints[3].name(), ints[8].name()), (9, "time", "periph"));
    }
}
