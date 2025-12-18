extern crate alloc;
extern crate std;

use alloc::alloc::{alloc_zeroed, Layout};
use bytes::{BufMut, BytesMut};
use std::{collections::HashMap, vec::Vec, slice};

use crate::{ATOM_ID, opcodes::OpCode, Program, VAtomMut, VMAtom, VirtMach, interrupts::{SoftInterruptDef}};

#[derive(Debug)]
pub enum ListingError <'a> {
    NoError,    
    IllegalOp(usize, &'a str),
    IllegalArgument(usize, &'a str),
    IllegalRegister(usize, &'a str),    
    IllegalInterrupt(usize, &'a str),    
    MalformedDefine(usize, &'a str),    
    IllegalDefineValue(usize, &'a str),    
    UnknownLabel(usize, &'a str),    
    UnknownInterrupt(usize, &'a str),
    UnknownFunction(usize, &'a str),
    MalformedFunction(usize, &'a str),    
}

#[derive(Copy, Clone)]
struct Label <'a> {
    name: &'a str,
    address: usize,
}

#[derive(Copy, Clone)]
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
    
    pub fn parse_argument <'a> (mut arg: &'a str, defines: &HashMap::<&'a str, &'a str>) -> Argument<'a> {
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
    
    pub fn compile <'a> (name: &'a str, listing: &'a str, interrupts: &[&SoftInterruptDef]) -> Result<( Program<'a>, *const u8 ), ListingError<'a>> {        
        let mut dest = BytesMut::new(); 
        
        dest.put_u8(ATOM_ID);               

        let mut len = 0;

        let mut labels = [Label { name: "", address: 0 };128];
        let mut labels_i = 0usize;
        let mut jumps = [Jump { label: "", address: 0, line_no: 0 };128];
        let mut jumps_i = 0usize;
                  
        let mut defines = HashMap::<&str, &str>::new();        
                
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
                                let mut found = false;
                                for int_def in interrupts {
                                    if int_name == int_def.name { found = true; break; }
                                }
                                if found == false { return Err(ListingError::MalformedDefine(line_no, "required interrupt not found")); }
                            }else{
                                return Err(ListingError::MalformedDefine(line_no, "malformed req"));
                            }  
                        }
                        _ => { return Err(ListingError::MalformedDefine(line_no, "illegal keyword")); }                   
                    }                                    
                }else{
                    return Err(ListingError::MalformedDefine(line_no, "empty"));
                }                
            }            
        }
        
        for (i, mut line) in listing.lines().enumerate() {                 
            let line_no = i + 1;
            line = line.trim();            
            line = line.split(";").next().unwrap_or("").trim();

            if line.starts_with("#") {
                continue;
            }else
            if line.ends_with(":") {
                let label = line.split(":").next().unwrap_or("").trim();
                labels[labels_i].name = label;
                labels[labels_i].address = len;  
                labels_i += 1;                             
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
                                
                let mut function: Option<[u8;2]> = None;
                for (int_no, int_def) in interrupts.iter().enumerate() {                    
                    for int_func in int_def.functions {
                        let names = [int_func.name, &[int_def.name, int_func.name].join(".")];
                        if names.contains(&function_str) {
                            if !ignore_inputs && int_func.arguments != inputs.len() { std::println!("{:?}", inputs); return Err(ListingError::MalformedFunction(line_no, "wrong number of arguments")); }
                            if !ignore_outputs && int_func.returns != outputs.len() { return Err(ListingError::MalformedFunction(line_no, "wrong number of return values")); }
                            function = Some([int_no as u8, int_func.no as u8]);
                        }
                    }
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
                                    dest.put_u8(OpCode::PSH as u8 | (*reg << 4));
                                    len += 1;
                                }
                                Argument::Atom(val) => {
                                    dest.put_u8(OpCode::PSH as u8 | (0x0f << 4));
                                    dest.put_atom(*val);
                                    len += 1 + size_of::<VMAtom>();
                                }
                                _ => {}
                            }            
                    }
                }

                dest.put_u8(OpCode::PSH as u8 | (0x0f << 4));
                dest.put_atom(function.unwrap()[1] as VMAtom);
                len += 1 + size_of::<VMAtom>();
                dest.put_u8(OpCode::INT as u8 | (function.unwrap()[0] << 4));
                len += 1;

                if !ignore_outputs {
                    outputs.reverse();
                    for output in &outputs {
                            match output {
                                Argument::Register(reg) => {
                                    dest.put_u8(OpCode::POP as u8 | (*reg << 4));
                                    len += 1;
                                }
                                Argument::Atom(val) => {
                                    dest.put_u8(OpCode::POP as u8 | (0x0f << 4));
                                    dest.put_atom(*val);
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
                let mut range = VMAtom::MIN..VMAtom::MAX;
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
                    "int" => { args = 0b110; range = 0..15; OpCode::INT }                                        
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
                        dest.put_u8(op_u8 | (reg << 4));
                        len += 1;
                    },
                    Argument::Atom(num) => if args & 0b010 == 0 {
                        return Err(ListingError::IllegalArgument(line_no, "did not expect a number"))                        
                    } else {
                        if range.contains(&num) {
                            match op_res {
                                OpCode::INT => {
                                    dest.put_u8(op_u8 | (num << 4) as u8);                                    
                                    len += 1;
                                }
                                _ => {
                                    dest.put_u8(op_u8 | 0xf0);
                                    dest.put_atom(num);                                       
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
                                let mut num = None;
                                for (i, def) in interrupts.iter().enumerate() {                                    
                                    if def.name == label {
                                        num = Some(i as u8);
                                    }
                                }
                                if num.is_some() {
                                    dest.put_u8(op_u8 | (num.unwrap() << 4) as u8);                                    
                                    len += 1;
                                }else{
                                    return Err(ListingError::UnknownInterrupt(line_no, label));
                                }
                            }
                            _ => {
                                dest.put_u8(op_u8 | 0xf0);
                                dest.put_atom(0);
                                len += 1 + size_of::<VMAtom>();
                                jumps[jumps_i].label = label;
                                jumps[jumps_i].address = len;   
                                jumps_i += 1;     
                            }
                        }                        
                    },
                    Argument::Empty() => if args == 0b000 {
                        dest.put_u8(op_u8);
                        len += 1;
                    } else {
                        return Err(ListingError::IllegalArgument(line_no, "missing argument"))
                    },
                    Argument::Ignore() => return Err(ListingError::IllegalArgument(line_no, "unexpected _ argument")),
                    Argument::Error(err) => return Err(ListingError::IllegalArgument(line_no, err))
                    
                }                                
            }            
        }      
        
        for jump in jumps {
            if jump.label.len() == 0 { break; }
            for label in labels {
                if label.name.len() == 0 { return Err(ListingError::UnknownLabel(jump.line_no, jump.label)); }
                if label.name == jump.label {
                    let diff = label.address as VMAtom - jump.address as VMAtom;                    
                    dest[1 + jump.address - size_of::<VMAtom>()..].as_mut().put_atom(diff);
                    break; 
                }
            }
        }                        

        let buf = unsafe { alloc_zeroed(Layout::from_size_align( dest.len(), 1).unwrap()) };        
        unsafe { buf.copy_from(dest.as_ptr(), dest.len()); }

        return Ok((Program {
                source: 254,
                id: name,
                data: unsafe { slice::from_raw_parts(buf, dest.len()) }
            }, buf ));
    }        
}