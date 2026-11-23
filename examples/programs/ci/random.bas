REM RANDOMIZE starts the same sequence of random numbers again for the same seed
REM After END: SAME = -1, DIFFERENT = -1
DEFINT A-Z
REQ math
DIM A(5), B(5), C(5)
RANDOMIZE 7
FOR I = 0 TO 5 : A(I) = math.random(1, 1000) : NEXT I
RANDOMIZE 7
FOR I = 0 TO 5 : B(I) = math.random(1, 1000) : NEXT I
RANDOMIZE 8
FOR I = 0 TO 5 : C(I) = math.random(1, 1000) : NEXT I
SAME = -1 : DIFFERENT = 0
FOR I = 0 TO 5
  IF A(I) <> B(I) THEN SAME = 0
  IF A(I) <> C(I) THEN DIFFERENT = -1
NEXT I
END
