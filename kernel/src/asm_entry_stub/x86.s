.global _start
.extern htmkrnl
.extern __stack_end

.section .text._start, "ax"

_start:
    # Set the stack pointer
    mov esp, __stack_end
    
    # Clear the base pointer to terminate backtraces
    xor ebp, ebp
    
    # Jump to the Rust kernel entry
    jmp htmkrnl