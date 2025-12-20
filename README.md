# virtmach-rs

## Description

A small-scale virtual machine implementation, using a custom set of op codes that is optimized for binary size and embedded/no_std environments.

A basic assembler/compiler is provided to turn op code listings into a binary format that the processor can run.

## Concepts

*virtmach* was developed to run on small to very small microcontrollers. The processor can be compiled leaving a sub-2kb footprint when stripped down to the processor core.

The processor exposes 15 registers (**r0** - **r14**) and a memory area of adjustable size.

The memory contains both a flat address space to load and store single data value by address, but also contains a stack growing down from the end of the memory to facilitate subroutine calling and push/pop instructions.

```
; Set register 3 to value -95, then store the value in memory location 14.
    
    reg r3   ; Select register 3 as the active register. The active
             ; register receives all results from other operations and
             ; also provides the first value used in `add` and `sub` ops.
    set #-95 ; Set register 3 to -95.
    sto #14  ; Store register 3's content at memory location 14. 
    reg r0   ; Change active register to register 0.
    loa #14  ; Load the value at memory address 14 into register 0.
```

### no_std

The processor, program loader and all provided software interrupts and also many debugging/logging features are **fully no_std compatible** to allow run virtmach programs on embedded/core devices.

Only the compiler requires **std**.

### Scalability

As mentioned, the size of the memory is user configurable. Reducing it to 0 is possible, but prevents op codes related to subroutine calling and push/pop from working.

The number of 14 registers could be reduced if fewer are required.

The most powerful scaleability is the choice of three register/memory entry sizes, 8, 16 and 32 bit, chosen with he **i8**, **i16** or **i32** feature in `Cargo.toml`.

### Extendability

While the basic op codes are very limited in their functionality, the processor can be infinitely extended by using software interrupts. The package already includes a **Math** interrupt, adding basic arithmetics to the addition and substraction provided by the instruction set.

The software interrupts are provided with the full virtual machine, including registers and memory, but using the stack to transfer values into the interrupt and back into the memory/register space of the processor is strongly suggested.

```
; Performing a multiplication with the Math-SoftInterrupt in interrupt slot 0,
; pop'ing the result into register 4.
    
    psh #7  ; 1st factor
    psh #16 ; 2nd factor
    psh #5  ; No. of the `mul`-function in the Math interrupt
    int 0   ; Call the Math interrupt
    pop r4  ; Move the result of the call from the stack into a register
```

### Processor basics

#### Registers

The registers and memory are arrays of atoms - called **VMAtom** in code - which can either be `i8`, `i16` or `i32`, which is chosen at the library's compile time. That means that registers and memory values are always signed.

Each operation only takes one argument, either a register (`r5`) or fixed, signed value (`#127`). To allow two registers to be added for example, first we choose an active register (`reg`), the perform an (`add`). The result will be put into the active register.

```
    reg r3   ; Set register 3 as active register.
    set #7   ; Put value 7 into the active register.
    add #100 ; Add 100 to the register, resulting in 107 in r3.
    sub r7   ; Substract what ever is stored in register 7 from r3 and put the result into r3.
    neg r3   ; Invert the value in register 0.
```

While `add` performs basic addition, `sub` performs substraction, substracting the specified register of values from the value in the active register.

Each register's value can be inverted from negative to positive and vice versa with the `neg` instruction.

#### The stack

Values - either from registers or fixed values - can be pushed onto the stack (`psh`) or moved from stack into a register (`pop`) when called with a register. Furthermore values from the stack can be pop'ed right into a memory address when it is called with a fixed value.

```
    psh r6     ; Push value in register 6 onto the stack.
    psh #101 ; Push 101 onto the stack.
    pop r8     ; Pop the previously pushed 101 from the stack into register 8.
    pop #12    ; Pop the pushed content of register 6 into memory address 12.
```

#### Flow control

The processor has an **Instruction pointer** that points to the next instruction to be executed by the processor. It always starts at 0, advances with each instruction and is changed by jumps and subroutine calls.

Subroutine calls (`cal`) are unconditional and will always be followed. On a call, the instruction pointer is pushed on the stack. Returning from a subroutine should always be performed by a return (`ret`), which pops the instruction pointer back from the stack.

Jumps are either unconditional (`jmp`) or conditional (`jpc`, `jmz`, `jms`), only changing the instruction pointer if one of three processor flags are set:

* **carry**: The last instruction's result overflowed in either direction, positive or negative.
* **zero**: The last instruction's result (add/sub) was 0.
* **sign**: The last instruction's result was negative.

All three flags can be inverted (`inv`) which allows the conditional jumps to be used for their specific inverted condition.

```
    reg r0  ; Put 4 in register 0 and substract 5, which does not set the zero
    set #4  ; or carry flag, but does set the sign flag.
    sub #5               
    jpz result_was_zero     ; Does not jump, because zero-flag is not set.
    inv                     ; Flags get inverted, which unsets sign but sets carry and zero.
    jpz result_was_not_zero ; Does jump since the flag is set now.
```

Furthermore the clear instruction (`clr`) unsets all flags.

#### Processor control

Once finished, the program can halt the processor either with a halt instruction (`hlt`) or end instruction (`end`), which sets the processor status to either "halted" or "ended". The program that is running the VM can decide how to react to the VM reaching these states. The VM will continue running from "halted" state but needs to be reset to run any further when "ended" is reached.

#### Software interrupts

As mentioned above, when a interrupt instruction (`int`) is executed with a fixed value, the processor checks its list of register interrupts and executes the interrupt handler, then proceeds when the handler has finished.

## Listing compiler

The provided compiler expects a limited assembler-related program listing.

Everything following a `;` are treated as comments are ignored until the end of the line is reached.

### Labels

Labels, the targets of jump and call instructions, are formatted as such:

```
result_was_zero:
    end
result_was_not_zero:
    jmp start
```

No spaces are allowed in labels.

### Instructions

The instructions are all three characters long and followed by at most one argument.

```
    sub #1233
    jps result_was_negative
    neg
    jps result_was_negative_after_neg
```
