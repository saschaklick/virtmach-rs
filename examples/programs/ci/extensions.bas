REM Extensions of the dialect that QBasic does not have, everything else in the ci programs is QBasic.
REM After END: A = 16, SAMECOL = -1, M = -11, R = 5, SHL = 16, SHR = 64, DIRECT = 0, C$ = "Hello, World!"
DEFINT A-Z
REQ math, string, surface
# a hash comments out a whole line
A = 0x10                        ' 0x hex literals
COL1 = #fff : COL2 = #ffffff    ' CSS colors are numbers
SAMECOL = COL1 = COL2
M = MIN(4, -2, 9) * 10 + MAX(4, -2, 9)
R = RND(5, 5)
SHL = 1 << 4 : SHR = 128 >> 1
DIRECT = math.and(3, &Hf4)      ' interrupt functions
HELLO = "Hello"                 ' a numeric variable holds the index of a string
REM string.concat writes into a dynamic string, STR$ provides one to write to
C$ = STR$(0)
string.concat(C$, "Hello, ", "World")
string.concat(C$, C$, "!")
surface.draw_text(1, 1, 0, C$)
W, H = surface.get_size()       ' several return values
HALT                            ' pauses the vm
END
