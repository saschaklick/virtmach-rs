REM String functions STR$, MID$, LEFT$, RIGHT$, LEN and joining with +. Their results are
REM dynamic strings, which need the vm's alloc feature. Positions are 1-based.
REQ surface, string
N = 1234
A$ = STR$(N)                        ' "1234"
B$ = STR$(-56)                      ' "-56"
L = LEN(A$)                         ' 4
M$ = MID$("Hello, World!", 8, 5)    ' "World"
R$ = RIGHT$("Hello, World!", 6)     ' "World!"
F$ = LEFT$("Hello", 2)              ' "He"
T$ = MID$("abc", 2)                 ' "bc"
X$ = RIGHT$("abc", 10)              ' "abc"
D$ = LEFT$(STR$(N * 2), 1)          ' "2"
MIN$ = STR$(-32767 - 1)             ' "-32768"
ZERO$ = STR$(0)                     ' "0"

REM string.concat writes into a dynamic string, STR$ provides one to write to
C$ = STR$(0)
string.concat(C$, "Hello, ", M$)    ' "Hello, World"
string.concat(C$, C$, "!")          ' "Hello, World!"
surface.draw_text(1, 1, 0, C$)

REM + joins strings, also when the target is one of the parts
J$ = "Hello" + ", " + M$ + "!"      ' "Hello, World!"
S$ = ""
FOR I = 1 TO 3
  S$ = S$ + STR$(I)
NEXT I                              ' S$ = "123"
END
