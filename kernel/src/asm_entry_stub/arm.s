.global _start
.extern htmkrnl
.extern __stack_end

.section .text._start, "ax"

_start:
    # Load the address of the stack end into sp
    ldr sp, =__stack_end
    
    # Clear the frame pointer (r11) and link register (lr)
    mov fp, #0
    mov lr, #0
    
    # Branch to the Rust entry point
    b htmkrnl