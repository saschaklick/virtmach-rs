# The BASIC dialect

Integer BASIC built on **Minimal BASIC** (ECMA-55, ANSI X3.60-1978). Whatever the standard says works as it says, with integers as the only number type, unless **Differences from Minimal BASIC** says otherwise. **Extensions** lists everything added.

A program must start with a `REM` line (its first non-empty line). It runs on a small VM that has no console, programs talk to the world through **interrupts** (see Interrupts).

## Differences from Minimal BASIC

### Not allowed (compile errors)

- **Real numbers.** Numbers are integers, 16 bits by default (8 or 32 on some builds). `1.5`, `1E5`, the suffixes `& ! #` and decimal `DATA` items are errors.
- **Functions** `ATN`, `COS`, `EXP`, `INT`, `LOG`, `SIN`, `SQR`, `TAN`. Only `ABS` and `SGN` exist (integer versions of some are `math.*` interrupt calls).
- **`RND` without arguments.** Use `RND(low, high)`.
- **`RANDOMIZE` without a seed.** Use `RANDOMIZE n`.
- **`INPUT`** in any form (`LINE INPUT`, `INKEY$`, `INPUT$`). Programs have no keyboard, do not use input features.
- **Arrays** with two dimensions, and arrays used without `DIM`. Every array needs a one-dimensional `DIM`.
- **`TAB`** in `PRINT`, and `PRINT` itself on builds without the text console (see Text console).
- **`GO TO` and `GO SUB`** as two words. Write `GOTO`, `GOSUB`.
- **More than 255 strings**: string literals and string operations (`STR$`, `MID$`, `LEFT$`, `RIGHT$`, `+`, each use counts once) together may not exceed 255 (`out of string slots`).

### Works differently

- `/` divides integers, truncating towards zero, and `^` is an integer power. Arithmetic wraps at the atom size. Division by zero stops the program with an error.
- `RANDOMIZE n` seeds the random numbers, the same seed repeats the same values.
- `DATA` takes signed numbers (also `&H`, `&O`, `&B`, `0x`), quoted texts and bare texts. All `DATA` forms one list in source order, also inside procedures. `READ` past the end stops the program with an error. **There is no type check:** reading a text into a number variable gives a meaningless number (its string index), a number into a `$` variable a bad string. `RESTORE` takes a line number or label.
- Array indices that are constants are checked when compiling, variable indices are **not checked** at run time.
- `OPTION BASE 1` must come before the first `DIM`, it makes a constant index 0 an error.

## Extensions

### Program layout

- Line numbers are optional, `END` need not be last. A line may start with a label `NAME:` (usable by `GOTO`, `GOSUB`, `ON`, `RESTORE`) and hold several statements separated by `:`.
- `LET` is optional. Keywords are case-insensitive. Names may contain letters, digits and `_` and be long.
- Comments: `REM` and `'` to the end of the line, also behind a statement. `#` comments out a whole line, behind a statement `#` starts a color. **`;` is no comment**, it separates the items of `PRINT`.
- `REQ name[, …]` states the interrupts the program needs, compiling fails if one is missing.
- `HALT` pauses the program (for example once per frame), `STOP` breaks.

### Control flow

- `IF cond THEN … [ELSEIF … THEN …] [ELSE …] END IF`, and one line `IF cond THEN stmts|line [ELSE stmts|line]`. Conditions are full expressions.
- `SELECT CASE x` with `CASE v1, v2`, `CASE a TO b`, `CASE IS > n`, `CASE ELSE`, also for strings, `END SELECT`.
- `WHILE … WEND`, `DO [WHILE|UNTIL cond] … LOOP [WHILE|UNTIL cond]`, `EXIT FOR|WHILE|DO|SUB|FUNCTION`.
- `ON n GOTO|GOSUB` and `GOTO`/`GOSUB` with labels.
- In the conditions of `IF`, `WHILE`, `DO`, `CASE`, `AND`, `OR`, `NOT` are logical and short-circuit. Anywhere else they are bitwise (they differ only for operands that are not -1 or 0).

### Procedures

- `SUB name[(params)]` … `END SUB`, `FUNCTION name[(params)]` … `END FUNCTION` (assign the result to the name), `DEF FNname(params) = expr`. Call with `CALL name(args)`, `name args` or `name(args)`; functions also inside expressions. Parameters are passed **by value**, arrays cannot be parameters.
- Locals start at 0 on every call and are not kept. **Recursion is a compile error**, also an indirect one.
- Module variables are not visible in procedures unless `SHARED name` is written inside, or `DIM SHARED` outside of procedures.
- `DECLARE SUB|FUNCTION name(params)` is optional and checked against the definition.
- `CONST name = expr[, …]` takes numbers, texts, other constants and operators, nothing that varies. Constants work in procedures and as array sizes: `DIM A(N * 2)`.

### Data types

- Only **INTEGER** and **STRING**. A name ending in `$` is a string, `%` marks an integer (`X%` is `X`).
- `AS INTEGER` / `AS STRING` in `DIM`, parameters and `SHARED`, and `DEFINT` / `DEFSTR` by first letter, declare types: `DIM X AS STRING` is `X$`.
- Other types are errors that name them: `AS LONG/SINGLE/DOUBLE`, `AS <name>`, `STRING * n`, `DEFLNG/DEFSNG/DEFDBL`, `TYPE`, `CSNG/CDBL/CLNG`.

### Strings

