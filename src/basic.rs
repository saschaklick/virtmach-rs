//! Compiles a small BASIC dialect into a virtmach listing, see VirtMach::compile for assembling it.
//!
//! Language summary:
//!   - optional line numbers and labels (NAME: at the start of a line), several statements per line
//!     separated by ":", comments: REM and ' to the end of the line, also behind statements,
//!     # for a whole line (a # behind a statement starts a color)
//!   - [LET] var = expr, arrays via DIM A(n) (elements 0..n, or 1..n after OPTION BASE 1), A(i) = expr, SWAP a, b
//!   - IF cond THEN stmts|line [ELSE stmts|line], block IF cond THEN / ELSEIF cond THEN / ELSE / END IF
//!   - SELECT CASE expr / CASE value, from TO to, IS op value, ... / CASE ELSE / END SELECT
//!   - FOR var = a TO b [STEP s] / NEXT [var], a STEP that is no constant is checked at run time
//!   - WHILE cond / WEND, DO [WHILE|UNTIL cond] / LOOP [WHILE|UNTIL cond]
//!   - EXIT FOR, EXIT WHILE, EXIT DO, EXIT SUB, EXIT FUNCTION
//!   - GOTO, GOSUB and ON n GOTO|GOSUB with line numbers or labels, RETURN, END,
//!     HALT (pauses the vm, e.g. once per frame), STOP (brk)
//!   - SUB name[(params)] / END SUB, FUNCTION name[(params)] / END FUNCTION with name = result,
//!     DEF FNname(params) = expr, calls as CALL name(args), name args or name(args) and in expressions.
//!     Variables inside are local unless declared SHARED and start at 0 on every call, parameters are
//!     passed by value. They are stored statically, so recursion is a compile error.
//!   - REQ interrupt[, ...] fails the compilation if an interrupt is not among the available ones
//!   - interrupt functions from the loaded .csv files, e.g. math.and(0x03, 0xf4) in expressions,
//!     surface.clear(0) as statement and W, H = surface.get_size() for multiple return values
//!   - colors as in CSS: #rgb, #rgba, #rrggbb, #rrggbbaa become a number, i8: RGB232, i16: RGB555, i32: RGB888,
//!     with the alpha reduced to 1 bit as the highest (sign) bit of the atom: alpha 1, also the default, is a
//!     negative value, alpha 0 a positive one. Decoding is up to the interrupts.
//!   - strings in double quotes ("" for a quote) become #db entries, their value is the entry's index,
//!     which is passed to interrupt functions, e.g. surface.draw_text(0, 0, 0, "Hello")
//!   - STR$(n) (with a blank in front of numbers that are not negative, like QBasic), MID$(s, start[, length]), LEFT$(s, n), RIGHT$(s, n) through string.format and string.substr,
//!     LEN(s) through string.get_length. Positions are 1-based. Every call site owns a dynamic string entry
//!     (after the #db entries) that is overwritten each time it runs, string arguments may be evaluated
//!     more than once. Variable names may end in $.
//!   - + joins strings through string.concat when an operand is a string: a literal, a name ending in $
//!     or another string +. Every + owns a dynamic string entry like the string functions.
//!   - operators: + - (native), * / MOD ^ << >> AND OR XOR NOT (math interrupt),
//!     = <> < > <= >= (-1 for true, 0 for false), strings compare by content through string.compare,
//!     < and > stay correct when the difference of the numbers overflows
//!   - built-in functions ABS(x), SGN(x), MIN(a, b, ...), MAX(a, b, ...), RND(low, high) (math.random)
//!   - in IF/WHILE/DO/CASE conditions AND, OR and NOT are logical and short-circuit
//!
//! Data types: only INTEGER and STRING exist. A variable is a STRING if its name ends in $, if it was declared
//! AS STRING (DIM, SUB/FUNCTION/DEF FN parameters, SHARED x AS STRING) or if DEFSTR covers its first letter, and an
//! INTEGER otherwise (also with %, X% and X are the same variable, DEFINT and AS INTEGER). Declarations are folded
//! into the name (X AS STRING is X$), they are scoped like the variables and count from where they appear.
//! Floating point literals (1.5, 1E5), the suffixes & ! #, AS LONG/SINGLE/DOUBLE/<type>, STRING * n,
//! DEFLNG/DEFSNG/DEFDBL, TYPE and CSNG/CDBL/CLNG fail with an error naming the type.
//! At most 255 string literals and string operations share the string indices, running out is a compile error.
//!
//! Declarations: CONST A = expr[, B$ = "text"] is folded when compiling (numbers, strings, earlier constants and all
//! operators), constants are visible in every SUB and FUNCTION and can size arrays: DIM A(N * 2). DIM SHARED makes
//! module level variables and arrays visible in every SUB and FUNCTION. DECLARE SUB|FUNCTION name(params) checks that
//! the definition exists and has as many parameters. OPTION BASE 0|1 must come before the first DIM.
//! DATA 1, -2, &HFF, "text", bare text / READ a, b$, c(i) / RESTORE [label|line]: all DATA is one list stored in an array,
//! reading past its end is a division by zero in the math interrupt. RANDOMIZE n is math.seed(n), SLEEP n is
//! time.wait_for(n, 0).
//! PEEK(address) / POKE address, value access the cells of the vm's memory, only the vm checks the address.
//! Comments: REM, ' and # for a whole line, a ; is no comment but separates the items of PRINT.
//! Drawing: CLS, COLOR (surface.set_color 1 and 0), PSET, PRESET, LINE [B|BF] and VIEW compile onto the surface interrupt, SCREEN is
//! dropped. Console: PRINT and ? are compile errors unless built with --cfg basic_console="surface", then they compile onto
//! the surface interrupt too, and LOCATE, WIDTH and VIEW PRINT are dropped. INPUT, LINE INPUT, INKEY$, INPUT$, LPRINT, BEEP, SOUND, PLAY, CIRCLE, PAINT, DRAW, GET, PUT, WINDOW, PALETTE
//! and POINT() are always errors.
//! Literals: 255, 0xFF, &HFF, &O377, &B11111111. Operators from the highest precedence: ^, unary -, * /, \ (integer
//! division, the same as /), MOD, + -, << >>, comparisons, NOT, AND, OR, XOR, EQV, IMP. EQV and IMP are bitwise like AND.
//!
//! Variables live in registers r0..r11, further variables and arrays in memory.
//! r12 and r13 are used for evaluating expressions.

extern crate alloc;
extern crate std;

use std::prelude::rust_2024::*;
use std::{ cell::RefCell, format, vec, collections::{ HashMap, HashSet } };
use pest::{ Parser as PestParser, error::LineColLocation, iterators::Pair, pratt_parser::{ Assoc, Op, PrattParser } };

use crate::{ VMAtom, interrupts::{ InterruptTable, function_map } };

/// Interrupt functions by "interrupt.function" name: (interrupt no, function no, arguments, returns)
pub type Functions = HashMap<String, (u8, VMAtom, usize, usize)>;

/// the array that holds the DATA items and the variable that points to the next one
const DATA_ARRAY: &str = "_DATA";
const DATA_POINTER: &str = "_DATAPTR";

/// The text console (PRINT, LOCATE, WIDTH, VIEW PRINT) is only compiled when the crate is built with `--cfg basic_console="surface"`,
/// the drawing statements (CLS, COLOR, PSET, PRESET, LINE, VIEW) are always there
const SURFACE_CONSOLE: bool = cfg!(basic_console = "surface");
/// State of the console: text cursor, colors, last graphics point and temporaries, visible in every SUB
const CONSOLE_VARS: [&str; 11] = ["_PX", "_PY", "_PCOL", "_PBG", "_GX", "_GY", "_LX", "_LY", "_PW", "_PH", "_PS$"];

/// The font of the surface is 3 pixels wide with 1 pixel between the characters and 4 pixels high with 1 pixel between
/// the lines, so PRINT moves on by 4 per character and down by 5 per line, and a zone of 14 characters is 56 pixels
const CHAR_WIDTH: VMAtom = 4;
const LINE_HEIGHT: VMAtom = 5;
const ZONE_WIDTH: VMAtom = 14 * CHAR_WIDTH;

const ACC: u8 = 13;
const TMP: u8 = 12;
const VAR_REGS: u8 = 12;

#[derive(Debug)]
pub struct Error {
    pub line: usize,
    pub msg: String
}

fn err<T>(line: usize, msg: impl Into<String>) -> Result<T, Error> {
    Err(Error { line, msg: msg.into() })
}

// ---------------------------------------------------------------------------------------------
// Syntax tree

#[derive(Debug, Clone, Copy, PartialEq)]
enum BinOp { Add, Sub, Mul, Div, Mod, Pow, Shl, Shr, And, Or, Xor, Eq, Ne, Lt, Gt, Le, Ge }

impl BinOp {
    fn symbol(&self) -> &'static str {
        match self {
            BinOp::Add => "+", BinOp::Sub => "-", BinOp::Mul => "*", BinOp::Div => "/", BinOp::Mod => "MOD",
            BinOp::Pow => "^", BinOp::Shl => "<<", BinOp::Shr => ">>", BinOp::And => "AND", BinOp::Or => "OR",
            BinOp::Xor => "XOR", BinOp::Eq => "=", BinOp::Ne => "<>", BinOp::Lt => "<", BinOp::Gt => ">",
            BinOp::Le => "<=", BinOp::Ge => ">="
        }
    }

    fn math_function(&self) -> Option<&'static str> {
        match self {
            BinOp::Mul => Some("mul"), BinOp::Div => Some("div"), BinOp::Mod => Some("mod"), BinOp::Pow => Some("pow"),
            BinOp::Shl => Some("lsh"), BinOp::Shr => Some("rsh"), BinOp::And => Some("and"), BinOp::Or => Some("or"),
            BinOp::Xor => Some("xor"),
            _ => None
        }
    }

    fn is_comparison(&self) -> bool {
        matches!(self, BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge)
    }
}

#[derive(Debug, Clone)]
enum Expr {
    Num(VMAtom),
    Str(String),
    Var(String),
    /// the value already in the accumulator, used internally by SWAP
    Acc,
    Call(String, Vec<Expr>),
    IntCall(String, Vec<Expr>),
    Neg(Box<Expr>),
    Not(Box<Expr>),
    Bin(BinOp, Box<Expr>, Box<Expr>)
}

#[derive(Debug, Clone)]
enum LValue {
    Var(String),
    Index(String, Expr)
}

#[derive(Debug)]
enum Branch {
    Line(u32),
    Stmts(Vec<Stmt>)
}

#[derive(Debug, Clone)]
enum Target {
    Line(u32),
    Label(String)
}

#[derive(Debug)]
enum CaseItem {
    Value(Expr),
    Range(Expr, Expr),
    Is(BinOp, Expr)
}

/// Items of PRINT
#[derive(Debug, Clone)]
enum PrintItem { Expr(Expr), Semicolon, Comma }

/// A graphics coordinate, relative to the last point after STEP
#[derive(Debug, Clone)]
struct Point { step: bool, x: Expr, y: Expr }

#[derive(Debug, Clone, Copy, PartialEq)]
enum LineKind { Line, Box, Fill }

#[derive(Debug, Clone, PartialEq)]
enum DataItem { Num(VMAtom), Str(String) }

#[derive(Debug, Clone, Copy, PartialEq)]
enum ExitKind { For, While, Do, Sub, Function }

#[derive(Debug)]
enum Stmt {
    Let(LValue, Expr),
    MultiAssign(Vec<LValue>, String, Vec<Expr>),
    Call(String, Vec<Expr>),
    /// arrays (name, cells) and the names DIM SHARED made visible in every SUB and FUNCTION
    Dim(Vec<(String, usize)>, Vec<String>),
    /// OPTION BASE, the first index of arrays
    OptionBase(usize),
    /// console and drawing statements on the surface interrupt, only built with --cfg basic_console="surface"
    Print(Vec<PrintItem>),
    Cls,
    Color(Option<Expr>, Option<Expr>),
    Pset { reset: bool, point: Point, color: Option<Expr> },
    Line { from: Option<Point>, to: Point, color: Option<Expr>, kind: LineKind },
    View(Option<(Point, Point)>),
    /// DATA items, they form one list for the whole program in the order of the source
    Data(Vec<DataItem>),
    Read(Vec<LValue>),
    /// POKE address, value
    Poke(Expr, Expr),
    /// RESTORE with the line or label whose DATA is read next
    Restore(Option<Target>),
    /// DECLARE name, number of parameters, is a FUNCTION
    Declare(String, usize, bool),
    If(Expr, Branch, Option<Branch>),
    BlockIf(Expr),
    ElseIf(Expr),
    Else,
    EndIf,
    Select(Expr),
    /// None for CASE ELSE
    Case(Option<Vec<CaseItem>>),
    EndSelect,
    For(String, Expr, Expr, Option<Expr>),
    Next(Option<String>),
    While(Expr),
    Wend,
    /// the condition with true for UNTIL, false for WHILE
    Do(Option<(bool, Expr)>),
    Loop(Option<(bool, Expr)>),
    Exit(ExitKind),
    Goto(Target),
    Gosub(Target),
    On(Expr, bool, Vec<Target>),
    Return,
    End,
    Halt,
    Stop,
    Swap(LValue, LValue),
    /// name, parameters, is a FUNCTION
    SubDef(String, Vec<String>, bool),
    EndSub(bool),
    Shared(Vec<String>),
    Req(Vec<String>),
    DefFn(String, Vec<String>, Expr),
    CallSub(String, Vec<Expr>)
}

#[derive(Debug)]
struct Line {
    src_line: usize,
    number: Option<u32>,
    label: Option<String>,
    text: String,
    stmts: Vec<Stmt>
}

// ---------------------------------------------------------------------------------------------
// Parser

#[derive(pest_derive::Parser)]
#[grammar = "basic.pest"]
struct BasicParser;

fn pratt() -> PrattParser<Rule> {
    PrattParser::new()
        .op(Op::infix(Rule::op_imp, Assoc::Left))
        .op(Op::infix(Rule::op_eqv, Assoc::Left))
        .op(Op::infix(Rule::op_xor, Assoc::Left))
        .op(Op::infix(Rule::op_or, Assoc::Left))
        .op(Op::infix(Rule::op_and, Assoc::Left))
        .op(Op::prefix(Rule::op_not))
        .op(Op::infix(Rule::op_eq, Assoc::Left) | Op::infix(Rule::op_ne, Assoc::Left)
          | Op::infix(Rule::op_lt, Assoc::Left) | Op::infix(Rule::op_gt, Assoc::Left)
          | Op::infix(Rule::op_le, Assoc::Left) | Op::infix(Rule::op_ge, Assoc::Left))
        .op(Op::infix(Rule::op_shl, Assoc::Left) | Op::infix(Rule::op_shr, Assoc::Left))
        .op(Op::infix(Rule::op_add, Assoc::Left) | Op::infix(Rule::op_sub, Assoc::Left))
        .op(Op::infix(Rule::op_mod, Assoc::Left))
        .op(Op::infix(Rule::op_idiv, Assoc::Left))
        .op(Op::infix(Rule::op_mul, Assoc::Left) | Op::infix(Rule::op_div, Assoc::Left))
        .op(Op::prefix(Rule::op_neg))
        .op(Op::infix(Rule::op_pow, Assoc::Right))
}

