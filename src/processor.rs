use crate::{ MEM_SIZE, StackPtr };

pub struct Processor {
    pub stack_ptr: StackPtr,
    pub prog_cnt: usize,
    pub act_reg: u8,
    pub zero: bool,    
    pub carry: bool,
    pub sign: bool    
}
impl Default for Processor {
    fn default() -> Self { Self {
        stack_ptr: (MEM_SIZE - 1) as StackPtr,
        prog_cnt: 0,    
        act_reg: 0,
        zero: false,
        carry: false,
        sign: false        
    } }
}
