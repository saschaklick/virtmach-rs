REM SUB, FUNCTION, DEF FN, CALL, SHARED and the built-ins SWAP, SGN, MIN, MAX, RND.
REM Variables in a SUB or FUNCTION are local unless declared SHARED and start at 0
REM on every call. They are stored statically, so recursion is not possible.
REM After END: G = 12, F = 120, TOTAL = 24, C1 = 2, SQ = 13, H$ = "Hello, World!",
REM            A = 2, B = 1, S = -99, M = -11, R = 5
REQ random, string

FUNCTION GCD(A, B)
  WHILE B <> 0
    T = A MOD B
    A = B
    B = T
  WEND
  GCD = A
END FUNCTION

FUNCTION FACT(N)
  FACT = 1
  FOR I = 2 TO N
    FACT = FACT * I
  NEXT I
END FUNCTION

SUB ADDTOTAL(V)
  SHARED TOTAL
  TOTAL = TOTAL + V
END SUB

FUNCTION COUNTER
  C = C + 1
  COUNTER = C
END FUNCTION

FUNCTION GREET$(WHO$)
  GREET$ = "Hello, " + WHO$ + "!"
END FUNCTION

DEF FNSQUARE(X) = X * X

G = GCD(84, 36)
F = FACT(5)
TOTAL = 0
CALL ADDTOTAL(5)
ADDTOTAL 7
ADDTOTAL(G)
C1 = COUNTER + COUNTER
SQ = FNSQUARE(GCD(10, 4)) + FNSQUARE(3)
H$ = GREET$("World")
A = 1 : B = 2 : SWAP A, B
S = SGN(-7) * 100 + SGN(0) * 10 + SGN(9)
M = MIN(4, -2, 9) * 10 + MAX(4, -2, 9)
R = RND(5, 5)
END
