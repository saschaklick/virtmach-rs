extern crate alloc;
extern crate std;

use alloc::alloc::{alloc_zeroed, Layout};
use bytes::{BufMut, BytesMut};
use std::{collections::HashMap, vec::Vec, slice, string::String};
use std::{format};
use num_literal_traits::NumLiteralTrait;

use crate::{ATOM_ID, Program, VAtomMut, VMAtom, VirtMach, interrupts::{ SoftInterruptFunction }, opcodes::OpCode};

#[derive(Debug)]
pub enum ListingError <'a> {
    NoError,    
    IllegalOp(usize, &'a str),
    IllegalArgument(usize, &'a str),
    IllegalRegister(usize, &'a str),    
    IllegalInterrupt(usize, &'a str),    
    MalformedDefine(usize, &'a str),   
    MalformedLiteral(usize, &'a str),   
    IllegalDefineValue(usize, &'a str),    
    UnknownLabel(usize, &'a str),    
    UnknownInterrupt(usize, &'a str),
    UnknownFunction(usize, &'a str),
    MalformedFunction(usize, &'a str),
    /// BASIC compile error: source line (0 if it has none) and message
    #[cfg(feature = "basic")]
    Basic(usize, String)
}

struct Label <'a> {
    name: &'a str,
    address: usize,
}

struct Jump <'a> {
    label: &'a str,
    address: usize,
    line_no: usize
}

#[derive(Debug, PartialEq)]
pub enum Argument <'a> {
    Empty(),
    Error(&'a str),    
    Ignore(),
    Register(u8),
    Atom(VMAtom),
    Label(&'a str)
}

impl VirtMach <'_> {
    
    fn parse_argument <'a> (mut arg: &'a str, defines: &HashMap::<&'a str, &'a str>) -> Argument<'a> {
        defines.get(arg).inspect(|value|{ arg = value; });
        if arg.is_empty() { Argument::Empty() } else
        if arg == "_" { Argument::Ignore() } else { match &arg[0..1] {
            "r" => match arg[1..].parse::<u8>() {
                Ok(r) => match r { 
                    0 .. 14 => Argument::Register(r),
                    _ => Argument::Error("malformed register") 
                }
                _ => Argument::Label(arg)
            }                                                  
            "#" => match &arg[1..] {
                "min" => Argument::Atom(VMAtom::MIN),
                "max" => Argument::Atom(VMAtom::MAX),
                _ => match arg[1..].parse::<VMAtom>() {
                    Ok(a) => Argument::Atom(a),
                    _ => Argument::Error("malformed value")                                        
                }
            }
            _ => Argument::Label(arg)
        } }
    }

    /// The functions of the interrupts, numbered as interrupts::interrupt_numbers does: built-in
    /// interrupts by their INDEX, others after them in the given order
    fn prepare_function_map(interrupts: Vec::<(&str, &[SoftInterruptFunction])>) -> Result<HashMap::<String, (u8, VMAtom, usize, usize)>, String> {
        let mut map: HashMap::<String, (u8, VMAtom, usize, usize)> = HashMap::new();
        let names: Vec<&str> = interrupts.iter().map(|i| i.0).collect();
        
        for (interrupt, int_no) in interrupts.iter().zip(crate::interrupts::interrupt_numbers(&names)?) {                        
            map.insert(String::from(interrupt.0), (int_no, 0, 0, 0));
            for function in interrupt.1 {
                map.insert(format!("{}.{}", interrupt.0, function.name), (int_no, function.no, function.arguments, function.returns));
            }
        }

        return Ok(map);
    }

    pub fn compile <'a> (name: &'a str, listing: &'a str, functions: Vec::<(&str, &[SoftInterruptFunction])>) -> Result<( Program<'a>, *const u8 ), ListingError<'a>> { 
        let interrupts = functions.clone().into_iter().map(|a| { String::from(a.0) }).collect::<Vec<String>>();        
        let map = VirtMach::prepare_function_map(functions).map_err(|_| ListingError::IllegalInterrupt(0, "interrupt numbers used twice or too many interrupts"))?;
        VirtMach::compile_owned(name, listing, &interrupts, map)
    }                                       
    
    /// Assembles a listing. With the basic feature, a source whose first non-empty line is a REM statement
    /// is compiled from BASIC into a listing first, see crate::basic.
    pub fn compile_owned <'a> (name: &'a str, listing: &'a str, interrupts: &[String], functions: HashMap::<String, (u8, VMAtom, usize, usize)>) -> Result<( Program<'a>, *const u8 ), ListingError<'a>> {
        #[cfg(feature = "basic")]
        if crate::basic::is_basic(listing) {
            let output = crate::basic::compile(name, listing, interrupts, &functions).map_err(|e| ListingError::Basic(e.line, e.msg))?;
            return VirtMach::assemble(name, &output.listing, interrupts, functions)
                .map_err(|e| ListingError::Basic(0, format!("generated listing failed to assemble: {:?}", e)));
        }
        VirtMach::assemble(name, listing, interrupts, functions)
    }

    fn assemble <'a, 'b> (name: &'a str, listing: &'b str, interrupts: &[String], functions: HashMap::<String, (u8, VMAtom, usize, usize)>) -> Result<( Program<'a>, *const u8 ), ListingError<'b>> {
        let mut data = BytesMut::new();         
        
        data.put_u8(ATOM_ID);               

        let mut len = 0;

        let mut labels: Vec<Label> = Vec::new();
        let mut jumps: Vec<Jump> = Vec::new();
                  
        let mut defines = HashMap::<&str, &str>::new();
        let mut dbs = Vec::new();
                
        for (i, mut line) in listing.lines().enumerate() {   
            let line_no = i + 1;
            line = line.trim();
            line = line.split(";").next().unwrap_or("");
            if line.starts_with("#") {                                                
                let def: Vec<&str> = line[1..].split(" ").filter(|a| !a.is_empty() ).collect();
                if def.len() > 0 {
                    match def[0] {
                        "def" => {
                            if def.len() == 3 {
                                let key = def[1].trim();
                                let value = def[2].trim();                    
                                if key.starts_with("r") || key.starts_with("#") {
                                    return Err(ListingError::IllegalDefineValue(line_no, def[2]));                    
                                }
                                defines.insert(key, value);
                                log::info!("#def {} = {}", key, value);
                            }else{
                                return Err(ListingError::MalformedDefine(line_no, "malformed def"));
                            }  
                        }
                        "req" => {
                            if def.len() == 2 {
                                let int_name = def[1].trim();                                 
                                if !interrupts.contains(&String::from(int_name)) { return Err(ListingError::MalformedDefine(line_no, "required interrupt not found")); }
                            }else{
                                return Err(ListingError::MalformedDefine(line_no, "malformed req"));
                            }  
                        }
                        "db" => {                            
                            let mut db = BytesMut::new();
                            let mut chunks = line[line.find(" ").unwrap_or(3)..].trim();
                            loop {
                                let char = chunks.chars().collect::<Vec<_>>()[0];                                
                                if char == ' ' {
                                    chunks = &chunks[1..];                                    
                                    continue;
                                }
                                if char == '"' {
                                    chunks = &chunks[1..];
                                    let string = &chunks[0..chunks.find('"').unwrap_or(chunks.len())];
                                    for byte in string.bytes() {
                                        db.put_u8(byte);
                                    }
                                    chunks = &chunks[string.len() + 1..];                                    
                                }else
                                if char == ',' {                                    
                                    chunks = &chunks[1..];
                                    let literal = &chunks[0..chunks.find(',').unwrap_or(chunks.len())];
                                    let value = u8::parse_literal(literal);
                                    if value.is_ok() {
                                        db.put_u8(value.unwrap());
                                    } else {
                                        return Err(ListingError::MalformedLiteral(line_no, "illegal literal"));
                                    }
                                    chunks = &chunks[literal.len()..];                             
                                }
                                if chunks.trim().is_empty() {
                                    break;
                                }                                
                            }                            
                            dbs.push(db);
                        }
                        _ => { return Err(ListingError::MalformedDefine(line_no, "illegal keyword")); }                   
                    }                                    
                }else{
                    return Err(ListingError::MalformedDefine(line_no, "empty"));
                }                
            }            
        }

        let mut db_pos = 0u16;
        data.put_u8(dbs.len() as u8);        
        for db in &dbs {
            db_pos += db.len() as u16;
            data.put_u16_ne(db_pos);
        }        
        for db in &dbs {
            for byte in db {
               data.put_u8(*byte);
            }
        }
        let db_end = data.len();          
        
        for (i, mut line) in listing.lines().enumerate() {                 
            let line_no = i + 1;
            line = line.trim();            
            line = line.split(";").next().unwrap_or("").trim();            

            if line.starts_with("#") {
                continue;
            }else
            if line.ends_with(":") {
                let label = line.split(":").next().unwrap_or("").trim();
                labels.push(Label { name: label, address: len });
            }else
            if line.contains("(") && line.ends_with(")") {
                let mut head = line[..line.find("(").unwrap()].split("=");
                let inputs_str = &line[line.find("(").unwrap() + 1 ..line.len() - 1].trim();                
                let mut function_str= head.next().unwrap().trim();
                let mut outputs_str = None;
                head.next().inspect(|a| { outputs_str = Some(function_str); function_str = a.trim();  });
                                
                let mut outputs: Vec<Argument> = if outputs_str.is_some() { outputs_str.unwrap().split(",").map(|a| VirtMach::parse_argument(a.trim(), &defines)).collect() } else { Vec::new() };
                let mut inputs: Vec<Argument> = inputs_str.split(",").map(|a| VirtMach::parse_argument(a.trim(), &defines)).collect();
                if inputs.len() == 1 { match inputs[0] { Argument::Empty() => { inputs.clear(); }, _ => {} } }

                let ignore_inputs = inputs.len() == 1 && inputs[0] == Argument::Ignore();
                let ignore_outputs = outputs.len() == 1 && outputs[0] == Argument::Ignore();
                
                let mut function: Option<(u8, VMAtom)> = None;
                if functions.contains_key(function_str) {
                    let func = functions.get(function_str).unwrap();
                    if !ignore_inputs && func.2 != inputs.len() { std::println!("{:?}", inputs); return Err(ListingError::MalformedFunction(line_no, "wrong number of arguments")); }
                    if !ignore_outputs && func.3 != outputs.len() { return Err(ListingError::MalformedFunction(line_no, "wrong number of return values")); }
                    function = Some((func.0, func.1));
                }                                

                if function.is_none() { return Err(ListingError::UnknownFunction(line_no, function_str)); }
                if !ignore_outputs {
                    for output in &outputs { match output {
                        Argument::Empty() => { return Err(ListingError::MalformedFunction(line_no, "empty output argument")); },
                        Argument::Error(_) => { return Err(ListingError::MalformedFunction(line_no, "malformed output argument")); }
                        Argument::Label(_) => { return Err(ListingError::MalformedFunction(line_no, "did not expect a label as output")); }
                        Argument::Ignore() => { return Err(ListingError::MalformedFunction(line_no, "cannot ignore a single output")); }
                        _ => {}
                    } }
                }
                if !ignore_inputs {
                    for input in &inputs { match input {
                        Argument::Empty() => { return Err(ListingError::MalformedFunction(line_no, "empty input argument")); },
                        Argument::Error(_) => { return Err(ListingError::MalformedFunction(line_no, "malformed input argument")); }
                        Argument::Label(_) => { return Err(ListingError::MalformedFunction(line_no, "did not expect a label as input")); }
                        Argument::Ignore() => { return Err(ListingError::MalformedFunction(line_no, "cannot ignore a single input")); }
                        _ => {}
                    } }
                }
                
                if !ignore_inputs {
                    inputs.reverse();
                    for input in &inputs {
                            match input {
                                Argument::Register(reg) => {
                                    data.put_u8(OpCode::PSH as u8 | (*reg << 4));
                                    len += 1;
                                }
                                Argument::Atom(val) => {
                                    data.put_u8(OpCode::PSH as u8 | (0x0f << 4));
                                    data.put_atom(*val);
                                    len += 1 + size_of::<VMAtom>();
                                }
                                _ => {}
                            }            
                    }
                }

                data.put_u8(OpCode::PSH as u8 | (0x0f << 4));
                data.put_atom(function.unwrap().1 as VMAtom);
                len += 1 + size_of::<VMAtom>();
                data.put_u8(OpCode::INT as u8 | (function.unwrap().0 << 4));
                len += 1;

                if !ignore_outputs {
                    outputs.reverse();
                    for output in &outputs {
                            match output {
                                Argument::Register(reg) => {
                                    data.put_u8(OpCode::POP as u8 | (*reg << 4));
                                    len += 1;
                                }
                                Argument::Atom(val) => {
                                    data.put_u8(OpCode::POP as u8 | (0x0f << 4));
                                    data.put_atom(*val);
                                    len += 1 + size_of::<VMAtom>();
                                }
                                _ => {}
                            }            
                    }
                }
            }else{
                let mut instruction = line.split(" ");
                let op = instruction.next().unwrap_or("").trim();
                let mut arg = instruction.next().unwrap_or("").trim();
                while arg.len() == 0 {
                    let next = instruction.next();
                    if next.is_none() { break; }
                    arg = next.unwrap_or("").trim();
                }
                
                let argument = VirtMach::parse_argument(arg, &defines);

                let mut args: u8 = 0b011;
                let mut range = VMAtom::MIN..=VMAtom::MAX;
                let op_res = match op.to_ascii_lowercase().as_str() {                    
                    "reg" => { args = 0b001; OpCode::REG }
                    "set" => { OpCode::SET }
                    "loa" => { OpCode::LOA }
                    "sto" => { OpCode::STO }                    
                    "psh" => { OpCode::PSH }
                    "pop" => { OpCode::POP }                                        
                    "add" => { OpCode::ADD }                                        
                    "sub" => { OpCode::SUB }                    
                    "cal" => { args = 0b110; OpCode::CAL }                                        
                    "int" => { args = 0b110; range = 0..=14; OpCode::INT }                                        
                    "jmp" => { args = 0b110; OpCode::JMP }                                        
                    "jpz" => { args = 0b110; OpCode::JPZ }                                                            
                    "jpc" => { args = 0b110; OpCode::JPC }                                        
                    "jps" => { args = 0b110; OpCode::JPS }                                        
                    "ret" => { OpCode::RET }    
                    "clr" => { OpCode::CLR }                    
                    "inv" => { OpCode::INV }                    
                    "neg" => { OpCode::NEG }                    
                    "brk" => { OpCode::BRK }
                    "hlt" => { OpCode::HLT }
                    "end" => { OpCode::END }                    
                    "" => { continue; }
                    _ => { return Err(ListingError::IllegalOp(line_no, op)) }
                }; 
                let op_u8 = op_res as u8;

                if op_u8 & 0x0f == 0x0f { args = 0b000; }

                match argument {
                    Argument::Register(reg) => if args & 0b001 == 0 {
                        return Err(ListingError::IllegalArgument(line_no, "did not expect a register"))                        
                    } else {
                        data.put_u8(op_u8 | (reg << 4));
                        len += 1;
                    },
                    Argument::Atom(num) => if args & 0b010 == 0 {
                        return Err(ListingError::IllegalArgument(line_no, "did not expect a number"))                        
                    } else {
                        if range.contains(&num) {
                            match op_res {
                                OpCode::INT => {
                                    data.put_u8(op_u8 | (num << 4) as u8);                                    
                                    len += 1;
                                }
                                _ => {
                                    data.put_u8(op_u8 | 0xf0);
                                    data.put_atom(num);                                       
                                    len += 1 + size_of::<VMAtom>();
                                }
                            }
                        } else {
                            return Err(ListingError::IllegalArgument(line_no, "out of range"));
                        }
                    },
                    Argument::Label(label) => if args & 0b100 == 0 {
                        return Err(ListingError::IllegalArgument(line_no, "did not expect a label"))                        
                    } else {
                        match op_res {
                            OpCode::INT => {
                                if interrupts.contains(&String::from(label)) {
                                    data.put_u8(op_u8 | (functions.get(label).unwrap().0 << 4) as u8);                                    
                                    len += 1;
                                }else{
                                    let int_no: u8 = str::parse(label).unwrap_or(255);                                    
                                    // a number must belong to one of the interrupts
                                    if interrupts.iter().any(|i| functions.get(i).is_some_and(|f| f.0 == int_no)) {
                                        data.put_u8(op_u8 | (int_no << 4) as u8);                                    
                                        len += 1;
                                    }else{
                                        return Err(ListingError::UnknownInterrupt(line_no, label));
                                    }
                                }                                                               
                            }
                            _ => {
                                data.put_u8(op_u8 | 0xf0);
                                data.put_atom(0);
                                len += 1 + size_of::<VMAtom>();
                                jumps.push(Jump { label, address: len, line_no });
                            }
                        }                        
                    },
                    Argument::Empty() => if args == 0b000 {
                        data.put_u8(op_u8);
                        len += 1;
                    } else {
                        return Err(ListingError::IllegalArgument(line_no, "missing argument"))
                    },
                    Argument::Ignore() => return Err(ListingError::IllegalArgument(line_no, "unexpected _ argument")),
                    Argument::Error(err) => return Err(ListingError::IllegalArgument(line_no, err))
                    
                }                                
            }            
        }      
        
        for jump in &jumps {
            match labels.iter().find(|label| label.name == jump.label) {
                Some(label) => {
                    let diff = label.address as VMAtom - jump.address as VMAtom;
                    data[db_end + jump.address - size_of::<VMAtom>()..].as_mut().put_atom(diff);
                }
                None => return Err(ListingError::UnknownLabel(jump.line_no, jump.label))
            }
        }                        

        let buf = unsafe { alloc_zeroed(Layout::from_size_align( data.len(), 1).unwrap()) };        
        unsafe { buf.copy_from(data.as_ptr(), data.len()); }        

        return Ok((Program {
                source: 254,
                id: name,                
                data: unsafe { slice::from_raw_parts(buf, data.len()) }
            }, buf ));
    }   

    
    pub fn functions(&self, interrupts: Vec::<(&str, &[SoftInterruptFunction])>, writer: &mut dyn core::fmt::Write) -> core::fmt::Result {                                        
        writer.write_str("name,int_no,func_no,arguments,returns,description\r\n").expect("");
        let names: Vec<&str> = interrupts.iter().map(|i| i.0).collect();
        let numbers = crate::interrupts::interrupt_numbers(&names).map_err(|_| core::fmt::Error)?;
        for (interrupt, int_no) in interrupts.iter().zip(numbers) {                                                
            for function in interrupt.1 {
                writer.write_fmt(format_args!("{}.{},{},{},{},{},\"{}\"\r\n", interrupt.0, function.name, int_no, function.no, function.arguments, function.returns, function.help)).expect("");                
            }
        }

        Ok(())
    }    
}