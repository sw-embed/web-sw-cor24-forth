; forth.s — tf24a DTC Forth: Phases 1-4 (bootstrap, threading, dictionary, interpreter)
; COR24 DTC Forth kernel
;
; Register allocation (frozen):
;   r0 = W (work/scratch)
;   r1 = RSP (return stack pointer, grows down from 0x0F0000)
;   r2 = IP (instruction pointer for threaded code)
;   sp = DSP (data stack, hardware push/pop in EBR)
;   fp = limited scratch (only pop/push/add-as-source work)
;
; UART: data at 0xFF0100 (-65280), status at 0xFF0101 (-65279)
;   TX busy = status bit 7, RX ready = status bit 0
;
; DTC NEXT (inlined at tail of every primitive, 5 bytes):
;   lw r0, 0(r2)    ; W = mem[IP] — fetch code address from thread
;   add r2, 3       ; IP += cell
;   jmp (r0)        ; execute code
;
; Colon word CFA formats:
;   Near (hand-assembled, within 127B of do_docol):
;     bra do_docol     ; 2 bytes
;     .byte 0          ; 1 byte pad — PFA at CFA+3
;   Far (runtime-compiled or distant):
;     push r0          ; 1 byte — save CFA on data stack
;     la r0, do_docol_far ; 4 bytes
;     jmp (r0)         ; 1 byte — PFA at CFA+6
;
; Dictionary header layout:
;   .word link    ; 3 bytes — link to previous entry (0 = end)
;   .byte flags   ; 1 byte — bit7=IMMEDIATE, bit6=HIDDEN, bits0-5=namelen
;   .byte c1..cN  ; N bytes — name characters
;   (CFA follows immediately: CFA = entry + 4 + namelen)

; ============================================================
; Entry point (address 0)
; ============================================================
_start:
    la r1, 983040       ; r1 = 0x0F0000 return stack base

    ; Initialize system variables (r0, r2 free before Phase 1)
    la r2, entry_quit
    la r0, var_latest_val
    sw r2, 0(r0)        ; LATEST = last dictionary entry
    la r2, dict_end
    la r0, var_here_val
    sw r2, 0(r0)        ; HERE = first free byte

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
    ; Launch threaded code tests (Phase 2 + Phase 3)
    ; ============================================================
    la r2, test_thread  ; IP = start of test thread
    ; NEXT — bootstrap into threaded execution
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

; ============================================================
; DOCOL — shared entry for colon definitions
; ============================================================

