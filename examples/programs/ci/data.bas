REM DATA, READ and RESTORE with numbers and texts, DATA after the code that reads it
REM After END: A = 10, B = -20, C = 255, N$ = "Ann", AGE = 31, TOTAL = 10, FIRST = 7, AGAIN = 8, W$ = "word", Q$ = "a, b"
DEFINT A-Z
REQ math, string
DIM V(4)

READ A, B, C
READ N$, AGE
FOR I = 1 TO 4
  READ V(I)
NEXT I
FOR I = 1 TO 4
  TOTAL = TOTAL + V(I)
NEXT I
RESTORE nums
READ FIRST
RESTORE
READ X
RESTORE words
READ W$, Q$
RESTORE 200
READ AGAIN

DATA 10, -20, &HFF
DATA "Ann", 31
DATA 1, 2, 3, 4
nums: DATA 7
200 DATA 8
words: DATA word, "a, b"
END
