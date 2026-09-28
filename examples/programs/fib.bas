REM Fibonacci numbers in an array (arrays live in memory, elements 0..11)
REM After END: F() = 0 1 1 2 3 5 8 13 21 34 55 89, TOTAL = 232
DIM F(11)
F(0) = 0
F(1) = 1
FOR I = 2 TO 11
  F(I) = F(I - 1) + F(I - 2)
NEXT I

TOTAL = 0
FOR I = 0 TO 11
  TOTAL = TOTAL + F(I)
NEXT
END
