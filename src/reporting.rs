use core::fmt::Write;
use crate::{RuntimeError, VirtMach, Writer, MEM_SIZE, REG_MAX};

impl VirtMach <'_> {
    pub fn log(&self) {
        if self.program.data.len() == 0 {
            log::error!("[{:5?}] no program loaded", self.state);
            return;
        }
        if self.processor.prog_cnt > self.program.data.len() {
            log::error!("[{:5?}] program out of bounds", self.state);
            return;
        }
        
        let mut buf = [0u8;16];
        let mut writer = Writer::from_buffer(&mut buf);
        VirtMach::decompile(&self.program, self.processor.prog_cnt, &mut writer);        
        
        log::info!("[{:?}] {:4} | {:10} | R: {:X} H: {:2} | Z: {} C: {} | {:4?} | {:?}", self.state, self.processor.prog_cnt, &writer.to_str(), self.processor.act_reg, self.processor.stack_ptr, self.processor.zero as u8, self.processor.carry as u8, self.registers, self.error);
    }
}

impl VirtMach <'_> {
    pub fn write_status<W: Write>(&self, mut writer: W) {                
        let _ = writer.write_fmt(format_args!("{:12}: {:?}-{}-{:05}-{:05}", self.program.id, self.state, self.error.clone() as u8, self.processor.prog_cnt, self.processor.stack_ptr));
    }
    
    pub fn write_dashboard<W: Write>(&self, mut writer: W, mask: usize, disassm_lines: usize) {                
        let columns = 2;
        let line = "----------------";            
        let write_line = |writer: &mut W| {            
            for _ in 0 .. columns {
                let _ = writer.write_str(line);           
            }            
            let _ = writer.write_str("\n");
        };                
        if mask & (1 << 0) != 0 {            
            if self.error == RuntimeError::NoError {
                write_line(&mut writer);
            }else{
                let _ = writer.write_fmt(format_args!("-{:24.24?}-|\n", self.error));
            }
            let _ = writer.write_fmt(format_args!("{:7.7}|   {:04x}|", &self.program.id, self.processor.prog_cnt));
            if columns == 1 { let _ = writer.write_str("\n"); }
            let _ = writer.write_fmt(format_args!("STA@FLG|{:?} {}{}{}|", self.state, if self.processor.zero { 'Z' } else { '.' }, if self.processor.carry { 'C' } else { '.' }, if self.processor.sign { 'S' } else { '.' }));            
            let _ = writer.write_str("\n");
            let _ = writer.write_fmt(format_args!("STCK@RG|{:4}  {:X}|", self.processor.stack_ptr, self.processor.act_reg));            
            if columns == 1 { let _ = writer.write_str("\n"); }
            let _ = writer.write_fmt(format_args!("CYCCNT@|{:7}|", self.cycle_cnt));            
            let _ = writer.write_str("\n");
        }
        if mask & (1 << 1) != 0 {
            write_line(&mut writer);
            let _ = writer.write_fmt(format_args!("REGS@@@|"));
            for i in 0 .. REG_MAX {                        
                if i % (columns * 2) == (columns * 2) - 1 { let _ = writer.write_str("\n"); }
                let _ = writer.write_fmt(format_args!("{:7}|", self.registers[i]));            
            }        
            let _ = writer.write_str("\n");
        }
        if mask & (1 << 2) != 0 {
            write_line(&mut writer);
            let _ = writer.write_fmt(format_args!("MEMORY@|"));
            for i in 0 .. MEM_SIZE {                        
                if i % (columns * 2) == (columns * 2) - 1 { let _ = writer.write_str("\n"); }
                let _ = writer.write_fmt(format_args!("{:7}|", self.memory[i]));            
            }        
            let _ = writer.write_str("\n");
        }                
        if disassm_lines != 0 {            
            write_line(&mut writer);
            let mut pos = self.processor.prog_cnt as usize;
            for i in 0 .. disassm_lines {
                match i {
                    0 => {
                        let _ = writer.write_str("@ADDR@OP@@ARG@@@");
                        for _ in 1 .. columns {
                            let _ = writer.write_str("@@@@@@@@@@@@@@@@");
                        }
                        let _ = writer.write_str("\n");
                    }
                    _ => {
                        if pos < self.program.data.len() {
                            let _ = writer.write_fmt(format_args!("{}{:4}:", if i == 1 { ">" } else { " " }, pos));                    
                            let mut buf = [0u8;16];
                            let mut op = Writer::from_buffer(&mut buf);
                            pos = VirtMach::decompile(&self.program, pos, &mut op);
                            let _ = writer.write_fmt(format_args!("{:12} ", op.to_str()));
                        }else{
                            let _ = writer.write_str("    -:---     ");
                        }
                            let _ = writer.write_str("              \n");                    
                        
                    }
                }
            }
        }        
    }
}
