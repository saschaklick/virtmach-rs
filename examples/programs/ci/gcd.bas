10 REM Greatest common divisor in classic line-numbered style
20 REM After END: G1 = 6, G2 = 1, G3 = 12
30 A = 48 : B = 18 : GOSUB 100 : G1 = A
40 A = 17 : B = 5 : GOSUB 100 : G2 = A
50 A = 36 : B = 84 : GOSUB 100 : G3 = A
60 END
100 REM Euclid: A = GCD(A, B)
110 IF B = 0 THEN RETURN
120 T = A MOD B
130 A = B
140 B = T
150 GOTO 110