; Near DOCOL: CFA is "bra do_docol; .byte 0" (3 bytes), r0 = CFA from NEXT
do_docol:
    add r1, -3
    sw r2, 0(r1)        ; push IP to return stack
    mov r2, r0           ; r2 = CFA (from NEXT's jmp)
    add r2, 3            ; r2 = PFA = CFA + 3
    ; NEXT
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

; Far DOCOL: CFA is "push r0; la r0, do_docol_far; jmp (r0)" (6 bytes)
; CFA address was pushed to data stack by "push r0" in the CFA
do_docol_far:
    add r1, -3
    sw r2, 0(r1)        ; push IP to return stack
    pop r2               ; r2 = CFA (from data stack)
    add r2, 6            ; r2 = PFA = CFA + 6
    ; NEXT
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

; ============================================================
; Primitives with Dictionary Headers
; ============================================================
; Chain: entry_emit(link=0) → entry_key → ... → entry_immediate(LATEST)

; ------------------------------------------------------------
; EMIT ( c -- ) : Write character to UART with TX busy-wait
; ------------------------------------------------------------
entry_emit:
    .word 0
    .byte 4
    .byte 69, 77, 73, 84
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
entry_key:
    .word entry_emit
    .byte 3
    .byte 75, 69, 89
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
entry_exit:
    .word entry_key
    .byte 4
    .byte 69, 88, 73, 84
do_exit:
    lw r2, 0(r1)        ; restore IP from return stack
    add r1, 3
    ; NEXT
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

; ------------------------------------------------------------
; LIT ( -- x ) : Push inline literal from thread [HIDDEN]
; ------------------------------------------------------------
entry_lit:
    .word entry_exit
    .byte 67
    .byte 76, 73, 84
do_lit:
    lw r0, 0(r2)        ; r0 = literal at IP
    add r2, 3           ; IP past literal
    push r0
    ; NEXT
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

; ------------------------------------------------------------
; BRANCH ( -- ) : Unconditional relative branch [HIDDEN]
; ------------------------------------------------------------
entry_branch:
    .word entry_lit
    .byte 70
    .byte 66, 82, 65, 78, 67, 72
do_branch:
    lw r0, 0(r2)        ; r0 = signed offset
    add r2, r0           ; IP += offset
    ; NEXT
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

; ------------------------------------------------------------
; 0BRANCH ( flag -- ) : Branch if TOS is zero [HIDDEN]
; ------------------------------------------------------------
entry_zbranch:
    .word entry_branch
    .byte 71
    .byte 48, 66, 82, 65, 78, 67, 72
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

; + ( n1 n2 -- n1+n2 )
entry_plus:
    .word entry_zbranch
    .byte 1
    .byte 43
do_plus:
    pop fp               ; fp = n2
    pop r0               ; r0 = n1
    add r0, fp           ; r0 = n1 + n2
    push r0
    ; NEXT
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

; - ( n1 n2 -- n1-n2 )
entry_minus:
    .word entry_plus
    .byte 1
    .byte 45
do_minus:
    add r1, -3
    sw r2, 0(r1)        ; save IP
    pop r2               ; r2 = n2
    pop r0               ; r0 = n1
    sub r0, r2           ; r0 = n1 - n2
    push r0
    lw r2, 0(r1)        ; restore IP
    add r1, 3
    ; NEXT
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

; AND ( n1 n2 -- n1&n2 )
entry_and:
    .word entry_minus
    .byte 3
    .byte 65, 78, 68
do_and:
    add r1, -3
    sw r2, 0(r1)
    pop r2
    pop r0
    and r0, r2
    push r0
    lw r2, 0(r1)
    add r1, 3
    ; NEXT
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

; OR ( n1 n2 -- n1|n2 )
entry_or:
    .word entry_and
    .byte 2
    .byte 79, 82
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

; XOR ( n1 n2 -- n1^n2 )
entry_xor:
    .word entry_or
    .byte 3
    .byte 88, 79, 82
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

; = ( n1 n2 -- flag ) : -1 if equal, 0 otherwise
entry_equal:
    .word entry_xor
    .byte 1
    .byte 61
do_equal:
    add r1, -3
    sw r2, 0(r1)
    pop r2
    pop r0
    ceq r0, r2           ; C = (n1 == n2)
    lc r0, 0
    brf eq_done
    lc r0, -1
eq_done:
    push r0
    lw r2, 0(r1)
    add r1, 3
    ; NEXT
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

; < ( n1 n2 -- flag ) : -1 if n1 < n2 signed, 0 otherwise
entry_less:
    .word entry_equal
    .byte 1
    .byte 60
do_less:
    add r1, -3
    sw r2, 0(r1)
    pop r2               ; n2
    pop r0               ; n1
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

; 0= ( n -- flag ) : -1 if zero, 0 otherwise
entry_zequ:
    .word entry_less
    .byte 2
    .byte 48, 61
do_zequ:
    pop r0
    ceq r0, z
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

; DROP ( x -- )
entry_drop:
    .word entry_zequ
    .byte 4
    .byte 68, 82, 79, 80
do_drop:
    pop r0
    ; NEXT
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

; DUP ( x -- x x )
entry_dup:
    .word entry_drop
    .byte 3
    .byte 68, 85, 80
do_dup:
    pop r0
    push r0
    push r0
    ; NEXT
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

; SWAP ( x1 x2 -- x2 x1 )
entry_swap:
    .word entry_dup
    .byte 4
    .byte 83, 87, 65, 80
do_swap:
    pop r0               ; x2
    pop fp               ; x1
    push r0              ; x2
    push fp              ; x1
    ; NEXT
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

; OVER ( x1 x2 -- x1 x2 x1 )
entry_over:
    .word entry_swap
    .byte 4
    .byte 79, 86, 69, 82
do_over:
    pop r0               ; x2
    pop fp               ; x1
    push fp              ; x1
    push r0              ; x2
    push fp              ; x1 copy
    ; NEXT
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

; >R ( x -- ) ( R: -- x )
entry_tor:
    .word entry_over
    .byte 2
    .byte 62, 82
do_tor:
    pop r0
    add r1, -3
    sw r0, 0(r1)
    ; NEXT
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

; R> ( -- x ) ( R: x -- )
entry_rfrom:
    .word entry_tor
    .byte 2
    .byte 82, 62
do_rfrom:
    lw r0, 0(r1)
    add r1, 3
    push r0
    ; NEXT
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

; R@ ( -- x ) ( R: x -- x )
entry_rfetch:
    .word entry_rfrom
    .byte 2
    .byte 82, 64
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

; @ ( addr -- x ) : Fetch cell from address
entry_fetch:
    .word entry_rfetch
    .byte 1
    .byte 64
do_fetch:
    pop r0
    lw r0, 0(r0)
    push r0
    ; NEXT
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

; ! ( x addr -- ) : Store cell at address
entry_store:
    .word entry_fetch
    .byte 1
    .byte 33
do_store:
    add r1, -3
    sw r2, 0(r1)        ; save IP
    pop r2               ; addr
    pop r0               ; value
    sw r0, 0(r2)
    lw r2, 0(r1)        ; restore IP
    add r1, 3
    ; NEXT
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

; C@ ( addr -- c ) : Fetch byte from address
entry_cfetch:
    .word entry_store
    .byte 2
    .byte 67, 64
do_cfetch:
    pop r0
    lbu r0, 0(r0)
    push r0
    ; NEXT
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

; C! ( c addr -- ) : Store byte at address
entry_cstore:
    .word entry_cfetch
    .byte 2
    .byte 67, 33
do_cstore:
    add r1, -3
    sw r2, 0(r1)        ; save IP
    pop r2               ; addr
    pop r0               ; byte value
    sb r0, 0(r2)
    lw r2, 0(r1)        ; restore IP
    add r1, 3
    ; NEXT
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

; ============================================================
; HALT — infinite loop (not in dictionary, just a code target)
; ============================================================
do_halt:
halt_loop:
    bra halt_loop

; ============================================================
; Phase 3: New Primitives
; ============================================================

; ------------------------------------------------------------
; EXECUTE ( cfa -- ) : Execute word at cfa
; ------------------------------------------------------------
entry_execute:
    .word entry_cstore
    .byte 7
    .byte 69, 88, 69, 67, 85, 84, 69
do_execute:
    pop r0
    jmp (r0)

; ------------------------------------------------------------
; HERE ( -- addr ) : Push address of HERE variable
; ------------------------------------------------------------
entry_here:
    .word entry_execute
    .byte 4
    .byte 72, 69, 82, 69
do_here:
    la r0, var_here_val
    push r0
    ; NEXT
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

; ------------------------------------------------------------
; LATEST ( -- addr ) : Push address of LATEST variable
; ------------------------------------------------------------
entry_latest:
    .word entry_here
    .byte 6
    .byte 76, 65, 84, 69, 83, 84
do_latest:
    la r0, var_latest_val
    push r0
    ; NEXT
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

; ------------------------------------------------------------
; STATE ( -- addr ) : Push address of STATE variable
; ------------------------------------------------------------
entry_state:
    .word entry_latest
    .byte 5
    .byte 83, 84, 65, 84, 69
do_state:
    la r0, var_state_val
    push r0
    ; NEXT
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

; ------------------------------------------------------------
; BASE ( -- addr ) : Push address of BASE variable
; ------------------------------------------------------------
entry_base:
    .word entry_state
    .byte 4
    .byte 66, 65, 83, 69
do_base:
    la r0, var_base_val
    push r0
    ; NEXT
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

; ------------------------------------------------------------
; , ( x -- ) : Store cell at HERE, advance HERE by 3
; ------------------------------------------------------------
entry_comma:
    .word entry_base
    .byte 1
    .byte 44
do_comma:
    add r1, -3
    sw r2, 0(r1)        ; save IP
    la r0, var_here_val
    lw r2, 0(r0)        ; r2 = HERE
    pop r0               ; r0 = value
    sw r0, 0(r2)        ; mem[HERE] = x
    add r2, 3            ; HERE += 3
    la r0, var_here_val
    sw r2, 0(r0)        ; update HERE
    lw r2, 0(r1)        ; restore IP
    add r1, 3
    ; NEXT
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

; ------------------------------------------------------------
; C, ( c -- ) : Store byte at HERE, advance HERE by 1
; ------------------------------------------------------------
entry_ccomma:
    .word entry_comma
    .byte 2
    .byte 67, 44
do_ccomma:
    add r1, -3
    sw r2, 0(r1)        ; save IP
    la r0, var_here_val
    lw r2, 0(r0)        ; r2 = HERE
    pop r0               ; r0 = byte
    sb r0, 0(r2)        ; mem[HERE] = c
    add r2, 1            ; HERE += 1
    la r0, var_here_val
    sw r2, 0(r0)        ; update HERE
    lw r2, 0(r1)        ; restore IP
    add r1, 3
    ; NEXT
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

; ------------------------------------------------------------
; ALLOT ( n -- ) : Advance HERE by n bytes
; ------------------------------------------------------------
entry_allot:
    .word entry_ccomma
    .byte 5
    .byte 65, 76, 76, 79, 84
do_allot:
    add r1, -3
    sw r2, 0(r1)        ; save IP
    la r0, var_here_val
    lw r2, 0(r0)        ; r2 = HERE
    pop r0               ; r0 = n
    add r2, r0           ; HERE += n
    la r0, var_here_val
    sw r2, 0(r0)        ; update HERE
    lw r2, 0(r1)        ; restore IP
    add r1, 3
    ; NEXT
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

; ------------------------------------------------------------
; [ ( -- ) : Enter interpret mode [IMMEDIATE]
; ------------------------------------------------------------
entry_lbrac:
    .word entry_allot
    .byte 129
    .byte 91
do_lbrac:
    add r1, -3
    sw r2, 0(r1)
    la r2, var_state_val
    lc r0, 0
    sw r0, 0(r2)        ; STATE = 0
    lw r2, 0(r1)
    add r1, 3
    ; NEXT
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

; ------------------------------------------------------------
; ] ( -- ) : Enter compile mode
; ------------------------------------------------------------
entry_rbrac:
    .word entry_lbrac
    .byte 1
    .byte 93
do_rbrac:
    add r1, -3
    sw r2, 0(r1)
    la r2, var_state_val
    lc r0, -1
    sw r0, 0(r2)        ; STATE = -1
    lw r2, 0(r1)
    add r1, 3
    ; NEXT
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

; ============================================================
; FIND ( c-addr -- c-addr 0 | cfa 1 | cfa -1 )
; Search dictionary for counted string at c-addr.
; Returns cfa and flag (1=immediate, -1=normal) or 0 if not found.
;
; Uses data stack to pass entry pointer between iterations (avoids
; long backward branches). RS base frame:
;   r1+0  = search_start (c-addr + 1)
;   r1+3  = search_len
;   r1+6  = c-addr (for not-found return)
;   r1+9  = saved IP
; ============================================================
entry_find:
    .word entry_rbrac
    .byte 4
    .byte 70, 73, 78, 68
do_find:
    add r1, -3
    sw r2, 0(r1)        ; save IP          RS: [IP]

    pop r0               ; r0 = c-addr
    add r1, -3
    sw r0, 0(r1)        ; save c-addr      RS: [c-addr, IP]

    lbu r2, 0(r0)       ; r2 = search length
    add r1, -3
    sw r2, 0(r1)        ; save search_len  RS: [search_len, c-addr, IP]

    add r0, 1           ; r0 = search name start
    add r1, -3
    sw r0, 0(r1)        ; save search_start RS: [ss, sl, ca, IP]

    ; Load LATEST and push on DS for find_loop
    la r0, var_latest_val
    lw r0, 0(r0)
    push r0              ; DS: [entry]

find_loop:
    ; Entry pointer is on data stack
    pop r0               ; r0 = entry (0 = end of chain)
    ceq r0, z
    brf find_have_entry

    ; === Not found (inline handler) ===
    lw r0, 6(r1)        ; c-addr (RS offset 6)
    add r1, 9           ; pop ss, sl, ca. RS: [IP]
    push r0              ; DS: [c-addr]
    lc r0, 0
    push r0              ; DS: [0, c-addr]
    lw r2, 0(r1)
    add r1, 3
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

find_have_entry:
    ; r0 = entry pointer
    ; Save entry on RS
    add r1, -3
    sw r0, 0(r1)        ; RS: [entry, ss, sl, ca, IP]

    ; Load flags_len byte
    lbu r2, 3(r0)       ; r2 = flags_len
    add r1, -3
    sw r2, 0(r1)        ; RS: [fl, entry, ss, sl, ca, IP]

    ; Check HIDDEN (bit 6): if hidden, skip via la+jmp
    lcu r0, 64
    and r0, r2
    ceq r0, z
    brt find_not_hidden
    la r0, find_skip_entry
    jmp (r0)
find_not_hidden:

    ; Extract name_len = flags_len & 0x3F
    lw r0, 0(r1)        ; r0 = flags_len
    lcu r2, 63
    and r0, r2           ; r0 = name_len

    ; Compare with search_len
    lw r2, 9(r1)        ; r2 = search_len (RS offset 9)
    ceq r0, r2
    brt find_len_match
    la r0, find_skip_entry
    jmp (r0)
find_len_match:

    ; === Lengths match — compare characters ===
    ; r0 = name_len = counter

    ; Save counter
    add r1, -3
    sw r0, 0(r1)        ; RS: [ctr, fl, entry, ss, sl, ca, IP]

    ; ename_ptr = entry + 4
    lw r0, 6(r1)        ; entry (RS offset 6)
    add r0, 4
    add r1, -3
    sw r0, 0(r1)        ; RS: [ep, ctr, fl, entry, ss, sl, ca, IP]

    ; sname_ptr = search_start
    lw r0, 12(r1)       ; search_start (RS offset 12)
    add r1, -3
    sw r0, 0(r1)        ; RS: [sp, ep, ctr, fl, entry, ss, sl, ca, IP]

find_cmp_loop:
    lw r0, 6(r1)        ; counter (RS offset 6)
    ceq r0, z
    brt find_matched

    ; Load entry char
    lw r0, 3(r1)        ; ename_ptr (RS offset 3)
    lbu r2, 0(r0)       ; r2 = entry char

    ; Load search char
    lw r0, 0(r1)        ; sname_ptr (RS offset 0)
    lbu r0, 0(r0)       ; r0 = search char

    ceq r0, r2
    brf find_char_fail

    ; Advance ename_ptr
    lw r0, 3(r1)
    add r0, 1
    sw r0, 3(r1)
    ; Advance sname_ptr
    lw r0, 0(r1)
    add r0, 1
    sw r0, 0(r1)
    ; Decrement counter
    lw r0, 6(r1)
    add r0, -1
    sw r0, 6(r1)
    bra find_cmp_loop

find_char_fail:
    add r1, 9           ; pop sp, ep, ctr
    ; RS: [fl, entry, ss, sl, ca, IP]
    ; Fall through to find_skip_entry

find_skip_entry:
    ; RS: [fl, entry, ss, sl, ca, IP]
    add r1, 3           ; pop flags_len
    lw r0, 0(r1)        ; entry
    add r1, 3           ; pop entry. RS: [ss, sl, ca, IP]
    lw r0, 0(r0)        ; follow link
    push r0              ; push next entry on DS
    la r0, find_loop
    jmp (r0)             ; back to loop (too far for bra)

find_matched:
    add r1, 9           ; pop sp, ep, ctr
    ; RS: [fl, entry, ss, sl, ca, IP]

    ; Read flags_len and entry BEFORE cleaning RS
    lw r0, 0(r1)        ; r0 = flags_len
    push r0              ; save flags_len on DS
    lw r2, 3(r1)        ; r2 = entry
    add r1, 15          ; pop fl, entry, ss, sl, ca. RS: [IP]

    ; Compute name_len = flags_len & 0x3F
    pop r0               ; r0 = flags_len
    push r0              ; keep flags_len on DS for IMMEDIATE check
    push r2              ; save entry. DS: [entry, flags_len]
    lcu r2, 63
    and r0, r2           ; r0 = name_len
    pop r2               ; r2 = entry. DS: [flags_len]

    ; CFA = entry + 4 + name_len
    add r2, 4
    add r2, r0           ; r2 = CFA
    push r2              ; DS: [CFA, flags_len]

    ; Check IMMEDIATE (bit 7 of original flags_len)
    pop r2               ; r2 = CFA (save temporarily)
    pop r0               ; r0 = flags_len. DS: []
    push r2              ; CFA back on DS: [CFA]
    lcu r2, 128
    and r0, r2           ; r0 = flags_len & 128
    ceq r0, z
    brt find_normal
    lc r0, 1             ; IMMEDIATE → flag = 1
    bra find_push_flag
find_normal:
    lc r0, -1            ; normal → flag = -1
find_push_flag:
    push r0              ; DS: [flag, CFA]

    ; Restore IP and NEXT
    lw r2, 0(r1)
    add r1, 3
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

; ============================================================
; WORD, CREATE, COLON, SEMICOLON, IMMEDIATE
; ============================================================

; ------------------------------------------------------------
; WORD ( -- c-addr ) : Read space-delimited word from UART
; Stores counted string at word_buffer, returns its address
; ------------------------------------------------------------
entry_word:
    .word entry_find
    .byte 4
    .byte 87, 79, 82, 68
do_word:
    add r1, -3
    sw r2, 0(r1)        ; save IP. RS: [IP]

    ; Check if previous call ended on newline
    la r0, word_eol_flag
    lbu r0, 0(r0)
    ceq r0, z
    brt word_no_eol
    ; Clear flag and return empty
    la r0, word_eol_flag
    lc r2, 0
    sb r2, 0(r0)
    la r0, word_buffer
    lc r2, 0
    sb r2, 0(r0)
    add r1, 3           ; pop IP wait... we haven't pushed buf_ptr yet
    ; RS: [IP]. Push word_buffer, restore IP, NEXT
    push r0
    lw r2, 0(r1)
    add r1, 3
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)
word_no_eol:

    ; Init buffer pointer (past count byte)
    la r0, word_buffer
    add r0, 1
    add r1, -3
    sw r0, 0(r1)        ; RS: [buf_ptr, IP]

    ; --- Skip leading spaces (NOT newlines) ---
    ; Spaces (32) are skipped. Newline (10, 13) → return empty.
    ; Any other char < 32 is skipped (control chars).
