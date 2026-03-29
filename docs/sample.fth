\ Sample Forth program - try loading with "Load .fth" button
: square dup * ;
: cube dup dup * * ;
3 square .
4 cube .
: greet 72 emit 73 emit 10 emit ;
greet
