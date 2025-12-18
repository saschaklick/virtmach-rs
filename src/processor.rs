use crate::MEM_SIZE;

pub struct Processor {
    pub stack_ptr: usize,
    pub prog_cnt: usize,
    pub act_reg: usize,
    pub zero: bool,    
    pub carry: bool,
    pub sign: bool    
}
impl Default for Processor {
    fn default() -> Self { Self {
        stack_ptr: (MEM_SIZE - 1),
        prog_cnt: 0,    
        act_reg: 0,
        zero: false,
        carry: false,
        sign: false        
    } }
}
