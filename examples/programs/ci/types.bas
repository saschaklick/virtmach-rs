REM Data types: INTEGER and STRING by suffix, AS clause and DEFSTR, scoped like the variables
DEFINT A-Z
DIM S AS STRING, N AS INTEGER, A(2) AS STRING
S = "Hel"
S = S + "lo"
A(1) = S + "!"
N = LEN(S)
M% = N + 1

DECLARE SUB Greet (W AS STRING, K AS INTEGER)
DECLARE FUNCTION Twice$ (X AS STRING)

SUB Greet(W AS STRING, K AS INTEGER)
    SHARED S AS STRING
    S = W + MID$(STR$(K), 2)
END SUB

FUNCTION Twice$(X AS STRING)
    Twice$ = X + X
END FUNCTION

DEFSTR Q
Q1 = "dq"
Q2 = Q1 + "!"
DEFINT Q
R = Q

Greet "w", 5
T$ = Twice$("ab")
