REM Declarations: CONST, constant array sizes, DIM SHARED, DECLARE and OPTION BASE
REM After END: R = 63, S = 6, TXT$ = "Hi!", ARR(1..6) = 3, 6, 9, 12, 15, 18, MASK = 63
DEFINT A-Z
REQ math, string
OPTION BASE 1
CONST N = 3, TWICE = N * 2
CONST GREETING$ = "Hi" + "!"
CONST MASK = &H0F OR &H30

DECLARE SUB Fill (Count)
DECLARE FUNCTION Total (Count)

DIM SHARED Arr(TWICE)
DIM SHARED Used AS INTEGER
DIM SHARED Txt$

Txt$ = GREETING$
Fill TWICE
R = Total(TWICE)
S = Used
M = MASK

SUB Fill (Count)
  FOR I = 1 TO Count
    Arr(I) = I * N
    Used = Used + 1
  NEXT I
END SUB

FUNCTION Total (Count)
  T = 0
  FOR I = 1 TO Count
    T = T + Arr(I)
  NEXT I
  Total = T
END FUNCTION
