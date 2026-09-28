//! Compiles a small BASIC dialect into a virtmach listing, see VirtMach::compile for assembling it.
//!
//! Language summary:
//!   - optional line numbers and labels (NAME: at the start of a line), several statements per line
//!     separated by ":", REM and ' comments
//!   - [LET] var = expr, arrays via DIM A(n) (elements 0..n), A(i) = expr, SWAP a, b
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
//!   - strings in double quotes ("" for a quote) become #db entries, their value is the entry's index,
//!     which is passed to interrupt functions, e.g. surface.draw_text(0, 0, 0, "Hello")
//!   - STR$(n), MID$(s, start[, length]), LEFT$(s, n), RIGHT$(s, n) through string.format and string.substr,
//!     LEN(s) through string.get_length. Positions are 1-based. Every call site owns a dynamic string entry
//!     (after the #db entries) that is overwritten each time it runs, string arguments may be evaluated
//!     more than once. Variable names may end in $.
//!   - + joins strings through string.concat when an operand is a string: a literal, a name ending in $
//!     or another string +. Every + owns a dynamic string entry like the string functions.
//!   - operators: + - (native), * / MOD ^ << >> AND OR XOR NOT (math interrupt),
//!     = <> < > <= >= (-1 for true, 0 for false), strings compare by content through string.compare,
//!     < and > stay correct when the difference of the numbers overflows
//!   - built-in functions ABS(x), SGN(x), MIN(a, b, ...), MAX(a, b, ...), RND(low, high) (random.range)
//!   - in IF/WHILE/DO/CASE conditions AND, OR and NOT are logical and short-circuit
//!
//! Variables live in registers r0..r11, further variables and arrays in memory.
//! r12 and r13 are used for evaluating expressions.

extern crate alloc;
extern crate std;

use std::prelude::rust_2024::*;
use std::{ format, vec, collections::{ HashMap, HashSet } };
use pest::{ Parser as PestParser, error::LineColLocation, iterators::Pair, pratt_parser::{ Assoc, Op, PrattParser } };

use crate::{ VMAtom, interrupts::SoftInterruptFunction };

/// Interrupt functions by "interrupt.function" name: (interrupt no, function no, arguments, returns)
pub type Functions = HashMap<String, (u8, VMAtom, usize, usize)>;

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

#[derive(Debug, Clone, Copy, PartialEq)]
enum ExitKind { For, While, Do, Sub, Function }

#[derive(Debug)]
enum Stmt {
    Let(LValue, Expr),
    MultiAssign(Vec<LValue>, String, Vec<Expr>),
    Call(String, Vec<Expr>),
    Dim(Vec<(String, usize)>),
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
        .op(Op::infix(Rule::op_or, Assoc::Left) | Op::infix(Rule::op_xor, Assoc::Left))
        .op(Op::infix(Rule::op_and, Assoc::Left))
        .op(Op::prefix(Rule::op_not))
        .op(Op::infix(Rule::op_eq, Assoc::Left) | Op::infix(Rule::op_ne, Assoc::Left)
          | Op::infix(Rule::op_lt, Assoc::Left) | Op::infix(Rule::op_gt, Assoc::Left)
          | Op::infix(Rule::op_le, Assoc::Left) | Op::infix(Rule::op_ge, Assoc::Left))
        .op(Op::infix(Rule::op_shl, Assoc::Left) | Op::infix(Rule::op_shr, Assoc::Left))
        .op(Op::infix(Rule::op_add, Assoc::Left) | Op::infix(Rule::op_sub, Assoc::Left))
        .op(Op::infix(Rule::op_mul, Assoc::Left) | Op::infix(Rule::op_div, Assoc::Left) | Op::infix(Rule::op_mod, Assoc::Left))
        .op(Op::prefix(Rule::op_neg))
        .op(Op::infix(Rule::op_pow, Assoc::Right))
}

fn is_keyword(rule: Rule) -> bool {
    matches!(rule, Rule::kw_if | Rule::kw_then | Rule::kw_else | Rule::kw_endif | Rule::kw_end | Rule::kw_for
        | Rule::kw_to | Rule::kw_step | Rule::kw_next | Rule::kw_while | Rule::kw_wend | Rule::kw_goto
        | Rule::kw_gosub | Rule::kw_return | Rule::kw_halt | Rule::kw_dim | Rule::kw_let | Rule::kw_elseif
        | Rule::kw_select | Rule::kw_case | Rule::kw_is | Rule::kw_do | Rule::kw_loop | Rule::kw_until | Rule::kw_on
        | Rule::kw_exit | Rule::kw_stop | Rule::kw_swap | Rule::kw_sub | Rule::kw_function | Rule::kw_call
        | Rule::kw_shared | Rule::kw_def | Rule::kw_req)
}