word_skip:
    la r0, -65280        ; UART base
word_skip_rx:
    lbu r2, 1(r0)       ; r2 = status
    lcu r0, 1
    and r2, r0           ; r2 = RX ready bit
    ceq r2, z
    brt word_skip_rx2    ; not ready, retry
    la r0, -65280
    lbu r0, 0(r0)       ; r0 = char
    ; Check for newline (10) → return empty
    lcu r2, 10
    ceq r0, r2
    brt word_empty       ; newline → empty token
    ; Check for CR (13) → return empty
    lcu r2, 13
    ceq r0, r2
    brt word_empty
    ; Skip spaces and other control chars
    lcu r2, 33
    clu r0, r2           ; C = (char < 33)
    brt word_skip        ; skip, read another
    bra word_store       ; got a real char
word_skip_rx2:
    la r0, -65280
    bra word_skip_rx

word_empty:
    ; Return empty counted string (length=0)
    la r0, word_buffer
    lc r2, 0
    sb r2, 0(r0)        ; store count=0
    add r1, 3           ; pop buf_ptr. RS: [IP]
    push r0              ; push word_buffer address
    lw r2, 0(r1)
    add r1, 3
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

    ; --- Store char and read more ---
word_store:
    ; r0 = char to store
    lw r2, 0(r1)        ; r2 = buf_ptr
    sb r0, 0(r2)        ; store char
    add r2, 1
    sw r2, 0(r1)        ; update buf_ptr

