use core::ops::Neg;
use core::{slice, str};

use crate::{REG_MAX, MEM_SIZE};
use crate::opcodes::OpCode;
use crate::processor::Processor;

pub use crate::atom::{ATOM_ID, VMAtom, VMAddr, VAtom};
pub use crate::errors::RuntimeError as RuntimeError;
pub use crate::program::Program as Program;
pub use crate::writer::Writer as Writer;
use crate::interrupts;

#[derive(Debug)]
#[derive(PartialEq)]
pub enum Runtime {
    Ini,
    Run,
    Hlt,
    Stp,    
    Err
}
pub struct VirtMach <'a> {
    pub registers: [VMAtom;REG_MAX],
    pub memory: [VMAtom;MEM_SIZE],
    pub cycle_cnt: usize,
    pub(crate) program: Program<'a>,    
    pub error: RuntimeError,    
    pub(crate) processor: Processor,
    pub state: Runtime,
    halt_on_break: bool
}

impl <'a> VirtMach <'_> {
    pub fn new() -> Self {        
        let res =  Self {
            registers: [0 as VMAtom;REG_MAX],
            memory: [0 as VMAtom;MEM_SIZE],            
            program: Program::EMPTY,                
            error: RuntimeError::NoError,
            cycle_cnt: 0,
            processor: Processor::default(),
            state: Runtime::Ini,
            halt_on_break: false
        };    
        
        return res;  
    }

    pub fn load_program (&mut self, program: Program) {        
        if program.data.len() == 0 {
            self.program = Program::EMPTY;
            return;
        }

        let data = &program.data[1..];
        
        if data.len() == 0 {
            self.program = Program::EMPTY;
            return;
        }

        if program.data[0] != ATOM_ID {
            self.program = Program::ERROR;
            self.error = RuntimeError::MismatchedAtomType;
            return;
        }
    
        self.reset();  
        self.program.source = program.source;
        self.program.id = unsafe { str::from_utf8_unchecked(slice::from_raw_parts(program.id.as_ptr(), program.id.len())) };
        self.program.data = unsafe { slice::from_raw_parts(data.as_ptr(), data.len()) };
        self.processor = Processor::default();         
        self.state = Runtime::Hlt;             
    }

    pub fn stack_push(&mut self, val: VMAtom) {
        if self.processor.stack_ptr == 0 {
            self.error = RuntimeError::HeapOverflow;
        } else {
            self.memory[self.processor.stack_ptr] = val;
            self.processor.stack_ptr -= 1;
        }
    }

    pub fn stack_pop(&mut self) ->  VMAtom {
        if self.processor.stack_ptr >= MEM_SIZE - 1 {
            self.error = RuntimeError::HeapUnderflow;
            return 0 as VMAtom;
        } else {
            self.processor.stack_ptr += 1;
            return self.memory[self.processor.stack_ptr];            
        }
    }

