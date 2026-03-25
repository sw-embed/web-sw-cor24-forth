; forth.s — tf24a DTC Forth: Phase 1 (bootstrap) + Phase 2 (threaded code)
; COR24 DTC Forth kernel
;
; Register allocation (frozen):
;   r0 = W (work/scratch)
;   r1 = RSP (return stack pointer, grows down from 0x0F0000)
;   r2 = IP (instruction pointer for threaded code)
;   sp = DSP (data stack, hardware push/pop in EBR)
;   fp = limited scratch (many instructions don't support fp)
;
; UART: data at 0xFF0100 (-65280), status at 0xFF0101 (-65279)
;   TX busy = status bit 7, RX ready = status bit 0
;
; DTC NEXT (inlined at tail of every primitive):
;   lw r0, 0(r2)    ; W = mem[IP] — fetch code address from thread
;   add r2, 3       ; IP += cell
;   jmp (r0)        ; execute code
;
; Colon word layout (3 bytes CFA):
;   word_entry:          ; thread entries point here
;       bra do_docol     ; 2 bytes — branch to shared DOCOL
;       .byte 0          ; 1 byte padding (never executed)
;   word_pfa:            ; PFA = CFA + 3
;       .word do_xxx     ; parameter field (thread of word addresses)
;       ...
;       .word do_exit

; ============================================================
; Entry point (address 0)
; ============================================================
_start:
    la r1, 983040       ; r1 = 0x0F0000 return stack base

    ; ============================================================
    ; Phase 1: Inline Tests — print "OK\n*\n"
    ; ============================================================

    ; Test 1: Data stack + UART — print "OK\n"
    lc r0, 10           ; '\n'
    push r0
    lc r0, 75           ; 'K'
    push r0
    lc r0, 79           ; 'O'
    push r0

    la r2, -65280       ; r2 = UART base (IP not needed yet)

    ; Emit 'O'
    pop r0
    push r0
tx1:
    lb r0, 1(r2)
    cls r0, z
    brt tx1
    pop r0
    sb r0, 0(r2)

    ; Emit 'K'
    pop r0
    push r0
tx2:
    lb r0, 1(r2)
    cls r0, z
    brt tx2
    pop r0
    sb r0, 0(r2)

    ; Emit '\n'
    pop r0
    push r0
tx3:
    lb r0, 1(r2)
    cls r0, z
    brt tx3
    pop r0
    sb r0, 0(r2)

    ; Test 2: Return stack — push 42, clear, pop, emit '*'
    lc r0, 42
    add r1, -3
    sw r0, 0(r1)
    lc r0, 0
    lw r0, 0(r1)
    add r1, 3

    push r0
tx4:
    lb r0, 1(r2)
    cls r0, z
    brt tx4
    pop r0
    sb r0, 0(r2)

    ; Emit '\n'
    lc r0, 10
    push r0
tx5:
    lb r0, 1(r2)
    cls r0, z
    brt tx5
    pop r0
    sb r0, 0(r2)

    ; ============================================================
    ; Phase 2: Launch threaded code test
    ; ============================================================
    la r2, test_thread  ; IP = start of test thread
    ; NEXT — bootstrap into threaded execution
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

; ============================================================
; Primitives
; ============================================================

; ------------------------------------------------------------
; EMIT ( c -- ) : Write character to UART with TX busy-wait
; ------------------------------------------------------------
do_emit:
    pop r0              ; r0 = character
    add r1, -3
    sw r2, 0(r1)        ; save IP on return stack
    add r1, -3
    sw r0, 0(r1)        ; save byte on return stack
    la r2, -65280       ; r2 = UART base
emit_poll:
    lb r0, 1(r2)        ; status (sign-extended; bit 7 → negative)
    cls r0, z           ; C = (status < 0) = TX busy
    brt emit_poll
    lw r0, 0(r1)        ; restore byte
    add r1, 3
    sb r0, 0(r2)        ; write byte to UART TX
    lw r2, 0(r1)        ; restore IP
    add r1, 3
    ; NEXT
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

; ------------------------------------------------------------
; KEY ( -- c ) : Read character from UART with RX busy-wait
; ------------------------------------------------------------
do_key:
    add r1, -3
    sw r2, 0(r1)        ; save IP on return stack
key_poll:
    la r0, -65280       ; UART base
    lbu r0, 1(r0)       ; status byte (zero-extended)
    lcu r2, 1           ; bit 0 mask
    and r0, r2          ; isolate RX ready bit
    ceq r0, z           ; C = (not ready)
    brt key_poll
    la r0, -65280       ; reload UART base
    lbu r0, 0(r0)       ; read byte
    push r0
    lw r2, 0(r1)        ; restore IP
    add r1, 3
    ; NEXT
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

; ------------------------------------------------------------
; EXIT ( -- ) : End colon definition, pop IP from return stack
; ------------------------------------------------------------
do_exit:
    lw r2, 0(r1)        ; restore IP from return stack
    add r1, 3
    ; NEXT
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

; ------------------------------------------------------------
; LIT ( -- x ) : Push inline literal from thread
; ------------------------------------------------------------
do_lit:
    lw r0, 0(r2)        ; r0 = literal at IP
    add r2, 3           ; IP past literal
    push r0
    ; NEXT
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

; ------------------------------------------------------------
; BRANCH ( -- ) : Unconditional branch, offset at IP
; ------------------------------------------------------------
do_branch:
    lw r0, 0(r2)        ; r0 = signed offset
    add r2, r0           ; IP += offset
    ; NEXT
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

; ------------------------------------------------------------
; 0BRANCH ( flag -- ) : Branch if TOS is zero
; ------------------------------------------------------------
do_zbranch:
    pop r0               ; r0 = flag
    ceq r0, z            ; C = (flag == 0)
    brt zbr_take         ; if zero, take branch
    add r2, 3            ; skip offset cell
    ; NEXT
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)
zbr_take:
    lw r0, 0(r2)        ; r0 = offset
    add r2, r0           ; IP += offset
    ; NEXT
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

