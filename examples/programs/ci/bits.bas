REM Bit operations and integer arithmetic through the math interrupt.
REM Operators * / MOD << >> AND OR XOR NOT compile to math.* calls,
REM the interrupt functions can also be called directly.
REM With more than 12 variables the last ones (TRUTH, V, BITS) live in memory.
DIRECT = math.and(0x03, 0xf4)   ' 0
BAND = 0x3C AND 0xF0            ' 48
BOR = 0x3C OR 0xF0              ' 252
BXOR = math.xor(0x3C, 0xF0)     ' 204
BNOT = NOT 0                    ' -1
MASK = 0xFFFF                   ' -1 with 16 bit atoms
SHL = 1 << 4                    ' 16
SHR = 0x100 >> 2                ' 64
QUOT = 100 / 7                  ' 14
REMD = 100 MOD 7                ' 2
PREC = 2 + 3 * 4 - 10 / 5       ' 12
PAREN = (2 + 3) * -4            ' -20
TRUTH = (3 < 4) + (4 < 3) * 2 + (5 = 5)   ' -1 + 0 + -1 = -2

REM population count of 0xB5 = 1011 0101 -> BITS = 5
V = 0xB5 : BITS = 0
WHILE V
  BITS = BITS + (V AND 1)
  V = V >> 1
WEND
END