    pub fn step (&mut self, interrupts: &mut [&'_ mut dyn interrupts::SoftInterrupt]) {
        if self.state != Runtime::Run {
            return;
        }

        if self.processor.prog_cnt >= self.program.data.len() {
            self.error = RuntimeError::ProgramOutOfBounds;
            return;
        }

        let instructions = self.program.data;        

        let byte = instructions[self.processor.prog_cnt];
        let op = byte & 0x0f;
        let reg:u8;
        let inst_pos = self.processor.prog_cnt;
        self.processor.prog_cnt += 1;

        let val: VMAtom;
        if op < 0x0f {
            reg = (byte >> 4) & 0x0f;
            if reg == 15 {
                val = instructions[self.processor.prog_cnt .. self.processor.prog_cnt + size_of::<VMAtom>()].as_ref().get_atom();
                self.processor.prog_cnt += 2;
            }else{
                val  = self.registers[reg as usize];
            }
        }else{
            reg = 0xf;
            val = 0 as VMAtom;
        }

        fn add(vm: &mut VirtMach, a: VMAtom, b: VMAtom) -> VMAtom {            
            let add_res = a.overflowing_add(b);
            vm.processor.zero = add_res.0 == 0;                        
            vm.processor.sign = add_res.0 < 0;
            vm.processor.carry = add_res.1;
            return add_res.0;
        }

        fn sub(vm: &mut VirtMach, a: VMAtom, b: VMAtom) -> VMAtom {            
            let sub_res = a.overflowing_sub(b);
            vm.processor.zero = sub_res.0 == 0;                        
            vm.processor.sign = sub_res.0 < 0;
            vm.processor.carry = sub_res.1;
            return sub_res.0;
        }

        fn jmpchk(vm: &mut VirtMach, offset: VMAtom, is_cal: bool) {            
            match VMAddr::try_from(vm.processor.prog_cnt) {
                Ok(prog_cnt) => {
                    let res = prog_cnt.overflowing_add(offset as VMAddr);
                    if res.1 == false && res.0 >= 0 {
                        if is_cal { vm.stack_push(vm.processor.prog_cnt as VMAtom); }
                        vm.processor.prog_cnt = res.0 as usize;
                    }else{
                        vm.error = RuntimeError::InstructionPointerOutOfBounds
                    }            
                }
                Err(_) => vm.error = RuntimeError::InstructionPointerOutOfBounds
            }
            
        }

        fn memchk(vm: &mut VirtMach, addr: VMAtom) -> bool {
            if addr < 0 as VMAtom || addr >= MEM_SIZE as VMAtom { vm.error = RuntimeError::MemoryOutOfBounds; return false; }
            if addr >= vm.processor.stack_ptr as VMAtom { vm.error = RuntimeError::HeapCrash; }
            return true;
        }

        match op {            
            x if x == (OpCode::REG as u8) => { self.processor.act_reg = reg.into(); }            
            x if x == (OpCode::SET as u8) => { self.registers[self.processor.act_reg] = val; }            
            x if x == (OpCode::LOA as u8) => { if memchk(self, val) { self.registers[self.processor.act_reg] = self.memory[val as usize]; } }
            x if x == (OpCode::STO as u8) => { if memchk(self, val) { self.memory[val as usize] = self.registers[self.processor.act_reg]; } }            
            x if x == (OpCode::PSH as u8) => { self.stack_push(val); }            
            x if x == (OpCode::POP as u8) => { if reg != 0x0f { self.registers[reg as usize] = self.stack_pop(); } else if memchk(self, val) { self.memory[val as usize] = self.stack_pop(); } else { self.error = RuntimeError::RegisterOutOfBounds; } }                                    
            x if x == (OpCode::ADD as u8) => { self.registers[self.processor.act_reg] = add(self, self.registers[self.processor.act_reg], val); }                                    
            x if x == (OpCode::SUB as u8) => { self.registers[self.processor.act_reg] = sub(self, self.registers[self.processor.act_reg], val); }                                   
            x if x == (OpCode::CAL as u8) => { jmpchk(self, val, true); }
            x if x == (OpCode::JMP as u8) => { jmpchk(self, val, false); }
            x if x == (OpCode::JPC as u8) => { if self.processor.carry { jmpchk(self, val, false); } }
            x if x == (OpCode::JPZ as u8) => { if self.processor.zero { jmpchk(self, val, false); } }
            x if x == (OpCode::JPS as u8) => { if self.processor.sign { jmpchk(self, val, false); } }                      
            x if x == (OpCode::INT as u8) => {
                let int_no = reg as usize;                
                if int_no < interrupts.len() {
                    interrupts[int_no].call(self);                    
                }else{                        
                    self.error = RuntimeError::UnhandledInterrupt;
                }
            }
            0x0f => {
                let op = byte;
                match op {                    
                    x if x == (OpCode::RET as u8) => { let addr = self.stack_pop(); if addr >= 0 { self.processor.prog_cnt = addr as usize; } else { self.error = RuntimeError::InstructionPointerOutOfBounds }  }                                      
                    x if x == (OpCode::CLR as u8) => { self.processor.zero = false; self.processor.carry = false; self.processor.carry = false;  }                                      
                    x if x == (OpCode::INV as u8) => { self.processor.zero = !self.processor.zero; self.processor.carry = !self.processor.carry; self.processor.sign = !self.processor.sign; }                                      
                    x if x == (OpCode::NEG as u8) => { let res = self.registers[self.processor.act_reg].neg(); self.registers[self.processor.act_reg] = res; self.processor.sign = res < 0; }                                      
                    x if x == (OpCode::BRK as u8) => { if self.halt_on_break == true { self.state = Runtime::Hlt; } }
                    x if x == (OpCode::HLT as u8) => { self.state = Runtime::Hlt; }
                    x if x == (OpCode::END as u8) => { self.state = Runtime::Stp; }
                    _ => { self.error = RuntimeError::IllegalInstruction; }
                }
            } 
            _ => { self.error = RuntimeError::IllegalInstruction; }
        }

        if self.error != RuntimeError::NoError {
            self.state = Runtime::Err;
            self.processor.prog_cnt = inst_pos;
        }

        self.cycle_cnt += 1;
    }

    pub fn reset(&mut self) {
        self.processor = Processor::default();   
        self.state = Runtime::Hlt;
        self.error = RuntimeError::NoError;        
        self.cycle_cnt = 0;
        self.memory.fill(0);
    }

    pub fn run (&mut self, max_ops: usize, interrupts: &mut [& mut dyn interrupts::SoftInterrupt]) {
        let mut op_cnt = 0;

        if self.state == Runtime::Hlt {
            self.state = Runtime::Run;
        }

        while (max_ops == 0 || op_cnt < max_ops) && self.state == Runtime::Run {
            self.step(interrupts);
            op_cnt += 1;            
        }    
    }

    pub fn pause(&mut self) {
        self.state = Runtime::Hlt;    
    }

    pub fn running(&self) -> bool {
        return self.state == Runtime::Run;
    }   

    pub fn paused(&self) -> bool {
        return self.state == Runtime::Hlt;
    }   
}