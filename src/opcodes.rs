#[repr(u8)]
#[derive(Clone, Copy)]
pub enum OpCode {
    // Register
    REG = 0x00,
    SET = 0x01,
    
    // Memory
    LOA = 0x02,
    STO = 0x03,

    // Heap
    PSH = 0x04,
    POP = 0x05,

    // Arithmetics    
    ADD = 0x06,
    SUB = 0x07,
    
    // Routine calling
    CAL = 0x08,    

    // Soft interrupt
    INT = 0x09,
    
    // Control flow
    JMP = 0x0a,    
    JPZ = 0x0b,    
    JPC = 0x0c,    
    JPS = 0x0d,     

    RET = 0x0f,
    CLR = 0x1f,    
    INV = 0x2f,
    NEG = 0x3f,

    BRK = 0xdf,
    HLT = 0xef,
    END = 0xff,
}