fn is_keyword(rule: Rule) -> bool {
    matches!(rule, Rule::kw_if | Rule::kw_then | Rule::kw_else | Rule::kw_endif | Rule::kw_end | Rule::kw_for
        | Rule::kw_to | Rule::kw_step | Rule::kw_next | Rule::kw_while | Rule::kw_wend | Rule::kw_goto
        | Rule::kw_gosub | Rule::kw_return | Rule::kw_halt | Rule::kw_dim | Rule::kw_let | Rule::kw_elseif
        | Rule::kw_select | Rule::kw_case | Rule::kw_is | Rule::kw_do | Rule::kw_loop | Rule::kw_until | Rule::kw_on
        | Rule::kw_exit | Rule::kw_stop | Rule::kw_swap | Rule::kw_sub | Rule::kw_function | Rule::kw_call
        | Rule::kw_shared | Rule::kw_def | Rule::kw_req | Rule::kw_as | Rule::kw_type | Rule::kw_const | Rule::kw_declare | Rule::kw_option
        | Rule::kw_data | Rule::kw_read | Rule::kw_restore | Rule::kw_randomize | Rule::kw_sleep
        | Rule::kw_poke | Rule::kw_print | Rule::kw_using | Rule::kw_cls | Rule::kw_locate | Rule::kw_color | Rule::kw_screen | Rule::kw_width
        | Rule::kw_view | Rule::kw_pset | Rule::kw_preset | Rule::kw_line | Rule::kw_input)
}

fn parse_number(text: &str, line: usize) -> Result<VMAtom, Error> {
    let bits = size_of::<VMAtom>() * 8;
    let lower = text.to_lowercase();
    let (radix, digits, kind) = if let Some(d) = lower.strip_prefix("0x").or_else(|| lower.strip_prefix("&h")) { (16, Some(d), "hex") }
        else if let Some(d) = lower.strip_prefix("&o") { (8, Some(d), "octal") }
        else if let Some(d) = lower.strip_prefix("&b") { (2, Some(d), "binary") }
        else { (10, None, "") };
    if let Some(digits) = digits {
        match u64::from_str_radix(digits, radix) {
            Ok(v) if v < (1u64 << bits) => Ok(v as VMAtom),
            _ => err(line, format!("{} number {} does not fit into {} bits", kind, text, bits))
        }
    } else {
        match text.parse::<i64>() {
            Ok(v) if v <= VMAtom::MAX as i64 => Ok(v as VMAtom),
            _ => err(line, format!("number {} is out of range (max {})", text, VMAtom::MAX))
        }
    }
}

fn parse_line_ref(pair: Pair<Rule>, line: usize) -> Result<u32, Error> {
    pair.as_str().parse::<u32>().or_else(|_| err(line, format!("line number {} is out of range", pair.as_str())))
}

/// Declared variable types while parsing: names are stored without their suffix, true for STRING.
/// Declarations (DIM x AS STRING, parameters, SHARED, DEFSTR) are folded into the name by adding the $ suffix, which
/// is all the code generator looks at. The first scope is the module, every SUB, FUNCTION and DEF FN adds one.
struct Types {
    scopes: Vec<HashMap<String, bool>>,
    /// CONST values, one map per scope like the declarations: module constants are visible everywhere
    consts: Vec<HashMap<String, Const>>,
    /// DEFSTR for the letters A..Z
    def_string: [bool; 26],
    /// OPTION BASE must come before the first DIM
    dim_seen: bool
}

impl Types {
    fn push_scope(&mut self, inherit: bool) {
        let (scope, consts) = if inherit { (self.scopes.last().unwrap().clone(), self.consts.last().unwrap().clone()) } else { Default::default() };
        self.scopes.push(scope);
        self.consts.push(consts);
    }

    fn pop_scope(&mut self) {
        if self.scopes.len() > 1 { self.scopes.pop(); self.consts.pop(); }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Const { Num(VMAtom), Str(String), Color(VMAtom) }

/// Evaluates a constant expression, for CONST and the size of arrays
fn fold(expr: &Expr) -> Result<Const, String> {
    fn num(expr: &Expr) -> Result<VMAtom, String> {
        match fold(expr)? { Const::Num(n) | Const::Color(n) => Ok(n), Const::Str(_) => Err(String::from("a string cannot be used here")) }
    }
    fn fit(v: Option<i64>) -> Result<VMAtom, String> {
        v.and_then(|v| VMAtom::try_from(v).ok()).ok_or_else(|| format!("the result does not fit into {} bits", size_of::<VMAtom>() * 8))
    }
    match expr {
        Expr::Num(n) => Ok(Const::Num(*n)),
        Expr::Str(s) => Ok(Const::Str(s.clone())),
        Expr::Neg(inner) => fit(Some(-(num(inner)? as i64))).map(Const::Num),
        Expr::Not(inner) => Ok(Const::Num(!num(inner)?)),
        Expr::Bin(BinOp::Add, lhs, rhs) if matches!((fold(lhs)?, fold(rhs)?), (Const::Str(_), Const::Str(_))) => {
            match (fold(lhs)?, fold(rhs)?) { (Const::Str(a), Const::Str(b)) => Ok(Const::Str(a + &b)), _ => unreachable!() }
        }
        Expr::Bin(op, lhs, rhs) => {
            let (a, b) = (num(lhs)? as i64, num(rhs)? as i64);
            let bits = size_of::<VMAtom>() as i64 * 8;
            let truth = |t: bool| Some(if t { -1 } else { 0 });
            fit(match op {
                BinOp::Add => a.checked_add(b), BinOp::Sub => a.checked_sub(b), BinOp::Mul => a.checked_mul(b),
                BinOp::Div | BinOp::Mod if b == 0 => return Err(String::from("division by zero")),
                BinOp::Div => a.checked_div(b), BinOp::Mod => a.checked_rem(b),
                BinOp::Pow if b < 0 => return Err(String::from("negative exponent")),
                BinOp::Pow => a.checked_pow(b as u32),
                BinOp::Shl | BinOp::Shr if !(0..bits).contains(&b) => return Err(format!("cannot shift by {}", b)),
                BinOp::Shl => Some(((a as VMAtom) << b) as i64), BinOp::Shr => Some(((a as VMAtom) >> b) as i64),
                BinOp::And => Some(a & b), BinOp::Or => Some(a | b), BinOp::Xor => Some(a ^ b),
                BinOp::Eq => truth(a == b), BinOp::Ne => truth(a != b), BinOp::Lt => truth(a < b),
                BinOp::Gt => truth(a > b), BinOp::Le => truth(a <= b), BinOp::Ge => truth(a >= b)
            }).map(Const::Num)
        }
        _ => Err(String::from("it is not a constant expression, only numbers, strings, other constants and operators are allowed"))
    }
}

struct Builder {
    pratt: PrattParser<Rule>,
    types: RefCell<Types>
}

impl Builder {
    fn stmt(&self, pair: Pair<Rule>, line: usize) -> Result<Stmt, Error> {
        let rule = pair.as_rule();
        let has = |kw: Rule| pair.clone().into_inner().any(|p| p.as_rule() == kw);
        let (has_shared, has_function) = (has(Rule::kw_shared), has(Rule::kw_function));
        let text = pair.as_str();
        let head_is_preset = text.trim_start().to_uppercase().starts_with("PRESET");
        let mut it = pair.into_inner().filter(|p| !is_keyword(p.as_rule()));
        Ok(match rule {
            Rule::let_stmt => {
                let target = self.lvalue(it.next().unwrap(), line)?;
                Stmt::Let(target, self.expr(it.next().unwrap(), line)?)
            }
            Rule::multi_assign => {
                let mut parts: Vec<Pair<Rule>> = it.collect();
                let (name, args) = self.int_call(parts.pop().unwrap(), line)?;
                let targets = parts.into_iter().map(|p| self.lvalue(p, line)).collect::<Result<Vec<_>, _>>()?;
                Stmt::MultiAssign(targets, name, args)
            }
            Rule::call_stmt => {
                let (name, args) = self.int_call(it.next().unwrap(), line)?;
                Stmt::Call(name, args)
            }
            // a DIM without a size declares a plain variable, which only needs its type recorded
            Rule::dim_stmt => {
                let (mut arrays, mut shared) = (vec![], vec![]);
                let is_shared = has_shared;
                if is_shared && self.types.borrow().scopes.len() > 1 { return err(line, "DIM SHARED is only allowed outside of SUB and FUNCTION"); }
                self.types.borrow_mut().dim_seen = true;
                for item in it {
                    let mut parts = item.into_inner();
                    let ident = parts.next().unwrap();
                    let (mut size, mut as_clause) = (None, None);
                    for part in parts {
                        if part.as_rule() == Rule::expr { size = Some(part) } else { as_clause = Some(part) }
                    }
                    let name = self.declare(ident.as_str(), as_clause.as_ref(), line)?;
                    if is_shared { shared.push(name.clone()); }
                    if let Some(size) = size {
                        let size = match fold(&self.expr(size, line)?) {
                            Ok(Const::Num(n) | Const::Color(n)) => n,
                            Ok(Const::Str(_)) => return err(line, format!("the size of array {} must be a number", name)),
                            Err(msg) => return err(line, format!("the size of array {} must be constant: {}", name, msg))
                        };
                        if size < 0 { return err(line, format!("illegal size for array {}", name)); }
                        arrays.push((name, size as usize + 1));
                    }
                }
                Stmt::Dim(arrays, shared)
            }
            Rule::input_stmt | Rule::line_input_stmt => return err(line, "INPUT is not supported, do not use input features"),
            Rule::unsupported_stmt => {
                let word = it.next().unwrap().as_str().to_uppercase();
                return err(line, match word.as_str() {
                    "CIRCLE" | "PAINT" | "DRAW" => format!("{} is not supported, the surface cannot draw it", word),
                    "GET" | "PUT" => format!("{} is not supported, there are no files or buffers", word),
                    "WINDOW" | "PALETTE" => format!("{} is not supported", word),
                    "LPRINT" => String::from("LPRINT is not supported, there is no printer"),
                    _ => format!("{} is not supported, there is no sound", word)
                });
            }
            Rule::print_stmt | Rule::locate_stmt | Rule::width_stmt | Rule::view_print_stmt if !SURFACE_CONSOLE => {
                let head: String = text.trim_start().chars().take_while(|c| c.is_ascii_alphabetic() || *c == '?').collect::<String>().to_uppercase();
                let head = if head == "?" { String::from("PRINT") } else if rule == Rule::view_print_stmt { String::from("VIEW PRINT") } else { head };
                return err(line, format!("{} is not supported, do not use console statements", head));
            }
            // the surface has no text window, display modes or line width, SCREEN needs no mode for the drawing statements
            Rule::locate_stmt | Rule::screen_stmt | Rule::width_stmt | Rule::view_print_stmt => Stmt::Dim(vec![], vec![]),
            Rule::print_stmt => {
                let mut items = vec![];
                let separator = |p: Pair<Rule>| if p.as_str() == ";" { PrintItem::Semicolon } else { PrintItem::Comma };
                for body in it {
                    for part in body.into_inner() {
                        match part.as_rule() {
                            Rule::using_clause => return err(line, "PRINT USING is not supported"),
                            _ => for item in part.into_inner() {
                                let mut parts = item.into_inner();
                                let first = parts.next().unwrap();
                                if first.as_rule() == Rule::print_sep { items.push(separator(first)); } else {
                                    items.push(PrintItem::Expr(self.expr(first, line)?));
                                    if let Some(sep) = parts.next() { items.push(separator(sep)); }
                                }
                            }
                        }
                    }
                }
                Stmt::Print(items)
            }
            Rule::cls_stmt => Stmt::Cls,
            Rule::color_stmt => {
                let args = it.map(|p| p.into_inner().next().map(|e| self.color_arg(e, line)).transpose()).collect::<Result<Vec<_>, _>>()?;
                if args.len() > 2 { return err(line, "COLOR takes a foreground and a background color"); }
                Stmt::Color(args.first().cloned().flatten(), args.get(1).cloned().flatten())
            }
            Rule::pset_stmt => {
                let point = self.coord(it.next().unwrap(), line)?;
                Stmt::Pset { reset: head_is_preset, point, color: it.next().map(|c| self.color_arg(c, line)).transpose()? }
            }
            Rule::line_stmt => {
                let mut points = vec![];
                let (mut color, mut kind) = (None, LineKind::Line);
                for part in it {
                    match part.as_rule() {
                        Rule::coord => points.push(self.coord(part, line)?),
                        Rule::opt_arg => color = part.into_inner().next().map(|e| self.color_arg(e, line)).transpose()?,
                        Rule::box_kind => kind = if part.as_str().eq_ignore_ascii_case("BF") { LineKind::Fill } else { LineKind::Box },
                        _ => return err(line, "line styles are not supported")
                    }
                }
                let to = points.pop().unwrap();
                Stmt::Line { from: points.pop(), to, color, kind }
            }
            Rule::view_stmt => {
                let mut points = vec![];
                for part in it {
                    match part.as_rule() {
                        Rule::coord => points.push(self.coord(part, line)?),
                        _ => if part.into_inner().next().is_some() { return err(line, "VIEW colors and borders are not supported"); }
                    }
                }
                if points.iter().any(|p| p.step) { return err(line, "VIEW takes absolute coordinates"); }
                let to = points.pop();
                Stmt::View(to.map(|to| (points.pop().unwrap(), to)))
            }
            Rule::data_stmt => {
                let mut items = vec![];
                for part in it {
                    items.push(match part.as_rule() {
                        Rule::string => { let t = part.as_str(); DataItem::Str(t[1..t.len() - 1].replace("\"\"", "\"")) }
                        _ => Self::data_item(part.as_str().trim(), line)?
                    });
                }
                Stmt::Data(items)
            }
            Rule::poke_stmt => { let address = self.expr(it.next().unwrap(), line)?; Stmt::Poke(address, self.expr(it.next().unwrap(), line)?) }
            Rule::read_stmt => Stmt::Read(it.map(|p| self.lvalue(p, line)).collect::<Result<Vec<_>, _>>()?),
            Rule::restore_stmt => Stmt::Restore(it.next().map(|p| self.target(p, line)).transpose()?),
            Rule::randomize_stmt => match it.next() {
                Some(seed) => Stmt::Call(String::from("math.seed"), vec![self.expr(seed, line)?]),
                None => return err(line, "RANDOMIZE needs a seed, there is no TIMER to take one from")
            },
            Rule::sleep_stmt => match it.next() {
                Some(seconds) => Stmt::Call(String::from("time.wait_for"), vec![self.expr(seconds, line)?, Expr::Num(0)]),
                None => return err(line, "SLEEP needs the number of seconds, waiting for a key is not supported")
            },
            Rule::const_stmt => {
                for item in it {
                    let mut parts = item.into_inner();
                    let name = Self::const_key(parts.next().unwrap().as_str());
                    let expr = parts.next().unwrap();
                    let is_color = Self::single_primary(&expr).is_some_and(|p| p.as_rule() == Rule::color);
                    let value = self.expr(expr, line)?;
                    if self.types.borrow().consts.last().unwrap().contains_key(&name) { return err(line, format!("constant {} is defined twice", name)); }
                    let value = fold(&value).or_else(|msg| err(line, format!("CONST {}: {}", name, msg)))?;
                    // a constant that is a color literal stays one, so that it can be used as the color of a drawing statement
                    let value = match value { Const::Num(n) if is_color => Const::Color(n), other => other };
                    if name.ends_with('$') && matches!(value, Const::Num(_) | Const::Color(_)) { return err(line, format!("CONST {} ends in $ but its value is a number", name)); }
                    self.types.borrow_mut().consts.last_mut().unwrap().insert(name, value);
                }
                Stmt::Dim(vec![], vec![])
            }
            Rule::declare_stmt => {
                let name = it.next().unwrap().as_str();
                let name = if has_function { self.var_name(name) } else { name.to_uppercase() };
                Stmt::Declare(name, it.next().map_or(0, |p| p.into_inner().count()), has_function)
            }
            Rule::option_stmt => {
                let base = parse_number(it.nth(1).unwrap().as_str(), line)?;
                if !(0..=1).contains(&base) { return err(line, "OPTION BASE must be 0 or 1"); }
                if self.types.borrow().dim_seen { return err(line, "OPTION BASE must come before the first DIM"); }
                Stmt::OptionBase(base as usize)
            }
            // DEFINT and DEFSTR set the default type of the names starting with these letters
            Rule::deftype_stmt => {
                let mut parts: Vec<Pair<Rule>> = it.collect();
                let string = parts.remove(0).as_str().to_uppercase() == "DEFSTR";
                for range in parts {
                    let text = range.as_str().to_uppercase();
                    let (first, last) = (text.as_bytes()[0], *text.as_bytes().last().unwrap());
                    if first > last { return err(line, format!("the letter range {} is backwards", text)); }
                    for letter in first..=last { self.types.borrow_mut().def_string[(letter - b'A') as usize] = string; }
                }
                Stmt::Dim(vec![], vec![])
            }
            Rule::type_stmt | Rule::end_type => return err(line, "user-defined types (TYPE ... END TYPE) are not supported, only INTEGER and STRING exist"),
            Rule::if_stmt => {
                let cond = self.expr(it.next().unwrap(), line)?;
                let then = self.branch(it.next().unwrap(), line)?;
                let otherwise = it.next().map(|p| self.branch(p, line)).transpose()?;
                Stmt::If(cond, then, otherwise)
            }
            Rule::block_if => Stmt::BlockIf(self.expr(it.next().unwrap(), line)?),
            Rule::elseif_stmt => Stmt::ElseIf(self.expr(it.next().unwrap(), line)?),
            Rule::else_stmt => Stmt::Else,
            Rule::select_stmt => Stmt::Select(self.expr(it.next().unwrap(), line)?),
            Rule::case_stmt => {
                let parts: Vec<Pair<Rule>> = it.collect();
                if parts[0].as_rule() == Rule::case_else { Stmt::Case(None) } else {
                    Stmt::Case(Some(parts.into_iter().map(|p| self.case_item(p, line)).collect::<Result<Vec<_>, _>>()?))
                }
            }
            Rule::end_select => Stmt::EndSelect,
            Rule::do_stmt => Stmt::Do(it.next().map(|p| self.do_cond(p, line)).transpose()?),
            Rule::loop_stmt => Stmt::Loop(it.next().map(|p| self.do_cond(p, line)).transpose()?),
            Rule::exit_stmt => Stmt::Exit(match it.next().unwrap().as_str().to_uppercase().as_str() {
                "FOR" => ExitKind::For, "WHILE" => ExitKind::While, "DO" => ExitKind::Do, "SUB" => ExitKind::Sub, _ => ExitKind::Function
            }),
            Rule::on_stmt => {
                let selector = self.expr(it.next().unwrap(), line)?;
                let gosub = it.next().unwrap().as_str().to_uppercase().starts_with("GOSUB");
                Stmt::On(selector, gosub, it.map(|p| self.target(p, line)).collect::<Result<Vec<_>, _>>()?)
            }
            Rule::stop_stmt => Stmt::Stop,
            Rule::swap_stmt => {
                let a = self.lvalue(it.next().unwrap(), line)?;
                Stmt::Swap(a, self.lvalue(it.next().unwrap(), line)?)
            }
            Rule::sub_def | Rule::function_def => {
                let is_function = rule == Rule::function_def;
                let raw = it.next().unwrap();
                // a FUNCTION returns a string if its name says so, the result is assigned to the name inside
                let mut name = raw.as_str().to_uppercase();
                if is_function {
                    name = self.var_name(&name);
                    if name.ends_with('$') { self.types.borrow_mut().scopes[0].insert(Self::base(&name), true); }
                }
                self.types.borrow_mut().push_scope(false);
                if is_function && name.ends_with('$') { self.types.borrow_mut().scopes.last_mut().unwrap().insert(Self::base(&name), true); }
                let params = it.next().map(|p| self.params(p, line)).transpose()?.unwrap_or_default();
                Stmt::SubDef(name, params, is_function)
            }
            Rule::end_sub | Rule::end_function => {
                self.types.borrow_mut().pop_scope();
                Stmt::EndSub(rule == Rule::end_function)
            }
            // SHARED takes over the type the module declared, or the one given with AS
            Rule::shared_stmt => Stmt::Shared(it.map(|item| {
                let mut parts = item.into_inner();
                let ident = parts.next().unwrap();
                let as_clause = parts.next();
                let base = Self::base(ident.as_str());
                let inherited = self.types.borrow().scopes[0].get(&base).copied();
                match (as_clause.as_ref(), inherited) {
                    (None, Some(string)) if !ident.as_str().ends_with(['$', '%']) => {
                        self.types.borrow_mut().scopes.last_mut().unwrap().insert(base.clone(), string);
                        Ok(if string { format!("{}$", base) } else { base })
                    }
                    _ => self.declare(ident.as_str(), as_clause.as_ref(), line)
                }
            }).collect::<Result<Vec<_>, _>>()?),
            Rule::req_stmt => Stmt::Req(it.map(|p| String::from(p.as_str())).collect()),
            Rule::def_fn => {
                let name = it.next().unwrap().as_str().to_uppercase();
                let mut parts: Vec<Pair<Rule>> = it.collect();
                let body = parts.pop().unwrap();
                // the parameters are only known inside the expression
                self.types.borrow_mut().push_scope(true);
                let result = (|| {
                    let params = parts.pop().map(|p| self.params(p, line)).transpose()?.unwrap_or_default();
                    Ok(Stmt::DefFn(name, params, self.expr(body, line)?))
                })();
                self.types.borrow_mut().pop_scope();
                result?
            }
            Rule::call_sub | Rule::sub_call => {
                let name = it.next().unwrap().as_str().to_uppercase();
                Stmt::CallSub(name, self.args(it.next(), line)?)
            }
            Rule::end_if => Stmt::EndIf,
            Rule::for_stmt => {
                let var = self.var_name(it.next().unwrap().as_str());
                let start = self.expr(it.next().unwrap(), line)?;
                let limit = self.expr(it.next().unwrap(), line)?;
                let step = it.next().map(|p| self.expr(p, line)).transpose()?;
                Stmt::For(var, start, limit, step)
            }
            Rule::next_stmt => Stmt::Next(it.next().map(|p| self.var_name(p.as_str()))),
            Rule::while_stmt => Stmt::While(self.expr(it.next().unwrap(), line)?),
            Rule::wend_stmt => Stmt::Wend,
            Rule::goto_stmt => Stmt::Goto(self.target(it.next().unwrap(), line)?),
            Rule::gosub_stmt => Stmt::Gosub(self.target(it.next().unwrap(), line)?),
            Rule::return_stmt => Stmt::Return,
            Rule::end_stmt => Stmt::End,
            Rule::halt_stmt => Stmt::Halt,
            r => unreachable!("unexpected statement rule {:?}", r)
        })
    }

    fn target(&self, pair: Pair<Rule>, line: usize) -> Result<Target, Error> {
        let inner = pair.into_inner().next().unwrap();
        match inner.as_rule() {
            Rule::line_ref => Ok(Target::Line(parse_line_ref(inner, line)?)),
            _ => Ok(Target::Label(inner.as_str().to_uppercase()))
        }
    }

    fn coord(&self, pair: Pair<Rule>, line: usize) -> Result<Point, Error> {
        let step = pair.clone().into_inner().any(|p| p.as_rule() == Rule::kw_step);
        let mut exprs = pair.into_inner().filter(|p| p.as_rule() == Rule::expr);
        Ok(Point { step, x: self.expr(exprs.next().unwrap(), line)?, y: self.expr(exprs.next().unwrap(), line)? })
    }

    /// The single primary of an expression, None if it has operators
    fn single_primary<'a>(expr: &Pair<'a, Rule>) -> Option<Pair<'a, Rule>> {
        let mut parts = expr.clone().into_inner();
        let first = parts.next()?;
        parts.next().is_none().then_some(first)
    }

