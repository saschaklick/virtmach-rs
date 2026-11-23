REM PEEK and POKE read and write cells of the VM's memory, the arrays start at address 0 here
REM After END: X = 77, Y = 5, Z = 9, W = 77
DEFINT A-Z
DIM A(4)
POKE 2, 77
X = A(2)
A(3) = 5
Y = PEEK(3)
I = 4
POKE I, 9
Z = A(4)
W = PEEK(1 + 1)
END
