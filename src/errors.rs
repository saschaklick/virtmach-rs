#[derive(Debug)]
#[derive(PartialEq)]
#[derive(Clone)]
pub enum RuntimeError {
    NoError,
    MismatchedAtomType,
    IllegalInstruction,
    RegisterOutOfBounds,
    ProgramOutOfBounds,
    MemoryOutOfBounds,
    InstructionPointerOutOfBounds,
    HeapOverflow,
    HeapUnderflow,
    HeapCrash,
    UnhandledInterrupt,
    UnimplementedInterruptFunc,
    InterruptError
}