    /// The color argument of a drawing statement: a color literal or a constant that was defined as one stays as it is,
    /// a constant number is an index of the default VGA palette, which is looked up now. Anything else is an error,
    /// as an index cannot be looked up at run time.
    fn color_arg(&self, pair: Pair<Rule>, line: usize) -> Result<Expr, Error> {
        if let Some(primary) = Self::single_primary(&pair) {
            match primary.as_rule() {
                Rule::color => return Ok(Expr::Num(crate::color::Color::parse(primary.as_str()))),
                Rule::ident => if let Some(Const::Color(color)) = self.constant(primary.as_str()) { return Ok(Expr::Num(color)); },
                _ => {}
            }
        }
        match fold(&self.expr(pair, line)?) {
            Ok(Const::Num(index)) if (0..=255).contains(&(index as i64)) => Ok(Expr::Num(crate::color::Color::vga(index as u8))),
            Ok(Const::Num(index)) => err(line, format!("palette index {} is out of range, 0..255", index)),
            _ => err(line, "colors must be a color literal or a constant palette index")
        }
    }

    /// A DATA item that is not in quotes: a number, or else a text
    fn data_item(text: &str, line: usize) -> Result<DataItem, Error> {
        if text.is_empty() { return err(line, "empty DATA items are not supported"); }
        let (negative, digits) = match text.strip_prefix('-') { Some(d) => (true, d), None => (false, text.strip_prefix('+').unwrap_or(text)) };
        let lower = digits.to_lowercase();
        let plain = !digits.is_empty() && digits.bytes().all(|c| c.is_ascii_digit());
        let is_number = plain || (["0x", "&h", "&o", "&b"].iter().any(|p| lower.starts_with(p)) && digits.len() > 2);
        if is_number {
            let value = if plain {
                digits.parse::<i64>().ok().map(|v| if negative { -v } else { v })
            } else {
                parse_number(digits, line).ok().map(|v| if negative { -(v as i64) } else { v as i64 })
            };
            return match value.and_then(|v| VMAtom::try_from(v).ok()) {
                Some(v) => Ok(DataItem::Num(v)),
                None => err(line, format!("the DATA number {} does not fit into {} bits", text, size_of::<VMAtom>() * 8))
            };
        }
        if (digits.starts_with(|c: char| c.is_ascii_digit() || c == '.')) && digits.to_lowercase().replace('d', "e").parse::<f64>().is_ok() {
            return err(line, format!("the DATA item {} is a floating point number, which is not supported, only INTEGER and STRING exist", text));
        }
        Ok(DataItem::Str(text.to_string()))
    }

    /// Name of a constant: % is dropped like for variables, $ stays
    fn const_key(raw: &str) -> String {
        raw.to_uppercase().trim_end_matches('%').to_string()
    }

    fn constant(&self, raw: &str) -> Option<Const> {
        let (name, types) = (Self::const_key(raw), self.types.borrow());
        types.consts.last().unwrap().get(&name).or_else(|| types.consts[0].get(&name)).cloned()
    }

    /// The name without its $ or % suffix
    fn base(raw: &str) -> String {
        raw.to_uppercase().trim_end_matches(['$', '%']).to_string()
    }

    fn default_string(&self, base: &str) -> bool {
        base.bytes().next().filter(u8::is_ascii_alphabetic).is_some_and(|c| self.types.borrow().def_string[(c.to_ascii_uppercase() - b'A') as usize])
    }

    /// Name of a variable as used in the source: the suffix, a declaration or DEFSTR decide if it is a string.
    /// % is dropped, as X% and X are the same INTEGER variable.
    fn var_name(&self, raw: &str) -> String {
        let up = raw.to_uppercase();
        if up.ends_with('$') { return up; }
        let base = Self::base(&up);
        let declared = self.types.borrow().scopes.last().unwrap().get(&base).copied();
        if declared.unwrap_or_else(|| self.default_string(&base)) { format!("{}$", base) } else { base }
    }

    /// Name of an array or function in a call: only declared names can be strings
    fn call_name(&self, raw: &str) -> String {
        let up = raw.to_uppercase();
        if up.ends_with('$') { return up; }
        let base = Self::base(&up);
        if self.types.borrow().scopes.last().unwrap().get(&base) == Some(&true) { format!("{}$", base) } else { base }
    }

    /// Records the type of a variable from its suffix, an AS clause or else DEFSTR, returns its name
    fn declare(&self, raw: &str, as_clause: Option<&Pair<Rule>>, line: usize) -> Result<String, Error> {
        let up = raw.to_uppercase();
        let base = Self::base(&up);
        if self.constant(raw).is_some() { return err(line, format!("{} is a constant and cannot be declared again", Self::const_key(raw))); }
        let suffix = if up.ends_with('$') { Some(true) } else if up.ends_with('%') { Some(false) } else { None };
        let declared = as_clause.map(|c| c.clone().into_inner().any(|p| p.as_rule() == Rule::type_name && p.as_str().eq_ignore_ascii_case("STRING")));
        if let (Some(a), Some(b)) = (suffix, declared) {
            if a != b { return err(line, format!("{} is declared as {}, which does not match its suffix", up, if b { "STRING" } else { "INTEGER" })); }
        }
        let string = suffix.or(declared).unwrap_or_else(|| self.default_string(&base));
        self.types.borrow_mut().scopes.last_mut().unwrap().insert(base.clone(), string);
        Ok(if string { format!("{}$", base) } else { base })
    }

    /// SUB and DEF FN parameters, declared in the scope that was just opened
    fn params(&self, pair: Pair<Rule>, line: usize) -> Result<Vec<String>, Error> {
        pair.into_inner().map(|param| {
            let mut parts = param.into_inner();
            let ident = parts.next().unwrap();
            self.declare(ident.as_str(), parts.next().as_ref(), line)
        }).collect()
    }

    fn do_cond(&self, pair: Pair<Rule>, line: usize) -> Result<(bool, Expr), Error> {
        let mut parts = pair.into_inner();
        let until = parts.next().unwrap().as_str().to_uppercase().starts_with("UNTIL");
        Ok((until, self.expr(parts.next().unwrap(), line)?))
    }

    fn case_item(&self, pair: Pair<Rule>, line: usize) -> Result<CaseItem, Error> {
        let inner = pair.into_inner().next().unwrap();
        match inner.as_rule() {
            Rule::case_is => {
                let mut parts = inner.into_inner().filter(|p| !is_keyword(p.as_rule()));
                let op = match parts.next().unwrap().as_str() {
                    "<>" => BinOp::Ne, "<=" => BinOp::Le, ">=" => BinOp::Ge, "<" => BinOp::Lt, ">" => BinOp::Gt, _ => BinOp::Eq
                };
                Ok(CaseItem::Is(op, self.expr(parts.next().unwrap(), line)?))
            }
            Rule::case_range => {
                let mut parts = inner.into_inner().filter(|p| !is_keyword(p.as_rule()));
                let from = self.expr(parts.next().unwrap(), line)?;
                Ok(CaseItem::Range(from, self.expr(parts.next().unwrap(), line)?))
            }
            _ => Ok(CaseItem::Value(self.expr(inner, line)?))
        }
    }

    fn branch(&self, pair: Pair<Rule>, line: usize) -> Result<Branch, Error> {
        let inner = pair.into_inner().next().unwrap();
        match inner.as_rule() {
            Rule::line_ref => Ok(Branch::Line(parse_line_ref(inner, line)?)),
            _ => Ok(Branch::Stmts(inner.into_inner().map(|p| self.stmt(p, line)).collect::<Result<Vec<_>, _>>()?))
        }
    }

    fn lvalue(&self, pair: Pair<Rule>, line: usize) -> Result<LValue, Error> {
        let mut parts = pair.into_inner();
        let raw = parts.next().unwrap().as_str();
        if self.constant(raw).is_some() { return err(line, format!("{} is a constant and cannot be assigned", Self::const_key(raw))); }
        match parts.next() {
            Some(index) => Ok(LValue::Index(self.call_name(raw), self.expr(index, line)?)),
            None => Ok(LValue::Var(self.var_name(raw)))
        }
    }

