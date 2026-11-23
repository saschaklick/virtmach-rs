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

The most powerful scaleability option is the choice of three register/memory entry sizes, 8, 16 and 32 bit, chosen with he **i8**, **i16** or **i32** feature in `Cargo.toml`.

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

## BASIC compiler

With the `basic` feature `VirtMach::compile` also accepts a small BASIC dialect, which is compiled into a listing first. A file is BASIC if its first non-empty line starts with `REM`. [BASIC.md](BASIC.md) is the language reference, written so that a programmer or an AI can write correct programs from it, and it is the document to keep in sync with every change of the dialect. This section is about the implementation.

### Where things are

|What|Where|
|--|--|
|Grammar (pest)|`src/basic.pest`|
|Compiler, with a language summary at the top|`src/basic.rs`|
|Color encoding, color literals and the VGA palette|`src/color.rs`|
|BASIC-only front end, it also writes the listing (`-l`)|`sdk/src/bin/basic.rs`|
|Interrupt function definitions (`.csv`, written by `gen-csv`)|`include/`|
|Example programs and the tests' programs|`examples/programs/`, see `examples/README.md`|

### How it compiles

- **Variables** live in registers `r0..r11`, further ones and all arrays in memory. `r12` and `r13` evaluate expressions. Locals of a `SUB` or `FUNCTION` are static and prefixed with its name, which is why recursion is rejected.
- **Strings** are one byte indices into the `#db` entries of the listing. Identical literals share an entry, every `STR$`, `MID$`, `LEFT$`, `RIGHT$` and string `+` owns one dynamic entry, together at most 255. Dynamic entries are written by the `string` interrupt and need the VM's `alloc` feature.
- **Operators** other than `+` and `-` call the `math` interrupt (`*` `\` `MOD` `^`, shifts, logic). `AND`, `OR` and `NOT` in conditions are compiled as short-circuit jumps instead.
- **Types** are decided by name, the parser folds declarations (`AS STRING`, `DEFSTR`) into the names (`X$`), so the code generator only looks at the `$`.
- **`CONST`** is folded while parsing, so are the sizes of arrays. A color literal that is a `CONST` keeps its color nature for the drawing statements.
- **`DATA`** of the whole program is an array `_DATA` that the program fills when it starts, a hidden pointer `_DATAPTR` walks it. Strings are stored as their index. Reading past the end divides by zero in the `math` interrupt, which stops the VM with an interrupt error.
- **`PEEK` and `POKE`** are `loa` and `sto` with the address in a register (or immediate for constants), so the VM's own memory check is the only check.
- **`RANDOMIZE n`** calls `math.seed`, `SLEEP n` calls `time.wait_for(n, 0)`.
- **Interrupt calls** are resolved by name against the `.csv` files of the loaded interrupts, which also gives the argument and result counts.
- **Hidden variables** with a leading `_` (`_PX`, `_PY`, `_GX` and so on) hold the state of the console and drawing statements. They are visible in every procedure.

### Drawing and console statements

All of them compile to calls of the `surface` interrupt:

|Statement|Calls|
|--|--|
|`COLOR`|`set_color(1, fg)` and `set_color(0, bg)` for the text, and remembers both colors|
|`PSET`, `PRESET`|`draw_pixel`|
|`LINE`|`draw_line`, `draw_rect` (`B`), `fill_rect` (`BF`)|
|`VIEW`|`set_clip` and `get_size`|
|`CLS`|`clear`|
|`PRINT`|`draw_text`, with the string interrupt's `get_length`|

The color arguments of the surface functions are color atoms: from the highest bit down alpha (the sign bit, so alpha 0 is a positive number and invisible), then RGB232, RGB555 or RGB888 depending on the atom size. The compiler turns a constant palette index into such an atom with `Color::vga`, which reads `VGA_PALETTE`, the default palette of VGA mode 13h as 8-bit RGB. Surface implementations decode the atoms with `Color::to_rgba`.

`PRINT` assumes the surface's base font to be 3 pixels wide with 1 pixel between characters and 4 pixels high with 1 pixel between lines, so a character advances the cursor by 4, a line by 5 and a `,` zone of 14 characters is 56 pixels (`CHAR_WIDTH`, `LINE_HEIGHT` and `ZONE_WIDTH` in `src/basic.rs`). It never asks the surface for a text size and does not scroll.

### The `basic_console` flag

The text console (`PRINT`, `?`, `LOCATE`, `WIDTH`, `VIEW PRINT`) is only compiled when the crate is built with an optional cfg flag, otherwise these statements are compile errors that say they are not supported. The drawing statements are always available.

```
RUSTFLAGS='--cfg basic_console="surface"' cargo build --features basic
```

`LOCATE`, `WIDTH` and `VIEW PRINT` are then accepted and ignored, the surface has no text window. `INPUT`, `INKEY$` and the statements the surface cannot do (`CIRCLE`, `PAINT`, sound and so on) are errors in every build. The flag is declared for `unexpected_cfgs` in the `Cargo.toml` files. Changing `RUSTFLAGS` rebuilds everything, use a separate `CARGO_TARGET_DIR` per configuration.

### Tests

`cargo test` runs the compiler tests of `sdk/src/bin/basic.rs` (they run the programs of `examples/programs/ci/` in the VM against a recording surface), the CLI tests of `sdk/tests/cli.rs` and the library tests.

- `ci_programs_compile` compiles every program of `examples/programs/ci/`, a new file is picked up by itself.
- The programs of `examples/programs/ci/invalid/` must fail to compile. Their first line, `REM expect: <part of the error message>`, names the error. They are checked in the compiler and through the `basic` and `compiler` binaries, so they must fail in every build.
- Programs and tests that only exist with the console flag are in `examples/programs/console/` and gated with `#[cfg(basic_console = "surface")]`, the tests that expect the console statements to be rejected with `#[cfg(not(...))]`. Run both configurations:

```
cargo test
RUSTFLAGS='--cfg basic_console="surface"' CARGO_TARGET_DIR=target-surface cargo test
```

- The surfaces of the SDK (`sdk/src/lib/int_surface_term.rs`, `int_surface_sdl2.rs`) decode color atoms, the terminal one has unit tests, the SDL2 one needs a window and is only compiled.
- `math.random` has one generator for all threads, so tests that depend on its values take turns (`RANDOM_LOCK`).

## Calling convention

While it is up to each implementation of a software interrupt handler how it receives and sends back atoms to the processor, the suggested calling convention - which all internal interrupts adhere to - expect the correct number arguments per function to be push'ed onto the stack. The interrupt function will pop the values of the stack and push the result atom(s) onto the stack for the processor to retrieve.