# Examples

```
cargo run --example compile --features compile

Program "compiled" (39b):

        0000 | 02       | loa r0
        0001 | 00       | reg r0
        0002 | F6 01 00 | add #1
        0005 | F3 00 00 | sto #0
...
```

Compiles file `programs/ci/count.txt` and prints out the disassembled binary.

```
cargo run --example run --features compile

--------------------------------
compile|   000a|STA@FLG|Run ...|
STCK@RG|  21  0|CYCCNT@|      4|
--------------------------------
REGS@@@|      1|      0|      0|
      0|      0|      0|      0|
      0|      0|      0|      0|
...
```

Compiles file `programs/ci/count.txt` and continuously runs it at 250ms per instruction.

```
cargo run --example run --features basic -- examples/programs/ci/sum.bas 10
```

Runs another listing or a `.bas` program. The optional second argument sets the milliseconds per instruction. For BASIC programs, the variable values are printed to the right of the dashboard. Only the **proc** and **math** interrupts are available here.

```
cargo run --example surface_term --features compile

╔════════════════════════════════════════════════════════════════╗ --------------------------------
║                                                                ║ compile|   0036|STA@FLG|Hlt ZCS|
║                                                                ║ STCK@RG|  22  1|CYCCNT@|  43390|
║                                                                ║ --------------------------------
║                                                                ║ REGS@@@|    -14|      7|      1|
║                                                                ║      -6|      1|      0|     12|
║    ▀                                                           ║      64|     40|     32|     20|
║                                                                ║       0|      0|      0|      0|
║                                                ▄▄              ║ --------------------------------
║                                                ▀▀              ║ MEMORY@|     14|     -2|    -23|
...
```

Compiles file `programs/gfx/starfield.txt` and continuously runs it at 30 frames-per-second.

Uses the **surface** interrupt defined in `int_surface_term.rs` to draw to a bitplane framebuffer, which is then printed to the terminal.

```
cargo run --example surface_sdl2 --features compile
```

![surface_sdl2 example output](surface_sdl2.png)

Compiles file `programs/gfx/primitives.txt` and continuously runs it at 30 frames-per-second.

Uses the **surface** interrupt defined in `int_surface_sdl2.rs` to draw to a window.

# Running other programs

Every example takes an optional file as its first argument, either a listing or a BASIC program. With the `basic` feature, a file whose first non-empty line is a `REM` statement (optionally numbered, like `10 REM`) is compiled as BASIC, and anything else is compiled as a listing. BASIC programs show their variable values next to the dashboard. `compile` lists where each variable is kept instead.

```
cargo run --example compile --features basic -- examples/programs/ci/gcd.bas
cargo run --example surface_term --features basic -- examples/programs/gfx/bounce.bas
cargo run --example surface_sdl2 --features basic -- examples/programs/gfx/bounce.bas
cargo run --example surface_term --features basic -- examples/programs/gfx/ellipse.bas
```

`run` only provides the **proc** and **math** interrupts. The other examples also provide **string** and **surface**.

# Programs