; ============================================================
; Arithmetic Primitives
; ============================================================

; ------------------------------------------------------------
; + ( n1 n2 -- n1+n2 )
; ------------------------------------------------------------
do_plus:
    pop fp               ; fp = TOS (n2)
    pop r0               ; r0 = NOS (n1)
    add r0, fp           ; r0 = n1 + n2
    push r0
    ; NEXT
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

; ------------------------------------------------------------
; - ( n1 n2 -- n1-n2 )
; Uses r2 as scratch (save/restore IP)
; ------------------------------------------------------------
do_minus:
    add r1, -3
    sw r2, 0(r1)        ; save IP
    pop r2               ; r2 = TOS (n2)
    pop r0               ; r0 = NOS (n1)
    sub r0, r2           ; r0 = n1 - n2
    push r0
    lw r2, 0(r1)        ; restore IP
    add r1, 3
    ; NEXT
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

; ------------------------------------------------------------
; AND ( n1 n2 -- n1&n2 )
; ------------------------------------------------------------
do_and:
    add r1, -3
    sw r2, 0(r1)
    pop r2               ; r2 = TOS
    pop r0               ; r0 = NOS
    and r0, r2
    push r0
    lw r2, 0(r1)
    add r1, 3
    ; NEXT
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

; ------------------------------------------------------------
; OR ( n1 n2 -- n1|n2 )
; ------------------------------------------------------------
do_or:
    add r1, -3
    sw r2, 0(r1)
    pop r2
    pop r0
    or r0, r2
    push r0
    lw r2, 0(r1)
    add r1, 3
    ; NEXT
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

; ------------------------------------------------------------
; XOR ( n1 n2 -- n1^n2 )
; ------------------------------------------------------------
do_xor:
    add r1, -3
    sw r2, 0(r1)
    pop r2
    pop r0
    xor r0, r2
    push r0
    lw r2, 0(r1)
    add r1, 3
    ; NEXT
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

; ------------------------------------------------------------
; = ( n1 n2 -- flag ) : -1 if equal, 0 otherwise
; ------------------------------------------------------------
do_equal:
    add r1, -3
    sw r2, 0(r1)
    pop r2               ; r2 = TOS
    pop r0               ; r0 = NOS
    ceq r0, r2           ; C = (n1 == n2)
    lc r0, 0             ; assume false
    brf eq_done
    lc r0, -1            ; true = -1 (0xFFFFFF)
eq_done:
    push r0
    lw r2, 0(r1)
    add r1, 3
    ; NEXT
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

; ------------------------------------------------------------
; < ( n1 n2 -- flag ) : -1 if n1 < n2 (signed), 0 otherwise
; ------------------------------------------------------------
do_less:
    add r1, -3
    sw r2, 0(r1)
    pop r2               ; r2 = TOS (n2)
    pop r0               ; r0 = NOS (n1)
    cls r0, r2           ; C = (n1 < n2) signed
    lc r0, 0
    brf lt_done
    lc r0, -1
lt_done:
    push r0
    lw r2, 0(r1)
    add r1, 3
    ; NEXT
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

; ------------------------------------------------------------
; 0= ( n -- flag ) : -1 if zero, 0 otherwise
; ------------------------------------------------------------
do_zequ:
    pop r0
    ceq r0, z            ; C = (n == 0)
    lc r0, 0
    brf zeq_done
    lc r0, -1
zeq_done:
    push r0
    ; NEXT
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

; ============================================================
; Stack Primitives
; ============================================================

; ------------------------------------------------------------
; DROP ( x -- )
; ------------------------------------------------------------
do_drop:
    pop r0
    ; NEXT
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

; ------------------------------------------------------------
; DUP ( x -- x x )
; ------------------------------------------------------------
do_dup:
    pop r0
    push r0
    push r0
    ; NEXT
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

; ------------------------------------------------------------
; SWAP ( x1 x2 -- x2 x1 )
; ------------------------------------------------------------
do_swap:
    pop r0               ; r0 = x2 (TOS)
    pop fp               ; fp = x1 (NOS)
    push r0              ; push x2 (becomes NOS)
    push fp              ; push x1 (becomes TOS)
    ; NEXT
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

; ------------------------------------------------------------
; OVER ( x1 x2 -- x1 x2 x1 )
; ------------------------------------------------------------
do_over:
    pop r0               ; r0 = x2
    pop fp               ; fp = x1
    push fp              ; x1 back
    push r0              ; x2 back
    push fp              ; copy of x1 on top
    ; NEXT
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

; ------------------------------------------------------------
; >R ( x -- ) ( R: -- x ) : Move data stack to return stack
; ------------------------------------------------------------
do_tor:
    pop r0
    add r1, -3
    sw r0, 0(r1)
    ; NEXT
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

; ------------------------------------------------------------
; R> ( -- x ) ( R: x -- ) : Move return stack to data stack
; ------------------------------------------------------------
do_rfrom:
    lw r0, 0(r1)
    add r1, 3
    push r0
    ; NEXT
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

; ------------------------------------------------------------
; R@ ( -- x ) ( R: x -- x ) : Copy return stack top
; ------------------------------------------------------------
do_rfetch:
    lw r0, 0(r1)
    push r0
    ; NEXT
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

; ============================================================
; Memory Primitives
; ============================================================

; ------------------------------------------------------------
; @ ( addr -- x ) : Fetch cell from address
; ------------------------------------------------------------
do_fetch:
    pop r0
    lw r0, 0(r0)
    push r0
    ; NEXT
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

; ------------------------------------------------------------
; ! ( x addr -- ) : Store cell at address
; Uses r2 as scratch (save/restore IP)
; ------------------------------------------------------------
do_store:
    add r1, -3
    sw r2, 0(r1)        ; save IP
    pop r2               ; r2 = addr (TOS)
    pop r0               ; r0 = value (NOS)
    sw r0, 0(r2)        ; store cell
    lw r2, 0(r1)        ; restore IP
    add r1, 3
    ; NEXT
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

; ------------------------------------------------------------
; C@ ( addr -- c ) : Fetch byte from address
; ------------------------------------------------------------
do_cfetch:
    pop r0
    lbu r0, 0(r0)
    push r0
    ; NEXT
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

; ------------------------------------------------------------
; C! ( c addr -- ) : Store byte at address
; Uses r2 as scratch (save/restore IP)
; ------------------------------------------------------------
do_cstore:
    add r1, -3
    sw r2, 0(r1)        ; save IP
    pop r2               ; r2 = addr
    pop r0               ; r0 = byte value
    sb r0, 0(r2)        ; store byte
    lw r2, 0(r1)        ; restore IP
    add r1, 3
    ; NEXT
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

; ============================================================
; HALT — infinite loop
; ============================================================
do_halt:
halt_loop:
    bra halt_loop

; ============================================================
; DOCOL — shared entry code for colon definitions
; After NEXT's jmp(r0), r0 = CFA address of the colon word.
; Colon words: bra do_docol + .byte 0 = 3 bytes CFA
; PFA starts at CFA+3.
; ============================================================
do_docol:
    add r1, -3
    sw r2, 0(r1)        ; push IP to return stack
    mov r2, r0           ; r2 = CFA (from NEXT's jmp)
    add r2, 3            ; r2 = PFA = CFA + 3
    ; NEXT
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

; ============================================================
; Test Colon Definitions (Phase 2)
; ============================================================

; : TEST  42 EMIT 10 EMIT ;   — prints "*\n"
test_word:
    bra do_docol
    .byte 0
    .word do_lit
    .word 42
    .word do_emit
    .word do_lit
    .word 10
    .word do_emit
    .word do_exit

; : DOUBLE  DUP + ;
double_word:
    bra do_docol
    .byte 0
    .word do_dup
    .word do_plus
    .word do_exit

; : MAIN  3 DOUBLE 48 + EMIT 10 EMIT ;   — prints "6\n"
; 3 DOUBLE → 6, then 6 + 48 = 54 = ASCII '6'
main_word:
    bra do_docol
    .byte 0
    .word do_lit
    .word 3
    .word double_word
    .word do_lit
    .word 48
    .word do_plus
    .word do_emit
    .word do_lit
    .word 10
    .word do_emit
    .word do_exit

; ============================================================
; Top-level test thread (entered from Phase 1 bootstrap)
; Runs MAIN (prints "6\n"), then TEST (prints "*\n"), then halts.
; ============================================================
test_thread:
    .word main_word
    .word test_word
    .word do_halt