word_read:
    la r0, -65280
word_read_rx:
    lbu r2, 1(r0)
    lcu r0, 1
    and r2, r0
    ceq r2, z
    brt word_read_rx2
    la r0, -65280
    lbu r0, 0(r0)       ; r0 = char
    lcu r2, 33
    clu r0, r2           ; C = (char < 33)
    brt word_end         ; delimiter found
    bra word_store       ; store and continue
word_read_rx2:
    la r0, -65280
    bra word_read_rx

word_end:
    ; r0 = delimiter char that ended the word
    ; Check if delimiter is newline → set eol flag
    push r0              ; save delimiter
    lcu r2, 10
    ceq r0, r2
    brt word_set_eol
    lcu r2, 13
    ceq r0, r2
    brt word_set_eol
    bra word_no_set_eol
word_set_eol:
    la r0, word_eol_flag
    lc r2, 1
    sb r2, 0(r0)
word_no_set_eol:
    pop r0               ; discard delimiter

    ; Compute length = buf_ptr - (word_buffer + 1)
    lw r2, 0(r1)        ; r2 = final buf_ptr
    add r1, 3           ; pop buf_ptr. RS: [IP]
    la r0, word_buffer
    add r0, 1           ; r0 = data start
    sub r2, r0           ; r2 = length
    la r0, word_buffer
    sb r2, 0(r0)        ; store count byte
    push r0              ; push word_buffer address

    ; Restore IP and NEXT
    lw r2, 0(r1)
    add r1, 3
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

; ------------------------------------------------------------
; CREATE ( -- ) : Read name, build dictionary header at HERE
; Reads next word from UART input, builds link+flags+name at HERE.
; Updates LATEST. Does NOT write CFA — caller does that.
; ------------------------------------------------------------
entry_create:
    .word entry_word
    .byte 6
    .byte 67, 82, 69, 65, 84, 69
do_create:
    add r1, -3
    sw r2, 0(r1)        ; save IP. RS: [IP]

    ; --- Inline word reading (same as WORD) ---
    la r0, word_buffer
    add r0, 1
    add r1, -3
    sw r0, 0(r1)        ; RS: [buf_ptr, IP]

create_skip:
    la r0, -65280
create_skip_rx:
    lbu r2, 1(r0)
    lcu r0, 1
    and r2, r0
    ceq r2, z
    brt create_skip_rx2
    la r0, -65280
    lbu r0, 0(r0)
    lcu r2, 33
    clu r0, r2
    brt create_skip
    bra create_store
create_skip_rx2:
    la r0, -65280
    bra create_skip_rx

create_store:
    lw r2, 0(r1)
    sb r0, 0(r2)
    add r2, 1
    sw r2, 0(r1)

create_read:
    la r0, -65280
