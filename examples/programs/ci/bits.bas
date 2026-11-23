REM Bit operations and integer arithmetic through the math interrupt.
REM Operators * \ MOD << >> AND OR XOR NOT compile to math.* calls,
REM the interrupt functions can also be called directly.
REM With more than 12 variables the last ones (TRUTH, V, BITS) live in memory.
DEFINT A-Z
DIRECT = math.and(&H03, &Hf4)   ' 0
BAND = &H3C AND &HF0            ' 48
BOR = &H3C OR &HF0              ' 252
BXOR = math.xor(&H3C, &HF0)     ' 204
BNOT = NOT 0                    ' -1
MASK = &HFFFF                   ' -1 with 16 bit atoms
SHL = 1 << 4                    ' 16
SHR = &H100 >> 2                ' 64
QUOT = 100 \ 7                  ' 14
REMD = 100 MOD 7                ' 2
PREC = 2 + 3 * 4 - 10 \ 5       ' 12
PAREN = (2 + 3) * -4            ' -20
TRUTH = (3 < 4) + (4 < 3) * 2 + (5 = 5)   ' -1 + 0 + -1 = -2

REM population count of &HB5 = 1011 0101 -> BITS = 5
V = &HB5 : BITS = 0
WHILE V
  BITS = BITS + (V AND 1)
  V = V >> 1
WEND
END