fn parse_number(text: &str, line: usize) -> Result<VMAtom, Error> {
    let bits = size_of::<VMAtom>() * 8;
    if text.len() > 2 && text[..2].eq_ignore_ascii_case("0x") {
        match u64::from_str_radix(&text[2..], 16) {
            Ok(v) if v < (1u64 << bits) => Ok(v as VMAtom),
            _ => err(line, format!("hex number {} does not fit into {} bits", text, bits))
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

struct Builder {
    pratt: PrattParser<Rule>
}

impl Builder {
    fn stmt(&self, pair: Pair<Rule>, line: usize) -> Result<Stmt, Error> {
        let rule = pair.as_rule();
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
            Rule::dim_stmt => Stmt::Dim(it.map(|item| {
                let mut parts = item.into_inner();
                let name = parts.next().unwrap().as_str().to_uppercase();
                let size = parse_number(parts.next().unwrap().as_str(), line)?;
                if size < 0 { return err(line, format!("illegal size for array {}", name)); }
                Ok((name, size as usize + 1))
            }).collect::<Result<Vec<_>, _>>()?),
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
                let name = it.next().unwrap().as_str().to_uppercase();
                let params = it.next().map(Self::names).unwrap_or_default();
                Stmt::SubDef(name, params, rule == Rule::function_def)
            }
            Rule::end_sub => Stmt::EndSub(false),
            Rule::end_function => Stmt::EndSub(true),
            Rule::shared_stmt => Stmt::Shared(it.map(|p| p.as_str().to_uppercase()).collect()),
            Rule::req_stmt => Stmt::Req(it.map(|p| String::from(p.as_str())).collect()),
            Rule::def_fn => {
                let name = it.next().unwrap().as_str().to_uppercase();
                let mut parts: Vec<Pair<Rule>> = it.collect();
                let body = self.expr(parts.pop().unwrap(), line)?;
                Stmt::DefFn(name, parts.pop().map(Self::names).unwrap_or_default(), body)
            }
            Rule::call_sub | Rule::sub_call => {
                let name = it.next().unwrap().as_str().to_uppercase();
                Stmt::CallSub(name, self.args(it.next(), line)?)
            }
            Rule::end_if => Stmt::EndIf,
            Rule::for_stmt => {
                let var = it.next().unwrap().as_str().to_uppercase();
                let start = self.expr(it.next().unwrap(), line)?;
                let limit = self.expr(it.next().unwrap(), line)?;
                let step = it.next().map(|p| self.expr(p, line)).transpose()?;
                Stmt::For(var, start, limit, step)
            }
            Rule::next_stmt => Stmt::Next(it.next().map(|p| p.as_str().to_uppercase())),
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

    fn names(pair: Pair<Rule>) -> Vec<String> {
        pair.into_inner().map(|p| p.as_str().to_uppercase()).collect()
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
        let name = parts.next().unwrap().as_str().to_uppercase();
        match parts.next() {
            Some(index) => Ok(LValue::Index(name, self.expr(index, line)?)),
            None => Ok(LValue::Var(name))
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
                let op = match op.as_rule() {
                    Rule::op_add => BinOp::Add, Rule::op_sub => BinOp::Sub, Rule::op_mul => BinOp::Mul,
                    Rule::op_div => BinOp::Div, Rule::op_mod => BinOp::Mod, Rule::op_pow => BinOp::Pow,
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
            Rule::string => { let s = pair.as_str(); Ok(Expr::Str(s[1..s.len() - 1].replace("\"\"", "\""))) }
            Rule::ident => Ok(Expr::Var(pair.as_str().to_uppercase())),
            Rule::call => {
                let mut parts = pair.into_inner();
                let name = parts.next().unwrap().as_str().to_uppercase();
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

fn parse_program(src: &str) -> Result<Vec<Line>, Error> {
    let mut pairs = BasicParser::parse(Rule::program, src).map_err(|e| {
        let line = match e.line_col { LineColLocation::Pos((l, _)) | LineColLocation::Span((l, _), _) => l };
        Error { line, msg: format!("syntax error\n{}", e) }
    })?;

    let builder = Builder { pratt: pratt() };
    let mut lines = vec![];

    for pair in pairs.next().unwrap().into_inner() {
        if pair.as_rule() != Rule::line { continue; }
        let src_line = pair.line_col().0;
        let text = pair.as_str().trim().to_string();
        let mut number = None;
        let mut label = None;
        let mut stmts = vec![];
        for part in pair.into_inner() {
            match part.as_rule() {
                Rule::line_no => number = Some(parse_line_ref(part, src_line)?),
                Rule::label_def => label = Some(part.as_str().trim_end_matches(':').to_uppercase()),
                Rule::comment => {}
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

const BUILTINS: [&str; 10] = ["ABS", "SGN", "MIN", "MAX", "RND", "LEN", "STR$", "MID$", "LEFT$", "RIGHT$"];

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
            blocks: vec![], targets: HashSet::new(), named_labels: HashSet::new(), subs: HashMap::new(), scope: None,
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
            Some(scope) if !name.contains('.') && !scope.shared.contains(name) => format!("{}.{}", scope.name, name),
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
        if self.strings.len() > u8::MAX as usize - 1 { return self.fail(format!("too many different strings, the maximum is {}", u8::MAX)); }
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
                if !self.functions.contains_key("random.range") { return self.fail("RND needs random.range from the random interrupt, add random to the interrupts"); }
                self.expr(&Expr::IntCall(String::from("random.range"), args.to_vec()))
            }
            _ if self.subs.contains_key(name) => self.call_sub(name, args, true),
            "STR$" => {
                if args.len() != 1 { return self.fail("STR$ takes exactly one argument"); }
                let slot = self.string_function("STR$", "format", 2)?;
                self.push_arg(&args[0])?;
                self.emit(format!("psh {}", slot));
                self.string_result("format", &slot)
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
        if self.strings.len() + self.dynamic >= u8::MAX as usize { return self.fail(format!("too many strings and string operations, at most {} fit the string indices", u8::MAX)); }
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
            if i < 0 || i as usize >= size { return self.fail(format!("index {} is out of bounds for {}(0..{})", i, name, size - 1)); }
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
                    if i < 0 || i as usize >= size { return self.fail(format!("index {} is out of bounds for {}(0..{})", i, name, size - 1)); }
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
                (Some((base, size)), Some(i)) if i >= 0 && (i as usize) < size => Ok(format!("#{}", base + i as usize)),
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
            Stmt::Dim(_) => {}
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
        fn walk(stmts: &[Stmt], dims: &mut Vec<(String, usize)>, refs: &mut Vec<Target>) {
            for stmt in stmts {
                match stmt {
                    Stmt::Dim(items) => dims.extend(items.iter().cloned()),
                    Stmt::Goto(t) | Stmt::Gosub(t) => refs.push(t.clone()),
                    Stmt::On(_, _, targets) => refs.extend(targets.iter().cloned()),
                    Stmt::If(_, then, otherwise) => {
                        for branch in [Some(then), otherwise.as_ref()].into_iter().flatten() {
                            match branch {
                                Branch::Line(n) => refs.push(Target::Line(*n)),
                                Branch::Stmts(s) => walk(s, dims, refs)
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        let mut numbers = HashSet::new();
        for line in lines {
            self.line = line.src_line;
            if let Some(n) = line.number {
                if !numbers.insert(n) { return self.fail(format!("duplicate line number {}", n)); }
            }
            if let Some(label) = &line.label {
                if !self.named_labels.insert(label.clone()) { return self.fail(format!("duplicate label {}", label)); }
            }
            for stmt in &line.stmts {
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
            walk(&line.stmts, &mut dims, &mut refs);
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

    fn program(mut self, name: &str, lines: &[Line]) -> Result<Output, Error> {
        self.prepare(lines)?;

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
        if self.strings.len() + self.dynamic > u8::MAX as usize { return err(0, "too many strings and string function calls"); }
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


/// Builds the interrupt list and function map from the built-in interrupt tables, in interrupt number order.
pub fn functions(tables: &[(&str, &[SoftInterruptFunction])]) -> (Vec<String>, Functions) {
    let mut functions = Functions::new();
    for (int_no, (name, table)) in tables.iter().enumerate() {
        for function in table.iter() {
            functions.insert(format!("{}.{}", name, function.name), (int_no as u8, function.no, function.arguments, function.returns));
        }
    }
    (tables.iter().map(|t| String::from(t.0)).collect(), functions)
}