create_read_rx:
    lbu r2, 1(r0)
    lcu r0, 1
    and r2, r0
    ceq r2, z
    brt create_read_rx2
    la r0, -65280
    lbu r0, 0(r0)
    lcu r2, 33
    clu r0, r2
    brt create_read_done
    bra create_store
create_read_rx2:
    la r0, -65280
    bra create_read_rx

create_read_done:
    ; Compute name length
    lw r2, 0(r1)        ; r2 = final buf_ptr
    add r1, 3           ; pop buf_ptr. RS: [IP]
    la r0, word_buffer
    add r0, 1
    sub r2, r0           ; r2 = name length
    la r0, word_buffer
    sb r2, 0(r0)        ; store count

    ; --- Build header at HERE ---
    ; Save name length
    add r1, -3
    sw r2, 0(r1)        ; RS: [name_len, IP]

    ; Load HERE
    la r0, var_here_val
    lw r2, 0(r0)        ; r2 = HERE (destination)

    ; Save HERE (= new entry address) for LATEST update
    add r1, -3
    sw r2, 0(r1)        ; RS: [new_entry, name_len, IP]

    ; Write link field = current LATEST
    la r0, var_latest_val
    lw r0, 0(r0)        ; r0 = LATEST
    sw r0, 0(r2)        ; mem[HERE] = link
    add r2, 3           ; past link

    ; Write flags_len = name_len (no flags)
    lw r0, 3(r1)        ; r0 = name_len (at RS offset 3)
    sb r0, 0(r2)        ; mem[HERE+3] = flags_len
    add r2, 1           ; past flags_len

    ; Copy name chars from word_buffer+1 to HERE+4
    lw r0, 3(r1)        ; r0 = name_len (counter)
    add r1, -3
    sw r0, 0(r1)        ; RS: [counter, new_entry, name_len, IP]
    la r0, word_buffer
    add r0, 1           ; r0 = source
    add r1, -3
    sw r0, 0(r1)        ; RS: [src, counter, new_entry, name_len, IP]

create_copy:
    lw r0, 3(r1)        ; r0 = counter
    ceq r0, z
    brt create_copy_done
    lw r0, 0(r1)        ; r0 = src
    lbu r0, 0(r0)       ; r0 = char
    sb r0, 0(r2)        ; store at dest (r2)
    add r2, 1           ; dest++
    lw r0, 0(r1)        ; src
    add r0, 1
    sw r0, 0(r1)        ; src++
    lw r0, 3(r1)        ; counter
    add r0, -1
    sw r0, 3(r1)        ; counter--
    bra create_copy

create_copy_done:
    add r1, 6           ; pop src, counter. RS: [new_entry, name_len, IP]

    ; Update HERE (r2 = new position after name)
    la r0, var_here_val
    sw r2, 0(r0)

    ; Update LATEST = new_entry
    lw r0, 0(r1)        ; r0 = new_entry
    add r1, 6           ; pop new_entry, name_len. RS: [IP]
    add r1, -3
    sw r2, 0(r1)        ; save r2 on RS temporarily
    la r2, var_latest_val
    sw r0, 0(r2)        ; LATEST = new_entry
    lw r2, 0(r1)        ; restore r2
    add r1, 3           ; RS: [IP]

    ; Restore IP and NEXT
    lw r2, 0(r1)
    add r1, 3
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

; ------------------------------------------------------------
; : ( -- ) : Start colon definition
; Calls CREATE, writes 6-byte far CFA, enters compile mode
; ------------------------------------------------------------
entry_colon:
    .word entry_create
    .byte 1
    .byte 58
do_colon:
    add r1, -3
    sw r2, 0(r1)        ; save IP. RS: [IP]

    ; --- Inline CREATE logic (read word, build header) ---
    ; This duplicates CREATE's word-reading and header-building.
    ; For code size, we CALL CREATE by jumping to it with a return trick.
    ; Push a return address on return stack, set IP to a thread that calls
    ; CREATE then returns. Actually, simpler: just copy CREATE's body.
    ;
    ; For now, use a colon-definition approach:
    ; We'll define do_colon as a hand-assembled colon word.
    ; This requires do_docol to be nearby... or use do_docol_far.
    ; Since we're IN a primitive, let's manually call CREATE.

    ; Save return info and call CREATE by setting up a mini thread
    ; Actually, the simplest approach: CREATE reads from UART and builds header.
    ; COLON does the same PLUS writes CFA + sets STATE.
    ; Rather than duplicating, let's store a return address and jump.

    ; Use data stack for return: push address of colon_after_create, jmp do_create
    ; But do_create uses NEXT to return, so it would follow IP, not our return addr.
    ;
    ; Alternative: set IP to point to a mini thread: [do_create, colon_continue]
    ; and let NEXT drive it.
    la r0, colon_thread
    lw r2, 0(r1)        ; restore original IP... wait, I already saved it.
    ; Actually, I want to save original IP, replace IP with colon_thread, do NEXT.
    ; Original IP is already on RS from the first sw.
    ; Set IP to colon_thread:
    la r2, colon_thread
    ; NEXT
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

; Helper thread for COLON: CREATE, then colon_write_cfa
colon_thread:
    .word do_create
    .word do_colon_cfa
    .word do_rbrac       ; enter compile mode
    .word do_exit        ; return to original IP (saved on RS by do_colon)

; Helper: write 6-byte CFA at HERE
; Writes: push r0 (0x7D), la r0 opcode (0x29), do_docol_far addr (3B), jmp r0 (0x26)
do_colon_cfa:
    add r1, -3
    sw r2, 0(r1)        ; save IP
    ; Load HERE
    la r0, var_here_val
    lw r2, 0(r0)        ; r2 = HERE (dest)
    ; Copy 6 bytes from cfa_template
    la r0, cfa_template
    push r0              ; save template addr
    lw r0, 0(r0)        ; first 3 bytes
    sw r0, 0(r2)        ; store at HERE
    add r2, 3
    pop r0               ; template addr
    add r0, 3
    lw r0, 0(r0)        ; next 3 bytes
    sw r0, 0(r2)        ; store at HERE+3
    add r2, 3
    ; Update HERE (+6)
    la r0, var_here_val
    sw r2, 0(r0)
    ; Restore IP and NEXT
    lw r2, 0(r1)
    add r1, 3
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

; Template for far colon CFA (6 bytes):
; push r0 (0x7D), la r0 (0x29), addr_lo, addr_mid, addr_hi, jmp(r0) (0x26)
cfa_template:
    .byte 125            ; push r0
    .byte 41             ; la r0 opcode
    .word do_docol_far   ; 3-byte address of do_docol_far
    .byte 38             ; jmp (r0)

; ------------------------------------------------------------
; ; ( -- ) : End colon definition [IMMEDIATE]
; Compiles EXIT, enters interpret mode
; ------------------------------------------------------------
entry_semi:
    .word entry_colon
    .byte 129
    .byte 59
