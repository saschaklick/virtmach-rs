use core::fmt::Write;
use core::mem::size_of;

use crate::{VirtMach, VMAtom, VAtom, opcodes::OpCode, Program, Writer};

impl VirtMach <'_> {
    pub fn decompile <W: Write> (program: &Program, position: usize, mut writer: W) -> usize {
        let mut ret = 1;
        
        if position >= program.data.len() as usize {
            let _ = writer.write_str("?");
            return 0;
        }
        
        let instructions = program.data;     
        let byte = instructions[position];
        
        let reg = byte >> 4;
        let val = if position + 1 + size_of::<VMAtom>() > program.data.len() { 0 } else { instructions[position + 1 ..position + 1 + size_of::<VMAtom>()].as_ref().get_atom() };

        let mut buf = [0u8;16];
        let mut arg= Writer::from_buffer(&mut buf);
        let use_reg = |buf: &mut dyn core::fmt::Write| { let _ = buf.write_fmt(format_args!("r{}", reg)); };
        let mut use_val = |buf: &mut dyn core::fmt::Write| { ret += size_of::<VMAtom>(); let _ = buf.write_fmt(format_args!("#{}", val)); };
        let use_int = |buf: &mut dyn core::fmt::Write| { let _ = buf.write_fmt(format_args!("{}", reg)); };
        let mut use_reg_or_val = |buf: &mut dyn core::fmt::Write| { if reg == 0xf { use_val(buf); } else { use_reg(buf); } };
        
        let op = match byte & 0x0f {
            x if x == (OpCode::REG as u8) => { use_reg(&mut arg); "reg" }            
            x if x == (OpCode::SET as u8) => { use_reg_or_val(&mut arg); "set" }            
            x if x == (OpCode::LOA as u8) => { use_reg_or_val(&mut arg); "loa" }
            x if x == (OpCode::STO as u8) => { use_reg_or_val(&mut arg); "sto" }            
            x if x == (OpCode::PSH as u8) => { use_reg_or_val(&mut arg); "psh" }            
            x if x == (OpCode::POP as u8) => { use_reg_or_val(&mut arg); "pop" }                                    
            x if x == (OpCode::ADD as u8) => { use_reg_or_val(&mut arg); "add" }                                    
            x if x == (OpCode::SUB as u8) => { use_reg_or_val(&mut arg); "sub" } 
            x if x == (OpCode::CAL as u8) => { use_reg_or_val(&mut arg); "cal" }                        
            x if x == (OpCode::JMP as u8) => { use_reg_or_val(&mut arg); "jmp" }                        
            x if x == (OpCode::JPZ as u8) => { use_reg_or_val(&mut arg); "jpz" }                                    
            x if x == (OpCode::JPC as u8) => { use_reg_or_val(&mut arg); "jpc" }                        
            x if x == (OpCode::JPS as u8) => { use_reg_or_val(&mut arg); "jps" }                        
            x if x == (OpCode::INT as u8) => { use_int(&mut arg); "int" }                
            0x0f => {
                let op = byte;
                match op {                    
                    x if x == (OpCode::RET as u8) => { "ret" }     
                    x if x == (OpCode::CLR as u8) => { "clr" }                                      
                    x if x == (OpCode::INV as u8) => { "inv" }                                      
                    x if x == (OpCode::NEG as u8) => { "neg" }                                      
                    x if x == (OpCode::BRK as u8) => { "brk" }
                    x if x == (OpCode::HLT as u8) => { "hlt" }
                    x if x == (OpCode::END as u8) => { "end" }
                    _ => { "?" }
                }
            }             
            _ => { "?" }
        };        
    
        let _ = writer.write_fmt(format_args!("{:3}{}{}", op, if arg.to_str().len() > 0 { " " } else { "" }, arg.to_str()));
        position + ret
    }
}
