REM Operators and literals: \, EQV, IMP, QBasic precedence, &H &O &B and strings in every kind of condition
REM After END: A = 3, B = -3, C = 3, D = 1, F = 1, Y = 5, G = -1, H = 0, I = 0, J = -1, K = -1, L = -7, M = -5,
REM            P = 2, T = 5, U = 31, V = 15, W = 10, X = 15, SC1..SC4 = 1, LOOPS = 3, SC5 = 2
DEFINT A-Z
REQ math, string
A = 7 \ 2
B = -7 \ 2
C = 2 + 6 \ 3 * 2
D = 19 MOD 5 \ 2
F = 7 MOD 3 * 2
Y = 1 + 7 \ 2 + 1

G = -1 EQV -1
H = -1 EQV 0
I = -1 IMP 0
J = 0 IMP -1
K = 0 IMP 0
L = 12 EQV 10
M = 12 IMP 10
P = 3 XOR 1 OR 1
T = NOT 0 AND 5

U = &H1F
V = &O17
W = &B1010
X = &h0f

S1$ = "abc"
IF S1$ = "abc" THEN SC1 = 1
IF S1$ <> "abd" THEN SC2 = 1
IF S1$ < "abd" THEN SC3 = 1
IF S1$ > "abd" THEN
  SC4 = 0
ELSEIF S1$ >= "abc" THEN
  SC4 = 1
END IF
WHILE S1$ <> "abcxxx"
  S1$ = S1$ + "x"
  LOOPS = LOOPS + 1
WEND
SELECT CASE S1$
  CASE "abc"
    SC5 = 1
  CASE "abcxxx", "q"
    SC5 = 2
END SELECT
