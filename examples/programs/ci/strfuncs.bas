REM String functions STR$, MID$, LEFT$, RIGHT$, LEN and joining with +. Their results are
REM dynamic strings, which need the vm's alloc feature. Positions are 1-based.
DEFINT A-Z
REQ string
N = 1234
A$ = STR$(N)                        ' " 1234", numbers that are not negative start with a blank
B$ = STR$(-56)                      ' "-56"
L = LEN(A$)                         ' 5
M$ = MID$("Hello, World!", 8, 5)    ' "World"
R$ = RIGHT$("Hello, World!", 6)     ' "World!"
F$ = LEFT$("Hello", 2)              ' "He"
T$ = MID$("abc", 2)                 ' "bc"
X$ = RIGHT$("abc", 10)              ' "abc"
D$ = LEFT$(STR$(N * 2), 2)          ' " 2"
MIN$ = STR$(-32767 - 1)             ' "-32768"
ZERO$ = STR$(0)                     ' " 0"

REM + joins strings, also when the target is one of the parts
J$ = "Hello" + ", " + M$ + "!"      ' "Hello, World!"
S$ = ""
FOR I = 1 TO 3
  S$ = S$ + MID$(STR$(I), 2)   ' the usual way to cut off the blank
NEXT I                              ' S$ = "123"
END