do_semi:
    add r1, -3
    sw r2, 0(r1)        ; save IP
    ; Compile EXIT at HERE
    la r0, var_here_val
    lw r2, 0(r0)        ; r2 = HERE
    la r0, do_exit
    sw r0, 0(r2)        ; mem[HERE] = do_exit
    add r2, 3
    la r0, var_here_val
    sw r2, 0(r0)        ; update HERE
    ; STATE = 0 (interpreting)
    la r0, var_state_val
    lc r2, 0
    sw r2, 0(r0)
    lw r2, 0(r1)
    add r1, 3
    ; NEXT
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

; ------------------------------------------------------------
; IMMEDIATE ( -- ) : Toggle IMMEDIATE flag on most recent word
; ------------------------------------------------------------
entry_immediate:
    .word entry_semi
    .byte 9
    .byte 73, 77, 77, 69, 68, 73, 65, 84, 69
do_immediate:
    add r1, -3
    sw r2, 0(r1)        ; save IP
    la r0, var_latest_val
    lw r0, 0(r0)        ; r0 = latest entry
    add r0, 3           ; r0 = address of flags_len
    lbu r2, 0(r0)       ; r2 = flags_len
    push r0              ; save flags address
    lcu r0, 128
    xor r2, r0           ; toggle bit 7
    pop r0               ; r0 = flags address
    sb r2, 0(r0)        ; store updated flags
    lw r2, 0(r1)
    add r1, 3
    ; NEXT
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

; ============================================================
; Phase 4: LED!, DOT, interpret-only shell
; ============================================================

; ------------------------------------------------------------
; LED! ( n -- ) : Write low bit of n to LED register at 0xFF0000
; ------------------------------------------------------------
entry_led_store:
    .word entry_immediate
    .byte 4
    .byte 76, 69, 68, 33   ; "LED!"
do_led_store:
    add r1, -3
    sw r2, 0(r1)        ; save IP
    pop r0               ; n
    lcu r2, 1
    and r0, r2           ; mask to low bit
    la r2, -65536        ; 0xFF0000 LED register
    sb r0, 0(r2)
    lw r2, 0(r1)
    add r1, 3
    ; NEXT
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

; ------------------------------------------------------------
; DOT ( n -- ) : Print signed number in BASE, followed by space
; Uses repeated subtraction for division.
; ------------------------------------------------------------
entry_dot:
    .word entry_led_store
    .byte 1
    .byte 46              ; "."
do_dot:
    add r1, -3
    sw r2, 0(r1)        ; save IP. RS: [IP]
    pop r0               ; r0 = n

    ; Check negative
    cls r0, z            ; C = (n < 0)
    brf dot_pos
    ; Negate and emit '-'
    add r1, -3
    sw r0, 0(r1)        ; save n
    lc r0, 45           ; '-'
    push r0
    la r2, -65280
dot_neg_tx:
    lb r0, 1(r2)
    cls r0, z
    brt dot_neg_tx
    pop r0
    sb r0, 0(r2)
    lw r0, 0(r1)        ; restore n
    add r1, 3
    ; Negate: 0 - n
    push r0
    lc r0, 0
    pop r2
    sub r0, r2           ; r0 = -n

dot_pos:
    ; r0 = unsigned value to print
    ; Push digit ASCII chars onto data stack in reverse, count on RS
    lc r2, 0
    add r1, -3
    sw r2, 0(r1)        ; digit_count = 0. RS: [count, IP]

dot_div_loop:
    ; Divide r0 by BASE: quotient→r0, remainder→r2
    ; Save value, load BASE
    add r1, -3
    sw r0, 0(r1)        ; RS: [val, count, IP]
    la r0, var_base_val
    lw r0, 0(r0)        ; r0 = BASE
    lw r2, 0(r1)        ; r2 = value
    add r1, 3           ; pop val. RS: [count, IP]

    ; Divide: r2 / r0 → quotient in fp area, remainder in r0
    ; Use repeated subtraction
    add r1, -3
    sw r0, 0(r1)        ; save BASE. RS: [BASE, count, IP]
    lc r0, 0            ; quotient = 0, r2 = remainder

dot_sub_loop:
    push r0              ; save quotient on DS
    mov r0, r2           ; r0 = remainder
    lw r2, 0(r1)        ; r2 = BASE
    clu r0, r2           ; C = (remainder < BASE)
    brt dot_sub_done
    sub r0, r2           ; remainder -= BASE
    mov r2, r0           ; r2 = new remainder
    pop r0               ; quotient
    add r0, 1
    bra dot_sub_loop

dot_sub_done:
    mov r2, r0           ; r2 = final remainder
    pop r0               ; r0 = quotient
    add r1, 3           ; pop BASE. RS: [count, IP]

    ; Convert remainder (r2) to ASCII digit
    push r0              ; save quotient on DS
    mov r0, r2
    lcu r2, 10
    clu r0, r2           ; C = (digit < 10)
    brt dot_digit_09
    add r0, 55           ; 'A' - 10
    bra dot_push_digit
dot_digit_09:
    add r0, 48           ; '0'