- `+` joins, all six relational operators compare by content. `""` inside a literal is a quote.
- `LEN(s)`, `MID$(s, start[, length])`, `LEFT$(s, n)`, `RIGHT$(s, n)` (1-based), `STR$(n)`, which puts a **blank in front of numbers that are not negative** (`STR$(5)` is `" 5"`, cut it with `MID$(STR$(n), 2)`).
- A string is an index, a number variable can hold one (`S = "Hello"`) to pass it to an interrupt function. Assignments are not type checked.

### Operators and literals

- Precedence, highest first: `^`, unary `-`, `*` `/`, `\` (integer division), `MOD`, `+` `-`, `<<` `>>`, comparisons, `NOT`, `AND`, `OR`, `XOR`, `EQV`, `IMP`.
- All of `\ MOD << >> AND OR XOR NOT EQV IMP`, `*`, `^` need the `math` interrupt (`REQ math`). `<<` `>>` shift, `AND OR XOR NOT EQV IMP` are bitwise. Comparisons give -1 (true) or 0.
- Literals: decimal, `&H`, `&O`, `&B`, `0x`, and colors `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa` (a number, see Drawing). Hex, octal and binary numbers wrap into the atom: `&HFFFF` is -1 with 16 bits.
- `MIN(a, b, …)`, `MAX(a, b, …)`, `RND(low, high)` (both limits included, needs `math`), `SWAP a, b`.

### Interrupts

- Everything outside of the processor is an interrupt function, called as `interrupt.function(args)` in an expression or as a statement, several results into several targets: `W, H = surface.get_size()`. Names and argument counts are checked when compiling.
- `PEEK(address)` and `POKE address, value` read and write a **cell of the VM's memory** (an atom). Nothing is checked but the memory length, so they can hit variables, arrays and the stack, outside the memory the program stops.
- `SLEEP n` waits `n` seconds (needs `time`). `SLEEP` alone is an error.
- What each part needs: operators above, `RND`, `RANDOMIZE`, `READ`: `math`. `STR$`, `MID$`, `LEFT$`, `RIGHT$`, `LEN`, string `+`: `string`. Drawing statements: `surface` and `math`. `SLEEP`: `time`.

### Drawing

`COLOR`, `PSET`, `PRESET`, `LINE`, `VIEW`, `CLS` draw on the `surface`.

**Colors** are never plain palette indices. Wherever a drawing statement takes a color, write a **color literal** (`#00ff00`), a constant defined as one (`CONST GREEN = #00ff00`), or a **constant number**, an index of the default VGA palette (0 to 255: 0 black, 1 blue, 2 green, 4 red, 14 yellow, 15 white, 16 to 31 grays). A variable or any other expression is an error (call `surface.*` directly for run-time colors). The colors start white on black.

| Statement | Effect |
|---|---|
| `COLOR fg, bg` | Foreground for `PSET` and `LINE`, background for `PRESET` and `CLS`, and the colors of text. Either part may be left out. |
| `PSET (x, y)[, c]` / `PRESET (x, y)[, c]` | A pixel in `c`, or in the foreground / background. `STEP (dx, dy)` counts from the last point. |
| `LINE [(x1, y1)]-(x2, y2)[, c[, B\|BF]]` | A line, a box (B) or a filled box (BF). A missing start is the last point. No line styles. |
| `VIEW (x1, y1)-(x2, y2)` / `VIEW` | Clips drawing to the area / resets the clip. No colors, borders or `STEP`. |
| `CLS` | Clears with the background, the text cursor goes to the top left. |
| `SCREEN` | Accepted and ignored. |

### Text console

`PRINT` (also `?`), `LOCATE`, `WIDTH` and `VIEW PRINT` exist only on builds with the optional surface console, on all others they are compile errors, so do not use them unless the target is known to support them. There `PRINT` draws text at a cursor on the surface: items separated by `;` follow each other, `,` jumps to the next zone of 14 characters, a `PRINT` not ending in `;` or `,` starts a new line, numbers get a blank in front of and behind them. **There is no scrolling.** `PRINT USING`, `TAB`, `SPC` are errors. `LOCATE`, `WIDTH` and `VIEW PRINT` are ignored. Needs `surface` and `string` (and `math` for `,`).

### Never supported

Each is a compile error that says so: `INPUT`, `LINE INPUT`, `INKEY$`, `INPUT$`, `LPRINT`, `BEEP`, `SOUND`, `PLAY`, `CIRCLE`, `PAINT`, `DRAW`, `GET`, `PUT`, `WINDOW`, `PALETTE`, `POINT()`.

### Reserved words

These cannot be names: all keywords of the standard and every statement or operator word above, in particular `AS`, `TYPE`, `REQ`, `HALT`, `EQV`, `IMP`, `MOD`, `XOR`, `AND`, `OR`, `NOT`, `IS`, `EXIT`, `STOP`, `SWAP`, `SUB`, `FUNCTION`, `CALL`, `SHARED`, `CONST`, `DECLARE`, `OPTION`, `DATA`, `READ`, `RESTORE`, `RANDOMIZE`, `SLEEP`, `POKE`, `PRINT`, `USING`, `CLS`, `LOCATE`, `COLOR`, `SCREEN`, `WIDTH`, `VIEW`, `PSET`, `PRESET`, `LINE`, `INPUT`, `CIRCLE`, `PAINT`, `DRAW`, `GET`, `PUT`, `WINDOW`, `PALETTE`, `PLAY`, `SOUND`, `BEEP`, `LPRINT`. Do not name variables `LINE`, `COLOR`, `DATA`, `READ`, `VIEW`, `WIDTH` and so on.

### Other

- A `FOR` variable cannot be a string. A `STEP` that is not constant is checked while running.