    fn int_call(&self, pair: Pair<Rule>, line: usize) -> Result<(String, Vec<Expr>), Error> {
        let mut parts = pair.into_inner();
        let name = parts.next().unwrap().as_str().to_string();
        Ok((name, self.args(parts.next(), line)?))
    }

    fn args(&self, pair: Option<Pair<Rule>>, line: usize) -> Result<Vec<Expr>, Error> {
        match pair {
            Some(args) => args.into_inner().map(|p| self.expr(p, line)).collect(),
            None => Ok(vec![])
        }
    }

    fn expr(&self, pair: Pair<Rule>, line: usize) -> Result<Expr, Error> {
        self.pratt
            .map_primary(|p| self.primary(p, line))
            .map_prefix(|op, rhs| Ok(match op.as_rule() {
                Rule::op_neg => Expr::Neg(Box::new(rhs?)),
                _ => Expr::Not(Box::new(rhs?))
            }))
            .map_infix(|lhs, op, rhs| {
                // EQV and IMP are built from the other operators, \ is the integer division / already is
                match op.as_rule() {
                    Rule::op_eqv => return Ok(Expr::Not(Box::new(Expr::Bin(BinOp::Xor, Box::new(lhs?), Box::new(rhs?))))),
                    Rule::op_imp => return Ok(Expr::Bin(BinOp::Or, Box::new(Expr::Not(Box::new(lhs?))), Box::new(rhs?))),
                    _ => {}
                }
                let op = match op.as_rule() {
                    Rule::op_add => BinOp::Add, Rule::op_sub => BinOp::Sub, Rule::op_mul => BinOp::Mul,
                    Rule::op_div | Rule::op_idiv => BinOp::Div, Rule::op_mod => BinOp::Mod, Rule::op_pow => BinOp::Pow,
                    Rule::op_shl => BinOp::Shl, Rule::op_shr => BinOp::Shr, Rule::op_and => BinOp::And,
                    Rule::op_or => BinOp::Or, Rule::op_xor => BinOp::Xor, Rule::op_eq => BinOp::Eq,
                    Rule::op_ne => BinOp::Ne, Rule::op_lt => BinOp::Lt, Rule::op_gt => BinOp::Gt,
                    Rule::op_le => BinOp::Le, Rule::op_ge => BinOp::Ge,
                    r => unreachable!("unexpected operator {:?}", r)
                };
                Ok(Expr::Bin(op, Box::new(lhs?), Box::new(rhs?)))
            })
            .parse(pair.into_inner())
    }

    fn primary(&self, pair: Pair<Rule>, line: usize) -> Result<Expr, Error> {
        match pair.as_rule() {
            Rule::number => Ok(Expr::Num(parse_number(pair.as_str(), line)?)),
            Rule::suffixed_number => Ok(Expr::Num(parse_number(pair.as_str().trim_end_matches('%'), line)?)),
            Rule::color => Ok(Expr::Num(crate::color::Color::parse(pair.as_str()))),
            Rule::string => { let s = pair.as_str(); Ok(Expr::Str(s[1..s.len() - 1].replace("\"\"", "\""))) }
            Rule::ident if pair.as_str().eq_ignore_ascii_case("INKEY$") => err(line, "INKEY$ is not supported, do not use input features"),
            Rule::ident => Ok(match self.constant(pair.as_str()) {
                Some(Const::Num(n) | Const::Color(n)) => Expr::Num(n),
                Some(Const::Str(s)) => Expr::Str(s),
                None => Expr::Var(self.var_name(pair.as_str()))
            }),
            Rule::call => {
                let mut parts = pair.into_inner();
                let raw = parts.next().unwrap().as_str().to_uppercase();
                match raw.as_str() {
                    "INPUT$" => return err(line, "INPUT$ is not supported, do not use input features"),
                    "TAB" | "SPC" => return err(line, format!("{} is not supported", raw)),
                    "POINT" => return err(line, "POINT is not supported"),
                    _ => {}
                }
                let name = self.call_name(&raw);
                Ok(Expr::Call(name, self.args(parts.next(), line)?))
            }
            Rule::int_call => {
                let (name, args) = self.int_call(pair, line)?;
                Ok(Expr::IntCall(name, args))
            }
            Rule::expr => self.expr(pair, line),
            r => unreachable!("unexpected primary {:?}", r)
        }
    }
}

/// The message for a type that does not exist in this dialect, which only has INTEGER and STRING.
fn unsupported_type(what: &str, name: &str) -> String {
    format!("{} is not supported, {}, only INTEGER (no suffix or %) and STRING ($) exist", what, name)
}

/// The type behind a suffix character, None for $ and % which are supported
fn suffix_type(c: char) -> Option<&'static str> {
    match c { '&' => Some("LONG (&)"), '!' => Some("SINGLE (!)"), '#' => Some("DOUBLE (#)"), _ => None }
}