dot_push_digit:
    pop r2               ; r2 = quotient
    push r0              ; push ASCII digit on DS
    lw r0, 0(r1)        ; count
    add r0, 1
    sw r0, 0(r1)        ; count++

    mov r0, r2           ; r0 = quotient
    ceq r0, z            ; done when quotient = 0
    brf dot_div_loop

    ; Emit all digits (they're on DS in reverse = correct print order)
dot_emit_loop:
    lw r0, 0(r1)        ; count
    ceq r0, z
    brt dot_emit_done
    la r2, -65280
dot_emit_tx:
    lb r0, 1(r2)
    cls r0, z
    brt dot_emit_tx
    pop r0
    sb r0, 0(r2)
    lw r0, 0(r1)
    add r0, -1
    sw r0, 0(r1)
    bra dot_emit_loop

dot_emit_done:
    add r1, 3           ; pop count. RS: [IP]
    ; Emit trailing space
    lc r0, 32
    push r0
    la r2, -65280
dot_sp_tx:
    lb r0, 1(r2)
    cls r0, z
    brt dot_sp_tx
    pop r0
    sb r0, 0(r2)
    ; Restore IP and NEXT
    lw r2, 0(r1)
    add r1, 3
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

; ------------------------------------------------------------
; NUMBER ( c-addr -- n flag ) : Parse counted string as number
; flag=0 success, flag=-1 failure. Handles leading '-'.
; Pure assembly, no sub-calls. Uses RS for locals.
; RS frame: [sign, acc, ptr, rem, saved_IP]
; ------------------------------------------------------------
entry_number:
    .word entry_dot
    .byte 6
    .byte 78, 85, 77, 66, 69, 82
do_number:
    add r1, -3
    sw r2, 0(r1)        ; RS: [IP]
    pop r0               ; r0 = c-addr
    lbu r2, 0(r0)       ; r2 = length
    ceq r2, z
    brf num_have_len
    ; Zero length = failure
    lc r0, 0
    push r0
    lc r0, -1
    push r0
    lw r2, 0(r1)
    add r1, 3
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

num_have_len:
    ; Build RS frame: [sign, acc, ptr, rem, IP]
    ; Currently RS: [IP], r0=c-addr, r2=length
    add r0, 1           ; r0 = first data char
    add r1, -3
    sw r2, 0(r1)        ; rem. RS: [rem, IP]
    add r1, -3
    sw r0, 0(r1)        ; ptr. RS: [ptr, rem, IP]
    lc r0, 0
    add r1, -3
    sw r0, 0(r1)        ; acc=0. RS: [acc, ptr, rem, IP]

    ; Check leading '-'
    lw r0, 3(r1)        ; ptr
    lbu r0, 0(r0)       ; first char
    lcu r2, 45           ; '-'
    ceq r0, r2
    brf num_no_neg
    ; Negative sign
    lc r0, -1
    add r1, -3
    sw r0, 0(r1)        ; sign=-1. RS: [sign, acc, ptr, rem, IP]
    lw r0, 6(r1)        ; ptr
    add r0, 1
    sw r0, 6(r1)        ; ptr++
    lw r0, 9(r1)        ; rem
    add r0, -1
    sw r0, 9(r1)        ; rem--
    ceq r0, z
    brf num_digit_loop
    ; Bare '-' = fail
    la r0, num_fail
    jmp (r0)

num_no_neg:
    lc r0, 1
    add r1, -3
    sw r0, 0(r1)        ; sign=1. RS: [sign, acc, ptr, rem, IP]

num_digit_loop:
    ; RS: [sign(0), acc(3), ptr(6), rem(9), IP(12)]
    lw r0, 9(r1)        ; rem
    ceq r0, z
    brf num_not_done
    la r0, num_done
    jmp (r0)
num_not_done:
    lw r0, 6(r1)        ; ptr
    lbu r0, 0(r0)       ; char

    ; Convert ASCII to digit: '0'-'9' → 0-9
    lcu r2, 48           ; '0'
    clu r0, r2           ; C = (char < '0')
    brf num_ge_0
    la r0, num_fail
    jmp (r0)
num_ge_0:
    lcu r2, 58           ; '9'+1
    clu r0, r2           ; C = (char <= '9')
    brt num_is_digit
    ; Not 0-9, fail for now (no hex support)
    la r0, num_fail
    jmp (r0)

num_is_digit:
    lcu r2, 48
    sub r0, r2           ; r0 = digit value (0-9)

    ; acc = acc * BASE + digit
    ; Multiply acc by BASE using repeated addition
    ; Save digit
    add r1, -3
    sw r0, 0(r1)        ; RS: [digit, sign, acc, ptr, rem, IP]
    ; acc is at offset 6, BASE from var
    la r0, var_base_val
    lw r0, 0(r0)        ; r0 = BASE
    lw r2, 6(r1)        ; r2 = acc
    ; result = 0, add acc BASE times
    add r1, -3
    sw r0, 0(r1)        ; save BASE counter. RS: [basectr, digit, sign, acc, ...]
    lc r0, 0
    add r1, -3
    sw r0, 0(r1)        ; result=0. RS: [result, basectr, digit, sign, acc, ...]

num_mul_loop:
    lw r0, 3(r1)        ; basectr
    ceq r0, z
    brt num_mul_done
    lw r0, 0(r1)        ; result
    add r0, r2           ; result += acc
    sw r0, 0(r1)
    lw r0, 3(r1)        ; basectr
    add r0, -1
    sw r0, 3(r1)
    bra num_mul_loop

num_mul_done:
    lw r0, 0(r1)        ; result = acc * BASE
    lw r2, 6(r1)        ; digit
    add r0, r2           ; new_acc = result + digit
    add r1, 9           ; pop result, basectr, digit
    ; RS: [sign, acc, ptr, rem, IP]
    sw r0, 3(r1)        ; acc = new_acc

    ; Advance ptr, decrement rem
    lw r0, 6(r1)
    add r0, 1
    sw r0, 6(r1)
    lw r0, 9(r1)
    add r0, -1
    sw r0, 9(r1)
    la r0, num_digit_loop
    jmp (r0)

num_fail:
    ; RS: [sign, acc, ptr, rem, IP]
    lw r2, 12(r1)       ; IP
    add r1, 15
    lc r0, 0
    push r0              ; n=0
    lc r0, -1
    push r0              ; flag=-1 (fail)
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

num_done:
    ; RS: [sign(0), acc(3), ptr(6), rem(9), IP(12)]
    lw r0, 3(r1)        ; acc
    lw r2, 0(r1)        ; sign
    cls r2, z            ; C = (sign < 0)
    brf num_pos
    ; Negate
    push r0
    lc r0, 0
    pop r2
    sub r0, r2           ; r0 = -acc
num_pos:
    lw r2, 12(r1)       ; IP
    add r1, 15
    push r0              ; n
    lc r0, 0
    push r0              ; flag=0 (success)
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

; ------------------------------------------------------------
; INTERPRET ( -- ) : Interpret-only text interpreter
; Monolithic primitive. No compile mode.
; Reads tokens with WORD, tries FIND then NUMBER.
; Found → EXECUTE. Number → leave on stack. Else → print "? "
;
; Architecture: INTERPRET is a primitive that internally calls
; WORD, FIND, NUMBER by directly jumping to their code entries.
; Each sub-primitive returns via NEXT which follows IP.
; INTERPRET chains them using small thread fragments.
;
; The key rule: at each "continuation point" (the handler primitive
; that runs after WORD/FIND/NUMBER), RS contains exactly [caller_IP].
; No nested continuations.
; ------------------------------------------------------------
entry_interpret:
    .word entry_number
    .byte 9
    .byte 73, 78, 84, 69, 82, 80, 82, 69, 84

do_interpret:
    add r1, -3
    sw r2, 0(r1)        ; RS: [caller_IP]
    ; Start: call WORD
    la r2, i_word_thread
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

i_word_thread:
    .word do_word
    .word do_i_after_word

; After WORD: DS has [c-addr]. Check if empty.
do_i_after_word:
    ; IP points past this in i_word_thread — we ignore it.
    ; RS: [caller_IP] (WORD saved/restored its own IP on RS)
    pop r0               ; c-addr
    lbu r2, 0(r0)       ; length
    ceq r2, z
    brf i_have_token
    ; Empty token → end of input, return to caller
    lw r2, 0(r1)
    add r1, 3
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

i_have_token:
    ; r0 = c-addr, push for FIND
    push r0
    la r2, i_find_thread
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

i_find_thread:
    .word do_find
    .word do_i_after_find

; After FIND: DS has (c-addr 0) or (cfa flag)
do_i_after_find:
    ; RS: [caller_IP]
    pop r0               ; flag (0=not found)
    ceq r0, z
    brt i_not_found
    ; Found: DS has [cfa]. Execute it.
    ; Set IP to continuation thread so after EXECUTE, we loop
    la r2, i_continue
    pop r0               ; cfa
    jmp (r0)             ; execute it — NEXT will use IP=i_continue

i_continue:
    .word do_word
    .word do_i_after_word

i_not_found:
    ; DS: [c-addr] (FIND returned it unchanged)
    ; Try NUMBER. Dup for error reporting.
    pop r0
    push r0
    push r0              ; DS: [c-addr, c-addr]
    la r2, i_num_thread
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

i_num_thread:
    .word do_number
    .word do_i_after_number

; After NUMBER: DS has [flag, n, c-addr]
do_i_after_number:
    ; RS: [caller_IP]
    pop r0               ; flag (0=ok)
    ceq r0, z
    brt i_num_ok
    ; Failed: print "? ", discard n and c-addr
    pop r0               ; discard n
    pop r0               ; discard c-addr
    ; Print "? "
    lc r0, 63
    push r0
    la r2, -65280
i_err1:
    lb r0, 1(r2)
    cls r0, z
    brt i_err1
    pop r0
    sb r0, 0(r2)
    lc r0, 32
    push r0
i_err2:
    lb r0, 1(r2)
    cls r0, z
    brt i_err2
    pop r0
    sb r0, 0(r2)
    ; Continue loop
    la r2, i_continue
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

i_num_ok:
    ; DS: [n, c-addr]. Keep n, discard c-addr
    pop r2               ; n
    pop r0               ; discard c-addr
    push r2              ; DS: [n]
    ; Continue loop
    la r2, i_continue
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

; ------------------------------------------------------------
; QUIT ( -- ) : Outer interpreter loop
; Resets RS, calls INTERPRET, prints " ok\n", loops.
; ------------------------------------------------------------
entry_quit:
    .word entry_interpret
    .byte 4
    .byte 81, 85, 73, 84

do_quit:
    la r1, 983040       ; reset RSP
    la r2, quit_thread
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

quit_thread:
    .word do_interpret
    .word do_quit_ok
    .word do_quit_restart

do_quit_ok:
    add r1, -3
    sw r2, 0(r1)
    la r0, var_state_val
    lw r0, 0(r0)
    ceq r0, z
    brf quit_no_ok
    ; Print " ok\n"
    la r2, -65280
    lc r0, 32
    push r0
quit_ok1:
    lb r0, 1(r2)
    cls r0, z
    brt quit_ok1
    pop r0
    sb r0, 0(r2)
    lc r0, 111
    push r0
quit_ok2:
    lb r0, 1(r2)
    cls r0, z
    brt quit_ok2
    pop r0
    sb r0, 0(r2)
    lc r0, 107
    push r0
quit_ok3:
    lb r0, 1(r2)
    cls r0, z
    brt quit_ok3
    pop r0
    sb r0, 0(r2)
    lc r0, 10
    push r0
quit_ok4:
    lb r0, 1(r2)
    cls r0, z
    brt quit_ok4
    pop r0
    sb r0, 0(r2)
quit_no_ok:
    lw r2, 0(r1)
    add r1, 3
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

do_quit_restart:
    la r1, 983040
    la r2, quit_thread
    lw r0, 0(r2)
    add r2, 3
    jmp (r0)

; ============================================================
; System Variable Storage
; ============================================================
var_here_val:
    .word 0
var_latest_val:
    .word 0
var_state_val:
    .word 0
var_base_val:
    .word 10

; ============================================================
; Phase 2 Test Colon Definitions (using far CFA format)
; ============================================================

; : TEST  42 EMIT 10 EMIT ;   — prints "*\n"
test_word_cfa:
    push r0
    la r0, do_docol_far
    jmp (r0)
    .word do_lit
    .word 42
    .word do_emit
    .word do_lit
    .word 10
    .word do_emit
    .word do_exit

; : DOUBLE  DUP + ;
double_word:
    push r0
    la r0, do_docol_far
    jmp (r0)
    .word do_dup
    .word do_plus
    .word do_exit

; : MAIN  3 DOUBLE 48 + EMIT 10 EMIT ;   — prints "6\n"
main_word:
    push r0
    la r0, do_docol_far
    jmp (r0)
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
; Test Data
; ============================================================

; Counted strings for FIND tests
cs_emit:
    .byte 4, 69, 77, 73, 84       ; "EMIT"
cs_plus:
    .byte 1, 43                     ; "+"

; EOL flag for WORD (1 byte)
word_eol_flag:
    .byte 0

; Word input buffer (32 bytes)
word_buffer:
    .byte 0, 0, 0, 0, 0, 0, 0, 0
    .byte 0, 0, 0, 0, 0, 0, 0, 0
    .byte 0, 0, 0, 0, 0, 0, 0, 0
    .byte 0, 0, 0, 0, 0, 0, 0, 0

; ============================================================
; Test Thread
; ============================================================
test_thread:
    ; --- Phase 2 regression: prints "6\n*\n" ---
    .word main_word
    .word test_word_cfa

    ; --- Phase 3 Test A: FIND "EMIT" + EXECUTE → prints 'H' ---
    .word do_lit
    .word 72             ; 'H'
    .word do_lit
    .word cs_emit        ; address of counted string "EMIT"
    .word do_find
    .word do_drop        ; drop flag (-1)
    .word do_execute     ; execute EMIT → prints 'H'

    ; --- Phase 3 Test B: FIND "+" + EXECUTE → prints 'A' ---
    .word do_lit
    .word 40
    .word do_lit
    .word 25
    .word do_lit
    .word cs_plus        ; address of counted string "+"
    .word do_find
    .word do_drop        ; drop flag
    .word do_execute     ; execute + → 40+25=65
    .word do_emit        ; emit 65 = 'A'

    ; --- Phase 3 Test C: COMMA → prints '\n' ---
    .word do_here        ; push &var_here_val
    .word do_fetch       ; get HERE value
    .word do_dup         ; save a copy
    .word do_lit
    .word 10             ; newline
    .word do_comma       ; store 10 at HERE, HERE += 3
    .word do_fetch       ; read back from saved address → 10
    .word do_emit        ; emit 10 = '\n'

    ; --- Phase 4 Test A: DOT → prints "42 " ---
    .word do_lit
    .word 42
    .word do_dot

    ; --- Phase 4 Test B: LED! → turn on LED D2 ---
    .word do_lit
    .word 1
    .word do_led_store

    ; --- Enter interactive interpreter ---
    .word do_quit

; ============================================================
; End of dictionary — HERE initialized to this address
; ============================================================
dict_end:
