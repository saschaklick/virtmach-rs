REM Syntax: comments, line numbers, labels, case, string quotes, &H literals and ON ... GOTO
DEFINT A-Z
' apostrophe comment
10 A = 1 ' trailing comment
20 B = &H10 : C = 2 : REM trailing REM
start: D = A + B
IF D = 17 THEN E = 1 ELSE E = 2
if d = 18 then f = 1 else f = 2
S$ = "a;b" : T$ = "say ""hi"""
ON C GOTO l1, l2
l1: G = 1 : GOTO done
l2: G = 2
done: H = G