The programs are written in QBasic style (`DEFINT A-Z`, `\` for integer division, `&H` literals, `$` strings) wherever they can be. What cannot be QBasic is the interrupt calls (`math.*`, `surface.*`, `periph.*`), `REQ`, `HALT`, `<<` and `>>`, and `extensions.bas` collects the rest.

Located in the `programs` directory.

|Program|Function|Required interrupt(s)|
|--|--|--|
|`count.txt`|Counts up register 0 and performs some basic arithmetics and store operations.|**base**|
|`starfield.txt`|Displays smaller blinking dots and larger dots floating outwards.|**base**, **surface**
|`primitives.txt`|Draws all of the surface-interrupts primitives along a moving point.|**base**, **surface**
|`sum.bas`|FOR/NEXT loops with positive and negative STEP.|**math**|
|`primes.bas`|Counts primes with WHILE, MOD and block IF.|**math**|
|`gcd.bas`|Line numbers, GOSUB/RETURN and GOTO.|**math**|
|`bits.bas`|Hex literals, operators and direct `math.*` calls; spills variables to memory.|**math**|
|`fib.bas`|Arrays with DIM.|**math**|
|`dice.bas`|Calls `math.random`.|**math**|
|`ellipse.bas`|Draws a rotating ellipse with `math.sin`, `math.cos`, `math.deg_to_rad` and `math.scale`, and pauses on HALT once per frame.|**math**, **surface**|
|`bounce.bas`|Draws a bouncing box with `surface.*` calls and pauses on HALT once per frame.|**math**, **surface**|
|`pong.bas`|Pong playing against itself, with computer paddles, bounce angles based on where the ball hits the paddle, and scores turned into text with `STR$` and drawn with `surface.draw_text`. Needs a surface that implements `draw_text` and `get_text_size`, and the `alloc` feature.|**math**, **surface**, **string**|
|`control.bas`|`ELSEIF`, `SELECT CASE`, `DO … LOOP`, `EXIT`, `ON … GOTO/GOSUB`, labels and a variable `STEP`.|**math**|
|`subs.bas`|`SUB`, `FUNCTION`, `DEF FN`, `SHARED` and the built-ins `SWAP`, `SGN`, `MIN`, `MAX`, `RND`.|**math**, **string**|
|`strings.bas`|String literals as `#db` entries, `LEN` and string comparison, passed to `surface.draw_text`.|**surface**, **string**|
|`strfuncs.bas`|`STR$`, `MID$`, `LEFT$`, `RIGHT$`, `LEN` and joining with `+` on the **string** interrupt. Results are dynamic strings and need the `alloc` feature.|**surface**, **string**|
|`types.bas`|`AS STRING`/`AS INTEGER`, `DEFSTR`/`DEFINT`, typed parameters and `SHARED`.|**math**, **string**|
|`syntax.bas`|`REM`/`'` comments, line numbers and labels, `&H` literals, case insensitivity, `ON … GOTO`.|**math**, **string**|
|`extensions.bas`|What QBasic does not have: `#` comment lines, `0x` literals, `#rgb` colors, `MIN`/`MAX`/`RND(lo, hi)`, `<<` `>>`, interrupt calls, string indices in numeric variables.|**math**, **string**, **surface**|
|`decl.bas`|`CONST`, constant array sizes, `DIM SHARED`, `DECLARE` and `OPTION BASE`.|**math**, **string**|
|`operators.bas`|`\`, `EQV`, `IMP`, precedence, `&H`/`&O`/`&B` literals and strings in conditions.|**math**, **string**|
|`data.bas`|`DATA`, `READ` and `RESTORE` with numbers, texts, labels and line numbers.|**math**, **string**|
|`random.bas`|`RANDOMIZE` repeating the sequence of `math.random`.|**math**|
|`sleep.bas`|`SLEEP` through `time.wait_for`, only compiled.|**time**|
|`console/print.bas`|`PRINT`, `?` and `LOCATE` on the surface, only with `basic_console="surface"`.|**math**, **string**, **surface**|
|`drawing.bas`|`CLS`, `PSET`, `PRESET`, `LINE`, `VIEW`, `SCREEN` and `COLOR` (`surface.set_color`) on the surface.|**math**, **surface**|
|`colors.bas`|Colors of drawing statements: color literals, constants and indices of the VGA palette, `COLOR` with one part.|**math**, **surface**|
|`peekpoke.bas`|`PEEK` and `POKE` on the VM's memory.|**math**|

# BASIC

How the dialect differs from Minimal BASIC, and what it adds, is described in [BASIC.md](../BASIC.md).

With the `basic` feature, `VirtMach::compile` and `VirtMach::compile_owned` also accept BASIC programs, so the SDK's `compiler` compiles them too. BASIC is detected by a first line starting with `REM`. Comments in BASIC are `REM` and `'`, which comment out the rest of the line, also behind a statement, and `#` for a whole line (behind a statement `#` starts a color literal). `;` is no comment, it separates the items of `PRINT`. Only `REM` counts for the detection, as `#` and `;` are comments in listings. `sdk/src/bin/basic.rs` is a BASIC-only front end that can also write the generated listing (`-l`) and prints where each variable is kept. It compiles into a virtmach listing and then into a binary. It takes the same interrupt list as `compiler`: the order sets the interrupt numbers, and each interrupt's functions are checked against the `.csv` file with the same name. Both tools look for that file next to the compiled source first, then in `./include/`, then in each directory given with `-I` (repeatable), in order, and last in the repository's `include/`, where its CSV files are. So they are found from any working directory. Use the runtime's order:

```
cargo run --manifest-path sdk/Cargo.toml --bin basic -- examples/programs/gfx/bounce.bas proc math surface -l bounce.txt
```

`-l` writes the generated listing, and `-v 2` also prints it. Without a print interrupt, results stay in registers and memory. The compiler prints which register or memory cell each variable uses.

`REQ surface, math` states which interrupts a program needs: compiling fails at that line, naming the available ones, if one of them isn't in the interrupt list. Interrupt functions are called as `interrupt.function(args)`, for example `X = math.and(0x03, 0xf4)`, `surface.clear(0)` or `W, H = surface.get_size()`. Text in double quotes (write `""` for a quote inside it) becomes a `#db` entry. Its value is the entry's index, so `S = "Hello"` stores a number that can be passed to functions like `surface.draw_text`. Identical texts share one entry, and there can be at most 255 different ones. `STR$(n)`, `MID$(s, start[, length])`, `LEFT$(s, n)` and `RIGHT$(s, n)` return the index of a dynamic string built with `string.format` or `string.substr`, and `LEN(s)` calls `string.get_length`. `+` joins strings with `string.concat` when an operand is a string (a literal, a name ending in `$`, or another join), e.g. `S$ = S$ + STR$(I)`. Positions are 1-based. Each call in the source owns one dynamic string that is overwritten every time the call runs, so literals and these calls together can use at most 255 string indices (more is a compile error). They need the vm's `alloc` feature. `+` and `-` compile to native instructions. `*`, `/`, `MOD`, `^`, `<<`, `>>`, `AND`, `OR`, `XOR` and `NOT` compile to `math` interrupt calls. `SUB` and `FUNCTION` have local variables (unless declared `SHARED`) that start at 0 on every call. They are stored statically, so recursion is a compile error. Strings compare by content through `string.compare`. Only INTEGER and STRING exist (strings by `$` suffix, `AS STRING` or `DEFSTR`): floating point literals, the suffixes `& ! #` and `AS LONG/SINGLE/DOUBLE` are compile errors. The language reference is at the top of `src/basic.rs`, and `cargo test --bin basic` (inside `sdk`) runs all example programs in the VM.

`programs/ci/invalid/` holds programs that must fail to compile. Their first line, `REM expect: <part of the error>`, names the expected error. `cargo test` checks them in the compiler and through the `basic` and `compiler` binaries, and compiles every program of `programs/ci/`.

## Console and drawing statements

The drawing statements (`CLS`, `COLOR`, `PSET`, `PRESET`, `LINE`, `VIEW`) always compile onto the **surface** interrupt. The text console (`PRINT`, `LOCATE`, `WIDTH`, `VIEW PRINT`) is a compile error unless the crate is built with an optional cfg flag, which compiles it onto the surface too:

```
RUSTFLAGS='--cfg basic_console="surface"' cargo run --example surface_term --features basic -- examples/programs/console/print.bas
RUSTFLAGS='--cfg basic_console="surface"' cargo test    # also runs the tests of programs/console/
```

Use a separate `CARGO_TARGET_DIR` when switching, since changing `RUSTFLAGS` rebuilds everything. See [BASIC.md](../BASIC.md) for what each statement does.