/// Rejects everything that would need a data type other than INTEGER and STRING.
fn check_types(pair: &Pair<Rule>, line: usize) -> Result<(), Error> {
    for p in pair.clone().into_inner().flatten() {
        let text = p.as_str();
        match p.as_rule() {
            Rule::float_lit => {
                let kind = match text.chars().last() {
                    Some(c @ ('&' | '!' | '#')) => suffix_type(c).unwrap(),
                    _ if text.to_uppercase().contains('D') => "DOUBLE",
                    _ => "SINGLE"
                };
                return err(line, unsupported_type(&format!("the {} literal {}", kind, text), "there are no floating point numbers"));
            }
            Rule::suffixed_number | Rule::ident => {
                if let Some(kind) = text.chars().last().and_then(suffix_type) {
                    let what = if p.as_rule() == Rule::ident { "variable" } else { "literal" };
                    return err(line, unsupported_type(&format!("the {} {} {}", kind, what, text), "a number has no type suffix"));
                }
            }
            Rule::as_clause => {
                let mut parts = p.clone().into_inner().filter(|p| !is_keyword(p.as_rule()));
                let name = parts.next().unwrap().as_str().to_uppercase();
                let message = match name.as_str() {
                    "INTEGER" | "STRING" if parts.next().is_none() => continue,
                    "INTEGER" => String::from("INTEGER takes no length"),
                    "STRING" => String::from("fixed length strings (STRING * n) are not supported, only STRING variables with a dynamic length"),
                    "LONG" | "SINGLE" | "DOUBLE" => unsupported_type(&format!("the type {}", name), "there are no floating point or 32 bit numbers"),
                    _ => unsupported_type(&format!("the type {}", name), "user-defined types do not exist")
                };
                return err(line, message);
            }
            Rule::deftype_kw => {
                let kind = text.to_uppercase();
                if kind != "DEFINT" && kind != "DEFSTR" {
                    return err(line, unsupported_type(&kind, "there are no floating point or 32 bit numbers"));
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn parse_program(src: &str) -> Result<Vec<Line>, Error> {
    let mut pairs = BasicParser::parse(Rule::program, src).map_err(|e| {
        let line = match e.line_col { LineColLocation::Pos((l, _)) | LineColLocation::Span((l, _), _) => l };
        Error { line, msg: format!("syntax error\n{}", e) }
    })?;

    let builder = Builder { pratt: pratt(), types: RefCell::new(Types { scopes: vec![HashMap::new()], consts: vec![HashMap::new()], def_string: [false; 26], dim_seen: false }) };
    let mut lines = vec![];

    for pair in pairs.next().unwrap().into_inner() {
        if pair.as_rule() != Rule::line { continue; }
        let src_line = pair.line_col().0;
        let text = pair.as_str().trim().to_string();
        let mut number = None;
        let mut label = None;
        let mut stmts = vec![];
        check_types(&pair, src_line)?;
        for part in pair.into_inner() {
            match part.as_rule() {
                Rule::line_no => number = Some(parse_line_ref(part, src_line)?),
                Rule::label_def => label = Some(part.as_str().trim_end_matches(':').to_uppercase()),
                Rule::comment | Rule::comment_line => {}
                _ => stmts.push(builder.stmt(part, src_line)?)
            }
        }
        if number.is_some() || label.is_some() || !stmts.is_empty() {
            lines.push(Line { src_line, number, label, text, stmts });
        }
    }

    Ok(lines)
}

// ---------------------------------------------------------------------------------------------
// Code generation

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Loc {
    Reg(u8),
    Mem(usize)
}

pub struct Output {
    pub listing: String,
    pub variables: Vec<(String, Loc)>,
    pub arrays: Vec<(String, usize, usize)>,
    /// String literals, the position is the #db index
    pub strings: Vec<String>,
    pub labels: usize,
    pub jumps: usize
}

impl Output {
    pub fn memory_cells(&self) -> usize {
        self.variables.iter().filter(|(_, loc)| matches!(loc, Loc::Mem(_))).count()
            + self.arrays.iter().map(|(_, _, size)| size).sum::<usize>()
    }
}

enum Block {
    If { else_label: String, end_label: String, has_else: bool, used_end: bool, line: usize },
    Select { selector: Expr, next: Option<String>, end: String, has_else: bool, any_case: bool, line: usize },
    For { var: String, step: Expr, top: String, exit: String, line: usize },
    While { top: String, exit: String, line: usize },
    Do { top: String, exit: String, line: usize },
    Sub { name: String, is_function: bool, exit: String, skip: String, line: usize }
}

/// The SUB or FUNCTION being compiled, its variables are local unless SHARED
struct Scope {
    name: String,
    shared: HashSet<String>,
    /// position in the output where the locals are reset on every call
    entry: usize
}

const BUILTINS: [&str; 11] = ["ABS", "SGN", "MIN", "MAX", "RND", "LEN", "STR$", "MID$", "LEFT$", "RIGHT$", "PEEK"];

struct Codegen <'a> {
    functions: &'a Functions,
    interrupts: &'a [String],
    lookup: HashMap<String, String>,
    out: Vec<String>,
    act: Option<u8>,
    labels: usize,
    jumps: usize,
    uid: usize,
    variables: Vec<(String, Loc)>,
    arrays: Vec<(String, usize, usize)>,
    strings: Vec<String>,
    dynamic: usize,
    next_reg: u8,
    next_mem: usize,
    blocks: Vec<Block>,
    targets: HashSet<u32>,
    named_labels: HashSet<String>,
    /// SUBs and FUNCTIONs: parameters, is a FUNCTION
    subs: HashMap<String, (Vec<String>, bool)>,
    /// names made visible in every SUB and FUNCTION by DIM SHARED
    global_shared: HashSet<String>,
    /// OPTION BASE
    option_base: usize,
    /// all DATA items, the positions where the labels and line numbers read from, see RESTORE
    data: Vec<DataItem>,
    data_labels: HashMap<String, usize>,
    data_lines: HashMap<u32, usize>,
    scope: Option<Scope>,
    /// caller ("" for the main program), callee, line
    calls: Vec<(String, String, usize)>,
    line: usize
}

impl <'a> Codegen <'a> {
    fn new(interrupts: &'a [String], functions: &'a Functions) -> Self {
        Self {
            functions, interrupts,
            lookup: functions.keys().map(|k| (k.to_lowercase(), k.clone())).collect(),
            out: vec![], act: None, labels: 0, jumps: 0, uid: 0,
            variables: vec![], arrays: vec![], strings: vec![], dynamic: 0, next_reg: 0, next_mem: 0,
            blocks: vec![], targets: HashSet::new(), named_labels: HashSet::new(), subs: HashMap::new(), global_shared: HashSet::new(), option_base: 0, data: vec![], data_labels: HashMap::new(), data_lines: HashMap::new(), scope: None,
            calls: vec![], line: 0
        }
    }

    fn fail<T>(&self, msg: impl Into<String>) -> Result<T, Error> {
        err(self.line, msg)
    }

    // --- emitting

    fn emit(&mut self, instruction: String) {
        self.out.push(format!("    {}", instruction));
    }

    fn select(&mut self, reg: u8) {
        if self.act != Some(reg) {
            self.emit(format!("reg r{}", reg));
            self.act = Some(reg);
        }
    }

    fn label(&mut self, name: &str) {
        self.out.push(format!("{}:", name));
        self.labels += 1;
        self.act = None;
    }

    fn jump(&mut self, op: &str, target: &str) {
        self.emit(format!("{} {}", op, target));
        self.jumps += 1;
        if op == "cal" { self.act = None; }
    }

    fn new_label(&mut self, kind: &str) -> String {
        self.uid += 1;
        format!("_{}{}", kind, self.uid)
    }

    fn line_label(n: u32) -> String {
        format!("_line{}", n)
    }

    // --- symbols

    /// The name a variable or array has in the listing, locals of a SUB or FUNCTION are prefixed with its name
    fn q(&self, name: &str) -> String {
        match &self.scope {
            Some(scope) if !name.contains('.') && !scope.shared.contains(name) && !self.global_shared.contains(name) => format!("{}.{}", scope.name, name),
            _ => String::from(name)
        }
    }

    fn array(&self, name: &str) -> Option<(usize, usize)> {
        let name = self.q(name);
        self.arrays.iter().find(|a| a.0 == name).map(|a| (a.1, a.2))
    }

    /// A FUNCTION without parameters used like a variable, except inside itself where its name is the result
    fn function_var(&self, name: &str) -> bool {
        self.subs.get(name).is_some_and(|(params, is_function)| *is_function && params.is_empty())
            && self.scope.as_ref().is_none_or(|scope| scope.name != name)
    }

    fn var(&mut self, name: &str) -> Result<Loc, Error> {
        if let Some((_, is_function)) = self.subs.get(name) {
            if !(*is_function && self.scope.as_ref().is_some_and(|scope| scope.name == name)) {
                return self.fail(if *is_function { format!("{} is a FUNCTION, call it with its arguments", name) } else { format!("{} is a SUB and has no value", name) });
            }
        }
        let name = &self.q(name);
        if let Some((_, loc)) = self.variables.iter().find(|v| v.0 == *name) { return Ok(*loc); }
        if self.array(name).is_some() { return self.fail(format!("{} is an array, use {}(index)", name, name)); }
        let loc = if self.next_reg < VAR_REGS {
            self.next_reg += 1;
            Loc::Reg(self.next_reg - 1)
        } else {
            self.next_mem += 1;
            Loc::Mem(self.next_mem - 1)
        };
        self.variables.push((String::from(name), loc));
        Ok(loc)
    }

    fn resolve(&self, name: &str) -> Result<(String, usize, usize), Error> {
        match self.lookup.get(&name.to_lowercase()) {
            Some(key) => { let f = self.functions[key]; Ok((key.clone(), f.2, f.3)) }
            None => {
                let int_name = name.split('.').next().unwrap_or("");
                if self.interrupts.iter().any(|i| i.eq_ignore_ascii_case(int_name)) {
                    self.fail(format!("interrupt {} has no function {}", int_name, name))
                } else {
                    self.fail(format!("unknown interrupt function {} (loaded interrupts: {})", name,
                        if self.interrupts.is_empty() { String::from("none") } else { self.interrupts.join(", ") }))
                }
            }
        }
    }

    /// Operand usable directly as instruction argument: a constant or a variable held in a register.
    fn operand(&mut self, expr: &Expr) -> Result<Option<String>, Error> {
        Ok(match expr {
            Expr::Num(n) => Some(format!("#{}", n)),
            Expr::Str(s) => Some(format!("#{}", self.string(s)?)),
            Expr::Neg(inner) => match inner.as_ref() { Expr::Num(n) => Some(format!("#{}", n.wrapping_neg())), _ => None },
            Expr::Var(name) if self.function_var(name) => None,
            Expr::Var(name) => match self.var(name)? { Loc::Reg(_) => Some(self.q(name)), Loc::Mem(_) => None },
            _ => None
        })
    }

    /// #db index of a string literal, identical strings share an entry
    fn string(&mut self, s: &str) -> Result<usize, Error> {
        if let Some(i) = self.strings.iter().position(|e| e == s) { return Ok(i); }
        if self.strings.len() > u8::MAX as usize - 1 { return self.fail(format!("out of string slots: more than {} different string literals, the maximum is {} (string indices are one byte and also hold the dynamic strings)", u8::MAX, u8::MAX)); }
        self.strings.push(String::from(s));
        Ok(self.strings.len() - 1)
    }

    /// The listing cuts lines at ';' and a #db string ends at '"', so the text from the first
    /// of these on is written as byte literals
    fn db_entry(s: &str) -> String {
        let split = s.find(|c| c == ';' || c == '"').unwrap_or(s.len());
        let mut entry = format!("#db \"{}\"", &s[..split]);
        for byte in s[split..].bytes() { entry.push_str(&format!(", 0x{:02x}", byte)); }
        entry
    }

    /// String typed expressions: literals, names ending in $ (variables, arrays, string functions) and joins
    fn is_string(expr: &Expr) -> bool {
        match expr {
            Expr::Str(_) => true,
            Expr::Var(name) | Expr::Call(name, _) => name.ends_with('$'),
            Expr::Bin(BinOp::Add, lhs, rhs) => Self::is_string(lhs) || Self::is_string(rhs),
            _ => false
        }
    }

    fn constant(expr: &Expr) -> Option<VMAtom> {
        match expr {
            Expr::Num(n) => Some(*n),
            Expr::Neg(inner) => Self::constant(inner).map(|n| n.wrapping_neg()),
            _ => None
        }
    }

    // --- expressions, result ends up in the accumulator

    fn expr(&mut self, expr: &Expr) -> Result<(), Error> {
        if let Some(operand) = self.operand(expr)? {
            self.select(ACC);
            self.emit(format!("set {}", operand));
            return Ok(());
        }
        match expr {
            Expr::Acc => {}
            Expr::Var(name) if self.function_var(name) => self.call_sub(name, &[], true)?,
            Expr::Var(name) => {
                self.select(ACC);
                self.emit(format!("loa {}", self.q(name)));
            }
            Expr::Neg(inner) => {
                self.expr(inner)?;
                self.select(ACC);
                self.emit(String::from("neg"));
            }
            Expr::Not(inner) => {
                self.expr(inner)?;
                self.math("NOT", "not", &["r13"])?;
            }
            Expr::Call(name, args) => self.call(name, args)?,
            Expr::IntCall(name, args) => {
                let (key, _, returns) = self.resolve(name)?;
                if returns != 1 { return self.fail(format!("{} returns {} values and cannot be used in an expression", key, returns)); }
                self.int_call(name, args, &[String::from("r13")])?;
            }
            Expr::Bin(op, ..) if op.is_comparison() => {
                let otherwise = self.new_label("false");
                let end = self.new_label("cmp");
                self.branch(expr, false, &otherwise)?;
                self.select(ACC);
                self.emit(String::from("set #-1"));
                self.jump("jmp", &end);
                self.label(&otherwise);
                self.select(ACC);
                self.emit(String::from("set #0"));
                self.label(&end);
                self.act = Some(ACC);
            }
            Expr::Bin(BinOp::Add, lhs, rhs) if Self::is_string(lhs) || Self::is_string(rhs) => {
                if !Self::is_string(lhs) || !Self::is_string(rhs) {
                    return self.fail("+ cannot join a string and a number, convert the number with STR$ or use a name ending in $");
                }
                let slot = self.string_function("+", "concat", 3)?;
                self.push_arg(rhs)?;
                self.push_arg(lhs)?;
                self.emit(format!("psh {}", slot));
                self.string_result("concat", &slot)?;
            }
            Expr::Bin(op, lhs, rhs) => match op.math_function() {
                Some(function) => self.math_binary(*op, function, lhs, rhs)?,
                None => self.arith(*op, lhs, rhs)?
            },
            Expr::Num(_) | Expr::Str(_) => unreachable!()
        }
        Ok(())
    }

    /// Native addition and subtraction; the last instruction emitted sets the flags for the result.
    fn arith(&mut self, op: BinOp, lhs: &Expr, rhs: &Expr) -> Result<(), Error> {
        let ins = if op == BinOp::Add { "add" } else { "sub" };
        if let Some(r) = self.operand(rhs)? {
            self.expr(lhs)?;
            self.select(ACC);
            self.emit(format!("{} {}", ins, r));
        } else if let (BinOp::Add, Some(l)) = (op, self.operand(lhs)?) {
            self.expr(rhs)?;
            self.select(ACC);
            self.emit(format!("add {}", l));
        } else {
            self.expr(rhs)?;
            self.emit(String::from("psh r13"));
            self.expr(lhs)?;
            self.emit(String::from("pop r12"));
            self.select(ACC);
            self.emit(format!("{} r12", ins));
        }
        Ok(())
    }

    fn math(&mut self, symbol: &str, function: &str, args: &[&str]) -> Result<(), Error> {
        let key = format!("math.{}", function);
        match self.functions.get(&key) {
            Some(f) if f.2 == args.len() && f.3 == 1 => {}
            Some(_) => return self.fail(format!("{} in math.csv does not match the signature needed for {}", key, symbol)),
            None => return self.fail(format!("operator {} needs {} from the math interrupt, add math to the interrupts", symbol, key))
        }
        self.emit(format!("r13 = {}({})", key, args.join(", ")));
        Ok(())
    }

    fn math_binary(&mut self, op: BinOp, function: &str, lhs: &Expr, rhs: &Expr) -> Result<(), Error> {
        match (self.operand(lhs)?, self.operand(rhs)?) {
            (Some(l), Some(r)) => self.math(op.symbol(), function, &[&l, &r]),
            (None, Some(r)) => { self.expr(lhs)?; self.math(op.symbol(), function, &["r13", &r]) }
            (Some(l), None) => { self.expr(rhs)?; self.math(op.symbol(), function, &[&l, "r13"]) }
            (None, None) => {
                self.expr(rhs)?;
                self.emit(String::from("psh r13"));
                self.expr(lhs)?;
                self.emit(String::from("pop r12"));
                self.math(op.symbol(), function, &["r13", "r12"])
            }
        }
    }

    fn call(&mut self, name: &str, args: &[Expr]) -> Result<(), Error> {
        if let Some((base, size)) = self.array(name) {
            if args.len() != 1 { return self.fail(format!("array {} takes exactly one index", name)); }
            return self.array_load(name, base, size, &args[0]);
        }
        match name {
            "SGN" => {
                if args.len() != 1 { return self.fail("SGN takes exactly one argument"); }
                let end = self.new_label("sgn");
                let negative = self.new_label("neg");
                self.expr(&args[0])?;
                self.select(ACC);
                self.emit(String::from("add #0"));
                self.jump("jpz", &end);
                self.jump("jps", &negative);
                self.emit(String::from("set #1"));
                self.jump("jmp", &end);
                self.label(&negative);
                self.select(ACC);
                self.emit(String::from("set #-1"));
                self.label(&end);
                self.act = Some(ACC);
                Ok(())
            }
            "MIN" | "MAX" => {
                if args.len() < 2 { return self.fail(format!("{} takes at least two arguments", name)); }
                if args.iter().any(Self::is_string) { return self.fail(format!("{} takes numbers", name)); }
                if args.len() > 2 {
                    let (last, rest) = args.split_last().unwrap();
                    return self.call(name, &[Expr::Call(String::from(name), rest.to_vec()), last.clone()]);
                }
                // r13 = first, r12 = second, keep the first if it is the MIN/MAX
                self.expr(&args[1])?;
                self.emit(String::from("psh r13"));
                self.expr(&args[0])?;
                self.emit(String::from("pop r12"));
                self.select(ACC);
                self.emit(String::from("psh r13"));
                self.emit(String::from("sub r12"));
                self.emit(String::from("pop r13"));
                let keep = self.new_label(if name == "MIN" { "min" } else { "max" });
                self.jump_less(name == "MIN", &keep);
                self.select(ACC);
                self.emit(String::from("set r12"));
                self.label(&keep);
                self.act = Some(ACC);
                Ok(())
            }
            "RND" => {
                if args.len() != 2 { return self.fail("RND takes the lowest and the highest value"); }
                if !self.functions.contains_key("math.random") { return self.fail("RND needs math.random from the math interrupt, add math to the interrupts"); }
                self.expr(&Expr::IntCall(String::from("math.random"), args.to_vec()))
            }
            _ if self.subs.contains_key(name) => self.call_sub(name, args, true),
            "STR$" => {
                if args.len() != 1 { return self.fail("STR$ takes exactly one argument"); }
                let slot = self.string_function("STR$", "format", 2)?;
                if !self.functions.get("string.concat").is_some_and(|f| f.2 == 3 && f.3 == 0) {
                    return self.fail("STR$ needs string.concat with 3 arguments from the string interrupt, add string to the interrupts");
                }
                // like in QBasic a number that is not negative gets a blank in front of it, in the place of the sign.
                // The number is needed twice, so anything that is no plain variable or constant is evaluated once into a hidden variable.
                let number = match &args[0] {
                    Expr::Num(_) => args[0].clone(),
                    Expr::Var(name) if !self.function_var(name) => args[0].clone(),
                    other => {
                        let hidden = self.new_label("strn");
                        self.assign(&LValue::Var(hidden.clone()), other)?;
                        Expr::Var(hidden)
                    }
                };
                self.push_arg(&number)?;
                self.emit(format!("psh {}", slot));
                self.string_result("format", &slot)?;
                let negative = self.new_label("strsign");
                self.branch(&Expr::Bin(BinOp::Lt, Box::new(number), Box::new(Expr::Num(0))), true, &negative)?;
                let blank = self.string(" ")?;
                self.emit(format!("psh {}", slot));
                self.emit(format!("psh #{}", blank));
                self.emit(format!("psh {}", slot));
                self.emit(String::from("string.concat(_)"));
                self.label(&negative);
                self.select(ACC);
                self.emit(format!("set {}", slot));
                self.act = Some(ACC);
                Ok(())
            }
            "LEN" => {
                if args.len() != 1 { return self.fail("LEN takes exactly one argument"); }
                self.expr(&Expr::IntCall(String::from("string.get_length"), args.to_vec()))
            }
            "MID$" | "LEFT$" | "RIGHT$" => {
                let one = Box::new(Expr::Num(1));
                // string.substr(dest, src, start, end) with 0-based start and exclusive end
                let (start, end) = match (name, args) {
                    // the assembler does not accept VMAtom::MAX itself
                    ("MID$", [_, start]) => (Expr::Bin(BinOp::Sub, Box::new(start.clone()), one), Expr::Num(VMAtom::MAX - 1)),
                    ("MID$", [_, start, length]) => {
                        let first = Expr::Bin(BinOp::Sub, Box::new(start.clone()), one);
                        (first.clone(), Expr::Bin(BinOp::Add, Box::new(first), Box::new(length.clone())))
                    }
                    ("LEFT$", [_, n]) => (Expr::Num(0), n.clone()),
                    ("RIGHT$", [s, n]) => {
                        let length = Expr::Call(String::from("LEN"), vec![s.clone()]);
                        (Expr::Bin(BinOp::Sub, Box::new(length.clone()), Box::new(n.clone())), length)
                    }
                    ("MID$", _) => return self.fail("MID$ takes a string, a start position and an optional length"),
                    _ => return self.fail(format!("{} takes a string and a number of characters", name))
                };
                let slot = self.string_function(name, "substr", 4)?;
                self.push_arg(&end)?;
                self.expr(&start)?;
                self.clamp_positive();
                self.emit(String::from("psh r13"));
                self.push_arg(&args[0])?;
                self.emit(format!("psh {}", slot));
                self.string_result("substr", &slot)
            }
            "ABS" => {
                if args.len() != 1 { return self.fail("ABS takes exactly one argument"); }
                let skip = self.new_label("abs");
                self.expr(&args[0])?;
                self.select(ACC);
                self.emit(String::from("add #0"));
                self.emit(String::from("inv"));
                self.jump("jps", &skip);
                self.emit(String::from("neg"));
                self.label(&skip);
                self.act = Some(ACC);
                Ok(())
            }
            // the cell at an address, whatever it holds: the vm only checks that it is inside of its memory
            "PEEK" => {
                if args.len() != 1 { return self.fail("PEEK takes exactly one argument"); }
                match Self::constant(&args[0]) {
                    Some(address) if address >= 0 => { self.select(ACC); self.emit(format!("loa #{}", address)); }
                    _ => { self.expr(&args[0])?; self.select(ACC); self.emit(String::from("loa r13")); }
                }
                self.act = Some(ACC);
                Ok(())
            }
            "CSNG" | "CDBL" | "CLNG" | "CVS" | "CVD" | "CVL" => self.fail(format!("{} is not supported, there are no floating point or 32 bit numbers, only INTEGER and STRING exist", name)),
            _ => self.fail(format!("unknown function {} (arrays need to be declared with DIM, interrupt functions are called as interrupt.function)", name))
        }
    }

    /// Checks the string interrupt function a string function is built on and reserves the call site's dynamic entry.
    fn string_function(&mut self, name: &str, function: &str, arguments: usize) -> Result<String, Error> {
        let key = format!("string.{}", function);
        match self.functions.get(&key) {
            Some(f) if f.2 == arguments && f.3 == 0 => {}
            _ => return self.fail(format!("{} needs {} with {} arguments from the string interrupt, add string to the interrupts", name, key, arguments))
        }
        if self.strings.len() + self.dynamic >= u8::MAX as usize { return self.fail(format!("out of string slots: {} string literals plus {} string operations (STR$, MID$, LEFT$, RIGHT$ and string +, one slot per call site) leave no room, at most {} fit the string indices", self.strings.len(), self.dynamic, u8::MAX)); }
        self.dynamic += 1;
        Ok(format!("_str{}", self.dynamic - 1))
    }

    /// Calls the string function with its arguments on the stack, the result is the dynamic entry's index.
    fn string_result(&mut self, function: &str, slot: &str) -> Result<(), Error> {
        self.emit(format!("string.{}(_)", function));
        self.select(ACC);
        self.emit(format!("set {}", slot));
        Ok(())
    }

    fn push_arg(&mut self, arg: &Expr) -> Result<(), Error> {
        match self.operand(arg)? {
            Some(op) => self.emit(format!("psh {}", op)),
            None => { self.expr(arg)?; self.emit(String::from("psh r13")); }
        }
        Ok(())
    }

    /// Sets a negative accumulator to 0.
    fn clamp_positive(&mut self) {
        let skip = self.new_label("clamp");
        self.select(ACC);
        self.emit(String::from("add #0"));
        self.emit(String::from("inv"));
        self.jump("jps", &skip);
        self.emit(String::from("set #0"));
        self.label(&skip);
        self.act = Some(ACC);
    }

    fn array_load(&mut self, name: &str, base: usize, size: usize, index: &Expr) -> Result<(), Error> {
        if let Some(i) = Self::constant(index) {
            if i < self.option_base as VMAtom || i as usize >= size { return self.fail(format!("index {} is out of bounds for {}({}..{})", i, name, self.option_base, size - 1)); }
            self.select(ACC);
            self.emit(format!("loa #{}", base + i as usize));
        } else {
            self.expr(index)?;
            self.select(ACC);
            if base > 0 { self.emit(format!("add {}", self.q(name))); }
            self.emit(String::from("loa r13"));
        }
        Ok(())
    }

    /// Calls an interrupt function, arguments that are no plain operands are pushed onto the stack first.
    fn int_call(&mut self, name: &str, args: &[Expr], outputs: &[String]) -> Result<(), Error> {
        let (key, arguments, returns) = self.resolve(name)?;
        if args.len() != arguments { return self.fail(format!("{} expects {} argument(s), got {}", key, arguments, args.len())); }
        if outputs.len() != returns { return self.fail(format!("{} returns {} value(s), got {} target(s)", key, returns, outputs.len())); }

        let mut operands = vec![];
        for arg in args {
            match self.operand(arg)? {
                Some(op) => operands.push(op),
                None => break
            }
        }
        let inputs = if operands.len() == args.len() {
            operands.join(", ")
        } else {
            for arg in args.iter().rev() {
                match self.operand(arg)? {
                    Some(op) => self.emit(format!("psh {}", op)),
                    None => { self.expr(arg)?; self.emit(String::from("psh r13")); }
                }
            }
            String::from("_")
        };

        if outputs.is_empty() {
            self.emit(format!("{}({})", key, inputs));
        } else {
            self.emit(format!("{} = {}({})", outputs.join(", "), key, inputs));
        }
        Ok(())
    }

    /// Sets the flags for comparing lhs with rhs: numbers by lhs - rhs, strings by their content
    /// through string.compare, whose result (-1, 0, 1) is compared with 0.
    fn compare(&mut self, lhs: &Expr, rhs: &Expr) -> Result<(), Error> {
        if Self::is_string(lhs) || Self::is_string(rhs) {
            if !Self::is_string(lhs) || !Self::is_string(rhs) {
                return self.fail("cannot compare a string with a number, convert the number with STR$ or use a name ending in $");
            }
            match self.functions.get("string.compare") {
                Some(f) if f.2 == 2 && f.3 == 1 => {}
                _ => return self.fail("comparing strings needs string.compare with 2 arguments and 1 result from the string interrupt, add string to the interrupts")
            }
            self.int_call("string.compare", &[lhs.clone(), rhs.clone()], &[String::from("r13")])?;
            self.select(ACC);
            self.emit(String::from("add #0"));
            return Ok(());
        }
        self.arith(BinOp::Sub, lhs, rhs)
    }

    /// Jumps to target if the flags of the last compare say lhs < rhs (`when`) or lhs >= rhs (not `when`).
    /// Less is sign != overflow, which stays right when lhs - rhs overflows (the vm keeps overflow in carry).
    fn jump_less(&mut self, when: bool, target: &str) {
        let overflow = self.new_label("ovf");
        let done = self.new_label("cmp");
        self.jump("jpc", &overflow);
        if !when { self.emit(String::from("inv")); }
        self.jump("jps", target);
        self.jump("jmp", &done);
        self.label(&overflow);
        if when { self.emit(String::from("inv")); }
        self.jump("jps", target);
        self.label(&done);
    }

    /// Jumps to target if the condition evaluates to `when`.
    fn branch(&mut self, cond: &Expr, when: bool, target: &str) -> Result<(), Error> {
        match cond {
            Expr::Not(inner) => self.branch(inner, !when, target)?,
            Expr::Bin(BinOp::And, a, b) => if when {
                let skip = self.new_label("and");
                self.branch(a, false, &skip)?;
                self.branch(b, true, target)?;
                self.label(&skip);
            } else {
                self.branch(a, false, target)?;
                self.branch(b, false, target)?;
            },
            Expr::Bin(BinOp::Or, a, b) => if when {
                self.branch(a, true, target)?;
                self.branch(b, true, target)?;
            } else {
                let skip = self.new_label("or");
                self.branch(a, true, &skip)?;
                self.branch(b, false, target)?;
                self.label(&skip);
            },
            Expr::Bin(op, lhs, rhs) if op.is_comparison() => {
                // only =, <>, < and >= map onto the zero and sign flags, > and <= swap their operands
                let (op, lhs, rhs) = match op {
                    BinOp::Gt => (BinOp::Lt, rhs, lhs),
                    BinOp::Le => (BinOp::Ge, rhs, lhs),
                    _ => (*op, lhs, rhs)
                };
                self.compare(lhs, rhs)?;
                match (op, when) {
                    (BinOp::Eq, true) | (BinOp::Ne, false) => self.jump("jpz", target),
                    (BinOp::Eq, false) | (BinOp::Ne, true) => { self.emit(String::from("inv")); self.jump("jpz", target); }
                    (BinOp::Lt, when) => self.jump_less(when, target),
                    (_, when) => self.jump_less(!when, target)
                }
            }
            _ => {
                self.expr(cond)?;
                self.select(ACC);
                self.emit(String::from("add #0"));
                if when { self.emit(String::from("inv")); }
                self.jump("jpz", target);
            }
        }
        Ok(())
    }

    // --- statements

    fn assign(&mut self, target: &LValue, value: &Expr) -> Result<(), Error> {
        match target {
            LValue::Var(name) => {
                let loc = self.var(name)?;
                if let Expr::IntCall(function, args) = value {
                    if self.resolve(function)?.2 == 1 { return self.int_call(function, args, &[self.q(name)]); }
                }
                match loc {
                    Loc::Reg(reg) => {
                        if let Expr::Bin(op @ (BinOp::Add | BinOp::Sub), lhs, rhs) = value {
                            if matches!(lhs.as_ref(), Expr::Var(v) if v == name) && !Self::is_string(value) {
                                if let Some(r) = self.operand(rhs)? {
                                    self.select(reg);
                                    self.emit(format!("{} {}", if *op == BinOp::Add { "add" } else { "sub" }, r));
                                    return Ok(());
                                }
                            }
                        }
                        match self.operand(value)? {
                            Some(op) => { self.select(reg); self.emit(format!("set {}", op)); }
                            None => { self.expr(value)?; self.select(reg); self.emit(String::from("set r13")); }
                        }
                    }
                    Loc::Mem(_) => {
                        self.expr(value)?;
                        self.select(ACC);
                        self.emit(format!("sto {}", self.q(name)));
                    }
                }
            }
            LValue::Index(name, index) => {
                let (base, size) = match self.array(name) { Some(a) => a, None => return self.fail(format!("array {} is not declared with DIM", name)) };
                if let Some(i) = Self::constant(index) {
                    if i < self.option_base as VMAtom || i as usize >= size { return self.fail(format!("index {} is out of bounds for {}({}..{})", i, name, self.option_base, size - 1)); }
                    self.expr(value)?;
                    self.select(ACC);
                    self.emit(format!("sto #{}", base + i as usize));
                } else {
                    self.expr(value)?;
                    self.emit(String::from("psh r13"));
                    self.expr(index)?;
                    self.select(ACC);
                    if base > 0 { self.emit(format!("add {}", self.q(name))); }
                    self.emit(String::from("pop r12"));
                    self.select(TMP);
                    self.emit(String::from("sto r13"));
                }
            }
        }
        Ok(())
    }

    fn output(&mut self, target: &LValue) -> Result<String, Error> {
        match target {
            LValue::Var(name) => { self.var(name)?; Ok(self.q(name)) }
            LValue::Index(name, index) => match (self.array(name), Self::constant(index)) {
                (Some((base, size)), Some(i)) if i >= self.option_base as VMAtom && (i as usize) < size => Ok(format!("#{}", base + i as usize)),
                (None, _) => self.fail(format!("array {} is not declared with DIM", name)),
                _ => self.fail("multiple return values can only be assigned to variables and array elements with constant index")
            }
        }
    }

    fn body(&mut self, branch: &Branch) -> Result<(), Error> {
        match branch {
            Branch::Line(n) => { self.jump("jmp", &Self::line_label(*n)); Ok(()) }
            Branch::Stmts(stmts) => { for stmt in stmts { self.stmt(stmt)?; } Ok(()) }
        }
    }

    fn stmt(&mut self, stmt: &Stmt) -> Result<(), Error> {
        match stmt {
            Stmt::Let(target, value) => self.assign(target, value)?,
            Stmt::MultiAssign(targets, name, args) => {
                let outputs = targets.iter().map(|t| self.output(t)).collect::<Result<Vec<_>, _>>()?;
                self.int_call(name, args, &outputs)?;
            }
            Stmt::Call(name, args) => {
                let (_, _, returns) = self.resolve(name)?;
                self.int_call(name, args, &vec![String::from("r12"); returns])?;
            }
            Stmt::Dim(..) | Stmt::OptionBase(_) | Stmt::Declare(..) | Stmt::Data(_) => {}
            Stmt::If(cond, then, otherwise) => match (then, otherwise) {
                (Branch::Line(n), otherwise) => {
                    self.branch(cond, true, &Self::line_label(*n))?;
                    if let Some(b) = otherwise { self.body(b)?; }
                }
                (then, None) => {
                    let end = self.new_label("endif");
                    self.branch(cond, false, &end)?;
                    self.body(then)?;
                    self.label(&end);
                }
                (then, Some(Branch::Line(n))) => {
                    self.branch(cond, false, &Self::line_label(*n))?;
                    self.body(then)?;
                }
                (then, Some(otherwise)) => {
                    let else_label = self.new_label("else");
                    let end = self.new_label("endif");
                    self.branch(cond, false, &else_label)?;
                    self.body(then)?;
                    self.jump("jmp", &end);
                    self.label(&else_label);
                    self.body(otherwise)?;
                    self.label(&end);
                }
            },
            Stmt::BlockIf(cond) => {
                let else_label = self.new_label("else");
                let end_label = self.new_label("endif");
                self.branch(cond, false, &else_label)?;
                self.blocks.push(Block::If { else_label, end_label, has_else: false, used_end: false, line: self.line });
            }
            Stmt::ElseIf(cond) => match self.blocks.last_mut() {
                Some(Block::If { else_label, end_label, has_else: false, used_end, .. }) => {
                    *used_end = true;
                    let (previous, end_label) = (else_label.clone(), end_label.clone());
                    self.uid += 1;
                    let next = format!("_else{}", self.uid);
                    if let Some(Block::If { else_label, .. }) = self.blocks.last_mut() { *else_label = next.clone(); }
                    self.jump("jmp", &end_label);
                    self.label(&previous);
                    self.branch(cond, false, &next)?;
                }
                _ => return self.fail("ELSEIF without IF ... THEN")
            },
            Stmt::Else => match self.blocks.last_mut() {
                Some(Block::If { else_label, end_label, has_else, used_end, .. }) if !*has_else => {
                    *has_else = true;
                    *used_end = true;
                    let (else_label, end_label) = (else_label.clone(), end_label.clone());
                    self.jump("jmp", &end_label);
                    self.label(&else_label);
                }
                _ => return self.fail("ELSE without IF ... THEN")
            },
            Stmt::EndIf => match self.blocks.pop() {
                Some(Block::If { else_label, end_label, has_else, used_end, .. }) => {
                    if !has_else { self.label(&else_label); }
                    if used_end { self.label(&end_label); }
                }
                _ => return self.fail("END IF without IF ... THEN")
            },
            Stmt::Select(value) => {
                let selector = if matches!(value, Expr::Str(_)) || self.operand(value)?.is_some() { value.clone() } else {
                    self.uid += 1;
                    let hidden = format!("_select{}{}", self.uid, if Self::is_string(value) { "$" } else { "" });
                    self.assign(&LValue::Var(hidden.clone()), value)?;
                    Expr::Var(hidden)
                };
                let end = self.new_label("endselect");
                self.blocks.push(Block::Select { selector, next: None, end, has_else: false, any_case: false, line: self.line });
            }
            Stmt::Case(items) => {
                let (selector, next) = match self.blocks.last() {
                    Some(Block::Select { has_else: false, selector, next, end, any_case, .. }) => {
                        let previous_case = *any_case;
                        let (selector, next, end) = (selector.clone(), next.clone(), end.clone());
                        if previous_case { self.jump("jmp", &end); }
                        (selector, next)
                    }
                    Some(Block::Select { .. }) => return self.fail("CASE after CASE ELSE"),
                    _ => return self.fail("CASE without SELECT CASE")
                };
                if let Some(next) = next { self.label(&next); }
                let next = match items {
                    None => None,
                    Some(items) => {
                        let body = self.new_label("case");
                        let next = self.new_label("nextcase");
                        for item in items {
                            let cond = match item {
                                CaseItem::Value(v) => Expr::Bin(BinOp::Eq, Box::new(selector.clone()), Box::new(v.clone())),
                                CaseItem::Is(op, v) => Expr::Bin(*op, Box::new(selector.clone()), Box::new(v.clone())),
                                CaseItem::Range(from, to) => Expr::Bin(BinOp::And,
                                    Box::new(Expr::Bin(BinOp::Ge, Box::new(selector.clone()), Box::new(from.clone()))),
                                    Box::new(Expr::Bin(BinOp::Le, Box::new(selector.clone()), Box::new(to.clone()))))
                            };
                            self.branch(&cond, true, &body)?;
                        }
                        self.jump("jmp", &next);
                        self.label(&body);
                        Some(next)
                    }
                };
                if let Some(Block::Select { next: n, has_else, any_case, .. }) = self.blocks.last_mut() {
                    *has_else = next.is_none();
                    *n = next;
                    *any_case = true;
                }
            }
            Stmt::EndSelect => match self.blocks.pop() {
                Some(Block::Select { next, end, .. }) => {
                    if let Some(next) = next { self.label(&next); }
                    self.label(&end);
                }
                _ => return self.fail("END SELECT without SELECT CASE")
            },
            Stmt::For(var, start, limit, step) => {
                if self.array(var).is_some() { return self.fail(format!("{} is an array and cannot be a loop variable", var)); }
                if var.ends_with('$') { return self.fail("a FOR variable cannot be a string"); }
                self.assign(&LValue::Var(var.clone()), start)?;
                let limit = if self.operand(limit)?.is_some() { limit.clone() } else {
                    self.uid += 1;
                    let hidden = format!("_limit{}", self.uid);
                    self.assign(&LValue::Var(hidden.clone()), limit)?;
                    Expr::Var(hidden)
                };
                let step = match step.as_ref().map(|s| (s, Self::constant(s))) {
                    None => Expr::Num(1),
                    Some((_, Some(0))) => return self.fail("STEP must not be 0"),
                    Some((_, Some(k))) => Expr::Num(k),
                    Some((s, None)) => {
                        self.uid += 1;
                        let hidden = format!("_step{}", self.uid);
                        self.assign(&LValue::Var(hidden.clone()), s)?;
                        Expr::Var(hidden)
                    }
                };
                let top = self.new_label("for");
                let exit = self.new_label("next");
                self.label(&top);
                let counter = Expr::Var(var.clone());
                let past_limit = |counter: &Expr, limit: &Expr, down: bool| if down {
                    Expr::Bin(BinOp::Lt, Box::new(counter.clone()), Box::new(limit.clone()))
                } else {
                    Expr::Bin(BinOp::Lt, Box::new(limit.clone()), Box::new(counter.clone()))
                };
                match Self::constant(&step) {
                    Some(k) => self.branch(&past_limit(&counter, &limit, k < 0), true, &exit)?,
                    None => {
                        let down = self.new_label("down");
                        let body = self.new_label("body");
                        self.branch(&Expr::Bin(BinOp::Lt, Box::new(step.clone()), Box::new(Expr::Num(0))), true, &down)?;
                        self.branch(&past_limit(&counter, &limit, false), true, &exit)?;
                        self.jump("jmp", &body);
                        self.label(&down);
                        self.branch(&past_limit(&counter, &limit, true), true, &exit)?;
                        self.label(&body);
                    }
                }
                self.blocks.push(Block::For { var: var.clone(), step, top, exit, line: self.line });
            }
            Stmt::Next(name) => match self.blocks.pop() {
                Some(Block::For { var, step, top, exit, .. }) => {
                    if let Some(name) = name { if *name != var { return self.fail(format!("NEXT {} does not match FOR {}", name, var)); } }
                    self.assign(&LValue::Var(var.clone()), &Expr::Bin(BinOp::Add, Box::new(Expr::Var(var.clone())), Box::new(step)))?;
                    self.jump("jmp", &top);
                    self.label(&exit);
                }
                _ => return self.fail("NEXT without FOR")
            },
            Stmt::While(cond) => {
                let top = self.new_label("while");
                let exit = self.new_label("wend");
                self.label(&top);
                self.branch(cond, false, &exit)?;
                self.blocks.push(Block::While { top, exit, line: self.line });
            }
            Stmt::Wend => match self.blocks.pop() {
                Some(Block::While { top, exit, .. }) => { self.jump("jmp", &top); self.label(&exit); }
                _ => return self.fail("WEND without WHILE")
            },
            Stmt::Do(cond) => {
                let top = self.new_label("do");
                let exit = self.new_label("loop");
                self.label(&top);
                // DO WHILE leaves when the condition is false, DO UNTIL when it is true
                if let Some((until, cond)) = cond { self.branch(cond, *until, &exit)?; }
                self.blocks.push(Block::Do { top, exit, line: self.line });
            }
            Stmt::Loop(cond) => match self.blocks.pop() {
                Some(Block::Do { top, exit, .. }) => {
                    match cond {
                        Some((until, cond)) => self.branch(cond, !*until, &top)?,
                        None => self.jump("jmp", &top)
                    }
                    self.label(&exit);
                }
                _ => return self.fail("LOOP without DO")
            },
            Stmt::Exit(kind) => {
                let mut target = None;
                for block in self.blocks.iter().rev() {
                    match (block, kind) {
                        (Block::For { exit, .. }, ExitKind::For) | (Block::While { exit, .. }, ExitKind::While) | (Block::Do { exit, .. }, ExitKind::Do) => { target = Some(exit.clone()); break; }
                        (Block::Sub { exit, is_function, .. }, ExitKind::Sub | ExitKind::Function) => {
                            if *is_function == (*kind == ExitKind::Function) { target = Some(exit.clone()); }
                            break;
                        }
                        _ => {}
                    }
                }
                match target {
                    Some(exit) => self.jump("jmp", &exit),
                    None => return self.fail(format!("EXIT {} outside of {}", format!("{:?}", kind).to_uppercase(), format!("{:?}", kind).to_uppercase()))
                }
            }
            Stmt::Goto(target) => self.jump("jmp", &Self::target_label(target)),
            Stmt::Gosub(target) => self.jump("cal", &Self::target_label(target)),
            Stmt::On(selector, gosub, targets) => {
                // n - 1 - ... - 1 is zero at the n-th target, out of range values continue after the statement
                let after = self.new_label("on");
                let stubs: Vec<String> = targets.iter().map(|_| self.new_label("ongosub")).collect();
                self.expr(selector)?;
                self.select(ACC);
                for (target, stub) in targets.iter().zip(&stubs) {
                    self.emit(String::from("sub #1"));
                    self.jump("jpz", &if *gosub { stub.clone() } else { Self::target_label(target) });
                }
                self.jump("jmp", &after);
                if *gosub {
                    for (target, stub) in targets.iter().zip(&stubs) {
                        self.label(stub);
                        self.jump("cal", &Self::target_label(target));
                        self.jump("jmp", &after);
                    }
                }
                self.label(&after);
            }
            Stmt::Return => self.emit(String::from("ret")),
            Stmt::End => self.emit(String::from("end")),
            Stmt::Halt => self.emit(String::from("hlt")),
            Stmt::Stop => self.emit(String::from("brk")),
            Stmt::Read(targets) => self.read(targets)?,
            Stmt::Poke(address, value) => match Self::constant(address) {
                Some(address) if address >= 0 => { self.expr(value)?; self.select(ACC); self.emit(format!("sto #{}", address)); }
                _ => {
                    self.expr(value)?;
                    self.emit(String::from("psh r13"));
                    self.expr(address)?;
                    self.select(ACC);
                    self.emit(String::from("pop r12"));
                    self.select(TMP);
                    self.emit(String::from("sto r13"));
                }
            },
            Stmt::Print(items) => self.print(items)?,
            Stmt::Cls => self.cls()?,
            Stmt::Color(foreground, background) => self.color(foreground, background)?,
            Stmt::Pset { reset, point, color } => self.pset(*reset, point, color)?,
            Stmt::Line { from, to, color, kind } => self.line(from, to, color, *kind)?,
            Stmt::View(area) => self.view(area)?,
            Stmt::Restore(target) => self.restore(target)?,
            Stmt::Swap(a, b) => {
                let value = |lv: &LValue| match lv { LValue::Var(name) => Expr::Var(name.clone()), LValue::Index(name, i) => Expr::Call(name.clone(), vec![i.clone()]) };
                self.expr(&value(a))?;
                self.emit(String::from("psh r13"));
                self.assign(a, &value(b))?;
                self.emit(String::from("pop r13"));
                self.assign(b, &Expr::Acc)?;
            }
            Stmt::SubDef(name, _, is_function) => {
                if self.scope.is_some() { return self.fail("SUB and FUNCTION cannot be nested"); }
                if !self.blocks.is_empty() { return self.fail("SUB and FUNCTION cannot be inside a block"); }
                let skip = self.new_label("skip");
                let exit = self.new_label("exit");
                self.jump("jmp", &skip);
                self.label(&Self::sub_label(name));
                self.scope = Some(Scope { name: name.clone(), shared: HashSet::new(), entry: self.out.len() });
                self.blocks.push(Block::Sub { name: name.clone(), is_function: *is_function, exit, skip, line: self.line });
            }
            Stmt::EndSub(is_function) => match self.blocks.pop() {
                Some(Block::Sub { name, is_function: kind, exit, skip, .. }) if kind == *is_function => {
                    self.label(&exit);
                    self.emit(String::from("ret"));
                    self.label(&skip);
                    let scope = self.scope.take().unwrap();
                    // locals start at 0 on every call like in QBasic, parameters are set by the caller
                    let params = &self.subs[&name].0;
                    let prefix = format!("{}.", name);
                    let mut reset = vec![];
                    for (var, loc) in &self.variables {
                        let Some(local) = var.strip_prefix(&prefix) else { continue };
                        if local.starts_with('_') || params.iter().any(|p| p == local) { continue; }
                        match loc {
                            Loc::Reg(r) => { reset.push(format!("    reg r{}", r)); reset.push(String::from("    set #0")); }
                            Loc::Mem(_) => {
                                reset.push(String::from("    reg r13"));
                                reset.push(String::from("    set #0"));
                                reset.push(format!("    sto {}", var));
                            }
                        }
                    }
                    self.out.splice(scope.entry..scope.entry, reset);
                }
                _ => return self.fail(if *is_function { "END FUNCTION without FUNCTION" } else { "END SUB without SUB" })
            },
            Stmt::Req(_) => {}
            Stmt::Shared(names) => match &mut self.scope {
                Some(scope) => scope.shared.extend(names.iter().cloned()),
                None => return self.fail("SHARED outside of SUB or FUNCTION")
            },
            Stmt::DefFn(name, params, body) => {
                self.stmt(&Stmt::SubDef(name.clone(), params.clone(), true))?;
                self.assign(&LValue::Var(name.clone()), body)?;
                self.stmt(&Stmt::EndSub(true))?;
            }
            Stmt::CallSub(name, args) => self.call_sub(name, args, false)?
        }
        Ok(())
    }

    fn sub_label(name: &str) -> String {
        format!("_sub_{}", name)
    }

    fn target_label(target: &Target) -> String {
        match target {
            Target::Line(n) => Self::line_label(*n),
            Target::Label(name) => format!("_label_{}", name)
        }
    }

    /// Sets the parameters, calls the SUB or FUNCTION and loads a FUNCTION's result into the accumulator.
    fn call_sub(&mut self, name: &str, args: &[Expr], want_result: bool) -> Result<(), Error> {
        let (params, is_function) = match self.subs.get(name) {
            Some(sub) => sub.clone(),
            None => return self.fail(format!("unknown SUB {}", name))
        };
        if want_result && !is_function { return self.fail(format!("{} is a SUB without a value, only a FUNCTION can be used in an expression", name)); }
        if args.len() != params.len() { return self.fail(format!("{} expects {} argument(s), got {}", name, params.len(), args.len())); }
        for (param, arg) in params.iter().zip(args) {
            self.assign(&LValue::Var(format!("{}.{}", name, param)), arg)?;
        }
        let caller = self.scope.as_ref().map(|s| s.name.clone()).unwrap_or_default();
        self.calls.push((caller, String::from(name), self.line));
        self.jump("cal", &Self::sub_label(name));
        if want_result { self.expr(&Expr::Var(format!("{}.{}", name, name)))?; }
        Ok(())
    }

    /// SUB and FUNCTION variables are static, so a call chain must not lead back to its caller.
    fn check_recursion(&self) -> Result<(), Error> {
        for (caller, callee, line) in &self.calls {
            if caller.is_empty() { continue; }
            let mut seen = HashSet::new();
            let mut todo = vec![callee.clone()];
            while let Some(sub) = todo.pop() {
                if sub == *caller {
                    let how = if callee == caller { String::from("itself") } else { format!("itself through {}", callee) };
                    return err(*line, format!("{} calls {}, recursion is not supported because SUB and FUNCTION variables are static", caller, how));
                }
                if seen.insert(sub.clone()) { todo.extend(self.calls.iter().filter(|c| c.0 == sub).map(|c| c.1.clone())); }
            }
        }
        Ok(())
    }

    // --- program

    fn prepare(&mut self, lines: &[Line]) -> Result<(), Error> {
        fn walk(stmts: &[Stmt], dims: &mut Vec<(String, usize)>, global: &mut Vec<String>, refs: &mut Vec<Target>) {
            for stmt in stmts {
                match stmt {
                    Stmt::Dim(items, shared) => { dims.extend(items.iter().cloned()); global.extend(shared.iter().cloned()); }
                    Stmt::Goto(t) | Stmt::Gosub(t) => refs.push(t.clone()),
                    Stmt::On(_, _, targets) => refs.extend(targets.iter().cloned()),
                    Stmt::If(_, then, otherwise) => {
                        for branch in [Some(then), otherwise.as_ref()].into_iter().flatten() {
                            match branch {
                                Branch::Line(n) => refs.push(Target::Line(*n)),
                                Branch::Stmts(s) => walk(s, dims, global, refs)
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        let mut numbers = HashSet::new();
        let mut declares = vec![];
        for line in lines {
            self.line = line.src_line;
            if let Some(n) = line.number {
                if !numbers.insert(n) { return self.fail(format!("duplicate line number {}", n)); }
                self.data_lines.insert(n, self.data.len());
            }
            if let Some(label) = &line.label {
                if !self.named_labels.insert(label.clone()) { return self.fail(format!("duplicate label {}", label)); }
                self.data_labels.insert(label.clone(), self.data.len());
            }
            for stmt in &line.stmts {
                match stmt {
                    Stmt::OptionBase(base) => self.option_base = *base,
                    Stmt::Data(items) => self.data.extend(items.iter().cloned()),
                    Stmt::Declare(name, count, is_function) => declares.push((name.clone(), *count, *is_function, line.src_line)),
                    _ => {}
                }
                if let Stmt::Req(names) = stmt {
                    for name in names {
                        if !self.interrupts.iter().any(|i| i.eq_ignore_ascii_case(name)) {
                            return self.fail(format!("the program requires the {} interrupt, which is not available (available: {})", name,
                                if self.interrupts.is_empty() { String::from("none") } else { self.interrupts.join(", ") }));
                        }
                    }
                }
                if let Stmt::SubDef(name, params, _) | Stmt::DefFn(name, params, _) = stmt {
                    if BUILTINS.contains(&name.as_str()) { return self.fail(format!("{} is a built-in function", name)); }
                    if self.subs.contains_key(name) { return self.fail(format!("{} is defined twice", name)); }
                    let is_function = !matches!(stmt, Stmt::SubDef(_, _, false));
                    self.subs.insert(name.clone(), (params.clone(), is_function));
                }
            }
        }
        for (name, count, is_function, src_line) in declares {
            self.line = src_line;
            let kind = if is_function { "FUNCTION" } else { "SUB" };
            match self.subs.get(&name) {
                None => return self.fail(format!("{} {} is declared but never defined", kind, name)),
                Some((_, defined_function)) if *defined_function != is_function => return self.fail(format!("{} is declared as a {} but defined as a {}", name, kind, if is_function { "SUB" } else { "FUNCTION" })),
                Some((params, _)) if params.len() != count => return self.fail(format!("{} {} is declared with {} parameter(s) but defined with {}", kind, name, count, params.len())),
                _ => {}
            }
        }
        // the DATA items live in an array, READ walks it with a pointer, both are visible in every SUB
        self.global_shared.extend([String::from(DATA_ARRAY), String::from(DATA_POINTER)]);
        self.global_shared.extend(CONSOLE_VARS.iter().map(|v| String::from(*v)));
        let mut current: Option<String> = None;
        for line in lines {
            self.line = line.src_line;
            for stmt in &line.stmts {
                match stmt {
                    Stmt::SubDef(name, _, _) => current = Some(name.clone()),
                    Stmt::EndSub(_) => current = None,
                    _ => {}
                }
            }
            let mut dims = vec![];
            let mut refs = vec![];
            let mut global = vec![];
            walk(&line.stmts, &mut dims, &mut global, &mut refs);
            self.global_shared.extend(global);
            for (name, size) in dims {
                let name = match &current { Some(sub) => format!("{}.{}", sub, name), None => name };
                if self.arrays.iter().any(|a| a.0 == name) { return self.fail(format!("array {} is declared twice", name)); }
                self.arrays.push((name, self.next_mem, size));
                self.next_mem += size;
            }
            for target in refs {
                match target {
                    Target::Line(n) => {
                        if !numbers.contains(&n) { return self.fail(format!("line number {} does not exist", n)); }
                        self.targets.insert(n);
                    }
                    Target::Label(label) => if !self.named_labels.contains(&label) { return self.fail(format!("label {} does not exist", label)); }
                }
            }
        }
        Ok(())
    }

    /// Reserves the array for the DATA items and fills it when the program starts. Strings are stored as
    /// their index, the pointer to the next item starts at 0.
    fn init_data(&mut self) -> Result<(), Error> {
        if self.data.is_empty() { return Ok(()); }
        let (base, size) = (self.next_mem, self.data.len());
        self.arrays.push((String::from(DATA_ARRAY), base, size));
        self.next_mem += size;
        self.line = 0;
        for (i, item) in self.data.clone().iter().enumerate() {
            self.expr(&match item { DataItem::Num(n) => Expr::Num(*n), DataItem::Str(s) => Expr::Str(s.clone()) })?;
            self.select(ACC);
            self.emit(format!("sto #{}", base + i));
        }
        self.assign(&LValue::Var(String::from(DATA_POINTER)), &Expr::Num(0))
    }

    // --- console and drawing statements on the surface interrupt

    fn has_console(stmts: &[Stmt]) -> bool {
        stmts.iter().any(|stmt| match stmt {
            Stmt::Print(_) | Stmt::Cls | Stmt::Color(..) | Stmt::Pset { .. } | Stmt::Line { .. } | Stmt::View(_) => true,
            Stmt::If(_, then, otherwise) => [Some(then), otherwise.as_ref()].into_iter().flatten()
                .any(|b| matches!(b, Branch::Stmts(s) if Self::has_console(s))),
            _ => false
        })
    }

    fn init_console(&mut self) -> Result<(), Error> {
        self.line = 0;
        // white on black, the colors 15 and 0 of the VGA palette
        for (name, value) in [("_PX", 0), ("_PY", 0), ("_PCOL", crate::color::Color::vga(15)), ("_PBG", crate::color::Color::vga(0)), ("_GX", 0), ("_GY", 0)] {
            self.set(name, Expr::Num(value))?;
        }
        Ok(())
    }

    fn set(&mut self, name: &str, value: Expr) -> Result<(), Error> {
        self.assign(&LValue::Var(String::from(name)), &value)
    }

    /// Fails with a message that names the interrupt functions a statement needs when one is missing
    fn console_needs(&self, statement: &str, functions: &[&str]) -> Result<(), Error> {
        match functions.iter().find(|f| !self.functions.contains_key(**f)) {
            Some(missing) => self.fail(format!("{} needs {}, add the {} interrupt", statement, missing, missing.split('.').next().unwrap_or("surface"))),
            None => Ok(())
        }
    }

    fn var_expr(name: &str) -> Expr { Expr::Var(String::from(name)) }

    fn add(a: Expr, b: Expr) -> Expr { Expr::Bin(BinOp::Add, Box::new(a), Box::new(b)) }

    /// PRINT draws every item at the text cursor and moves it on, numbers get a blank in front and behind them like
    /// in QBasic, `,` goes to the next zone of 14 characters and a PRINT without ; or , at its end starts a new line.
    fn print(&mut self, items: &[PrintItem]) -> Result<(), Error> {
        self.console_needs("PRINT", &["surface.draw_text", "string.get_length"])?;
        if items.iter().any(|i| matches!(i, PrintItem::Comma)) { self.console_needs("PRINT with ,", &["math.div", "math.mul"])?; }
        for item in items {
            match item {
                PrintItem::Expr(expr) => {
                    let text = if Self::is_string(expr) { expr.clone() } else {
                        Self::add(Expr::Call(String::from("STR$"), vec![expr.clone()]), Expr::Str(String::from(" ")))
                    };
                    self.set("_PS$", text)?;
                    self.int_call("surface.draw_text", &[Self::var_expr("_PX"), Self::var_expr("_PY"), Expr::Num(0), Self::var_expr("_PS$")], &[])?;
                    // every character is CHAR_WIDTH wide, added up so that no multiplication is needed
                    self.set("_PW", Expr::Call(String::from("LEN"), vec![Self::var_expr("_PS$")]))?;
                    let mut x = Self::var_expr("_PX");
                    for _ in 0..CHAR_WIDTH { x = Self::add(x, Self::var_expr("_PW")); }
                    self.set("_PX", x)?;
                }
                PrintItem::Comma => {
                    let zones = Expr::Bin(BinOp::Div, Box::new(Self::var_expr("_PX")), Box::new(Expr::Num(ZONE_WIDTH)));
                    self.set("_PX", Expr::Bin(BinOp::Mul, Box::new(Self::add(zones, Expr::Num(1))), Box::new(Expr::Num(ZONE_WIDTH))))?;
                }
                PrintItem::Semicolon => {}
            }
        }
        if !matches!(items.last(), Some(PrintItem::Comma | PrintItem::Semicolon)) {
            self.set("_PX", Expr::Num(0))?;
            self.set("_PY", Self::add(Self::var_expr("_PY"), Expr::Num(LINE_HEIGHT)))?;
        }
        Ok(())
    }

    /// COLOR remembers the colors of PSET, LINE, PRESET and CLS and sets the palette entries 1 and 0 of the surface,
    /// which are the colors of text
    fn color(&mut self, foreground: &Option<Expr>, background: &Option<Expr>) -> Result<(), Error> {
        self.console_needs("COLOR", &["surface.set_color"])?;
        for (index, name, color) in [(1, "_PCOL", foreground), (0, "_PBG", background)] {
            if let Some(color) = color {
                self.set(name, color.clone())?;
                self.int_call("surface.set_color", &[Expr::Num(index), Self::var_expr(name)], &[])?;
            }
        }
        Ok(())
    }

    fn cls(&mut self) -> Result<(), Error> {
        self.console_needs("CLS", &["surface.clear"])?;
        self.int_call("surface.clear", &[Self::var_expr("_PBG")], &[])?;
        self.set("_PX", Expr::Num(0))?;
        self.set("_PY", Expr::Num(0))
    }

    /// The coordinates of a point in the globals, STEP counts from the base point
    fn point(&self, point: &Point, base: (&str, &str)) -> (Expr, Expr) {
        if point.step { (Self::add(Self::var_expr(base.0), point.x.clone()), Self::add(Self::var_expr(base.1), point.y.clone())) }
        else { (point.x.clone(), point.y.clone()) }
    }

    fn pset(&mut self, reset: bool, point: &Point, color: &Option<Expr>) -> Result<(), Error> {
        self.console_needs(if reset { "PRESET" } else { "PSET" }, &["surface.draw_pixel"])?;
        let (x, y) = self.point(point, ("_GX", "_GY"));
        self.set("_GX", x)?;
        self.set("_GY", y)?;
        let color = color.clone().unwrap_or_else(|| Self::var_expr(if reset { "_PBG" } else { "_PCOL" }));
        self.int_call("surface.draw_pixel", &[Self::var_expr("_GX"), Self::var_expr("_GY"), color], &[])
    }

    fn line(&mut self, from: &Option<Point>, to: &Point, color: &Option<Expr>, kind: LineKind) -> Result<(), Error> {
        let function = match kind { LineKind::Line => "surface.draw_line", LineKind::Box => "surface.draw_rect", LineKind::Fill => "surface.fill_rect" };
        self.console_needs("LINE", &[function])?;
        let (x1, y1) = match from { Some(p) => self.point(p, ("_GX", "_GY")), None => (Self::var_expr("_GX"), Self::var_expr("_GY")) };
        self.set("_LX", x1)?;
        self.set("_LY", y1)?;
        let (x2, y2) = self.point(to, ("_LX", "_LY"));
        self.set("_GX", x2)?;
        self.set("_GY", y2)?;
        let color = color.clone().unwrap_or_else(|| Self::var_expr("_PCOL"));
        let (lx, ly, gx, gy) = (Self::var_expr("_LX"), Self::var_expr("_LY"), Self::var_expr("_GX"), Self::var_expr("_GY"));
        if kind == LineKind::Line { return self.int_call(function, &[lx, ly, gx, gy, color], &[]); }
        // a box includes both of its corners
        let side = |a: &Expr, b: &Expr| Self::add(Expr::Call(String::from("ABS"), vec![Expr::Bin(BinOp::Sub, Box::new(a.clone()), Box::new(b.clone()))]), Expr::Num(1));
        let min = |a: &Expr, b: &Expr| Expr::Call(String::from("MIN"), vec![a.clone(), b.clone()]);
        self.int_call(function, &[min(&lx, &gx), min(&ly, &gy), side(&gx, &lx), side(&gy, &ly), color], &[])
    }

    /// VIEW (x1, y1)-(x2, y2) clips the surface, VIEW alone clears the clipping
    fn view(&mut self, area: &Option<(Point, Point)>) -> Result<(), Error> {
        self.console_needs("VIEW", &["surface.set_clip", "surface.get_size"])?;
        match area {
            Some((a, b)) => {
                let size = |from: &Expr, to: &Expr| Self::add(Expr::Bin(BinOp::Sub, Box::new(to.clone()), Box::new(from.clone())), Expr::Num(1));
                self.int_call("surface.set_clip", &[a.x.clone(), a.y.clone(), size(&a.x, &b.x), size(&a.y, &b.y)], &[])
            }
            None => {
                let (w, h) = (self.output(&LValue::Var(String::from("_PW")))?, self.output(&LValue::Var(String::from("_PH")))?);
                self.int_call("surface.get_size", &[], &[w, h])?;
                self.int_call("surface.set_clip", &[Expr::Num(0), Expr::Num(0), Self::var_expr("_PW"), Self::var_expr("_PH")], &[])
            }
        }
    }

    fn read(&mut self, targets: &[LValue]) -> Result<(), Error> {
        if self.data.is_empty() { return self.fail("READ without any DATA"); }
        if !self.functions.contains_key("math.div") { return self.fail("READ needs the math interrupt, it signals the end of the DATA with it"); }
        let (strings, numbers) = (self.data.iter().any(|d| matches!(d, DataItem::Str(_))), self.data.iter().any(|d| matches!(d, DataItem::Num(_))));
        let pointer = || Expr::Var(String::from(DATA_POINTER));
        for target in targets {
            let name = match target { LValue::Var(n) | LValue::Index(n, _) => n };
            if name.ends_with('$') && !strings { return self.fail(format!("READ {} needs a string, but the DATA has only numbers", name)); }
            if !name.ends_with('$') && !numbers { return self.fail(format!("READ {} needs a number, but the DATA has only strings", name)); }
            // reading past the last item is an error of the vm, as it is for QBasic: a division by zero in the math interrupt
            let ok = self.new_label("data");
            self.branch(&Expr::Bin(BinOp::Lt, Box::new(pointer()), Box::new(Expr::Num(self.data.len() as VMAtom))), true, &ok)?;
            self.int_call("math.div", &[Expr::Num(1), Expr::Num(0)], &[String::from("r12")])?;
            self.label(&ok);
            self.assign(target, &Expr::Call(String::from(DATA_ARRAY), vec![pointer()]))?;
            self.assign(&LValue::Var(String::from(DATA_POINTER)), &Expr::Bin(BinOp::Add, Box::new(pointer()), Box::new(Expr::Num(1))))?;
        }
        Ok(())
    }

    fn restore(&mut self, target: &Option<Target>) -> Result<(), Error> {
        let position = match target {
            None => 0,
            Some(Target::Line(n)) => match self.data_lines.get(n) { Some(p) => *p, None => return self.fail(format!("line number {} does not exist", n)) },
            Some(Target::Label(l)) => match self.data_labels.get(l) { Some(p) => *p, None => return self.fail(format!("label {} does not exist", l)) }
        };
        if self.data.is_empty() { return Ok(()); }
        self.assign(&LValue::Var(String::from(DATA_POINTER)), &Expr::Num(position as VMAtom))
    }

    fn program(mut self, name: &str, lines: &[Line]) -> Result<Output, Error> {
        self.prepare(lines)?;
        self.init_data()?;
        if lines.iter().any(|l| Self::has_console(&l.stmts)) { self.init_console()?; }

        for line in lines {
            self.line = line.src_line;
            self.out.push(format!("; {}", line.text));
            if let Some(n) = line.number {
                if self.targets.contains(&n) { self.label(&Self::line_label(n)); }
            }
            if let Some(label) = &line.label { self.label(&Self::target_label(&Target::Label(label.clone()))); }
            for stmt in &line.stmts { self.stmt(stmt)?; }
        }
        if let Some(block) = self.blocks.last() {
            return match block {
                Block::If { line, .. } => err(*line, "IF ... THEN without END IF"),
                Block::Select { line, .. } => err(*line, "SELECT CASE without END SELECT"),
                Block::For { line, var, .. } => err(*line, format!("FOR {} without NEXT", var)),
                Block::While { line, .. } => err(*line, "WHILE without WEND"),
                Block::Do { line, .. } => err(*line, "DO without LOOP"),
                Block::Sub { line, name, is_function, .. } => err(*line, format!("{} {} without END {}", if *is_function { "FUNCTION" } else { "SUB" }, name, if *is_function { "FUNCTION" } else { "SUB" }))
            };
        }
        self.check_recursion()?;
        self.line = 0;
        self.emit(String::from("end"));

        let mut listing = vec![format!("; {} - generated by the virtmach BASIC compiler", name)];
        for interrupt in self.interrupts { listing.push(format!("#req {}", interrupt)); }
        for (i, s) in self.strings.iter().enumerate() { listing.push(format!("{}   ; string #{}", Self::db_entry(s), i)); }
        if self.strings.len() + self.dynamic > u8::MAX as usize { return err(0, format!("out of string slots: {} string literals plus {} string operations, at most {} fit the string indices", self.strings.len(), self.dynamic, u8::MAX)); }
        for i in 0..self.dynamic { listing.push(format!("#def _str{} #{}   ; dynamic string of a string function call", i, self.strings.len() + i)); }
        for (var, loc) in &self.variables {
            listing.push(match loc {
                Loc::Reg(r) => format!("#def {} r{}", var, r),
                Loc::Mem(a) => format!("#def {} #{}", var, a)
            });
        }
        for (array, base, size) in &self.arrays {
            listing.push(format!("#def {} #{}   ; {}(0..{}) at memory #{}..#{}", array, base, array, size - 1, base, base + size - 1));
        }
        listing.extend(self.out);

        Ok(Output { listing: listing.join("\n") + "\n", variables: self.variables, arrays: self.arrays, strings: self.strings, labels: self.labels, jumps: self.jumps })
    }
}

/// A source is BASIC if its first non-empty line is a REM statement, optionally with a line number.
/// Other comments do not count: # and ; are comments in listings too.
pub fn is_basic(src: &str) -> bool {
    match src.lines().map(str::trim).find(|l| !l.is_empty()) {
        Some(line) => {
            let rest = line.trim_start_matches(|c: char| c.is_ascii_digit()).trim_start();
            rest.get(..3).is_some_and(|kw| kw.eq_ignore_ascii_case("REM"))
                && !rest[3..].starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_')
        }
        None => false
    }
}

pub fn compile(name: &str, src: &str, interrupts: &[String], functions: &Functions) -> Result<Output, Error> {
    let lines = parse_program(src)?;
    Codegen::new(interrupts, functions).program(name, &lines)
}


/// Builds the interrupt list and function map from the interrupt tables of interrupt_tables!,
/// numbered as they are there.
pub fn functions(tables: &[InterruptTable]) -> Result<(Vec<String>, Functions), String> {
    function_map(tables)
}


#[cfg(test)]
mod color_tests {
    use std::vec::Vec;

    #[test]
    fn color_literals_compile() {
        let src = "REM c\nA = #fff\nB = #00ff00 + #f008\nC = #12345\n";
        let r = crate::VirtMach::compile("t", src, Vec::new());
        assert!(r.is_err(), "5 digits is not a color");
        let src = "REM c\nA = #fff\nB = #00ff00\nC = #f008\nD = #0000ff00\n";
        let r = crate::VirtMach::compile("t", src, Vec::new());
        assert!(r.is_ok(), "{:?}", r.err());
    }
}

#[cfg(test)]
mod comment_tests {
    use std::{ collections::HashMap, string::String, vec::Vec };
    use super::{ compile, parse_program };

    fn listing(src: &str) -> String {
        compile("t", src, &Vec::new(), &HashMap::new()).unwrap().listing
    }

    #[test]
    fn comments_do_not_change_the_program() {
        let plain = listing("REM c\nA = 1\nB = A + 2\nHALT\n");
        let commented = listing("REM c\n# a whole line\nA = 1 ' the rest of the line\n  # indented\nB = A + 2 : ' after a colon\n10 # numbered\n20 foo: # labeled\nHALT ' old style\n# last");
        // only the label that was added, in front of the HALT, and the comments of the listing differ
        let code = |l: &&str| !l.trim_start().starts_with(';') && !l.contains("_label_FOO") && !l.contains("_line_");
        assert_eq!(commented.lines().filter(code).collect::<Vec<_>>(), plain.lines().filter(code).collect::<Vec<_>>());
    }

    #[test]
    fn hash_and_quotes_in_other_places() {
        // # is a color behind a statement or in an expression, not a comment
        assert_eq!(parse_program("REM c\nA = #fff\nB = #00ff00 + 1 ' green\n").unwrap().len(), 2);
        // a ; in a string is just text, and a ; behind a statement is no comment
        assert!(compile("t", "REM c\nA = 1 ; B = 2\n", &Vec::new(), &HashMap::new()).is_err());
        let out = compile("t", "REM c\nS$ = \"a;b # c\" ' the comment\nHALT\n", &Vec::new(), &HashMap::new()).unwrap();
        assert_eq!(out.strings, ["a;b # c"]);
        // behind THEN of a block IF
        listing("REM c\nIF 1 THEN ' yes\nA = 1\nEND IF ' done\n");
        // a comment line between the lines of a block
        listing("REM c\nFOR I = 1 TO 3 ' loop\n# in the loop\nA = I\nNEXT I\n");
        // a # behind a statement is no comment: syntax error
        assert!(compile("t", "REM c\nA = 1 # comment\n", &Vec::new(), &HashMap::new()).is_err());
    }
}
