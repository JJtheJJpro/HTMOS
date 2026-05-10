.global _start
.extern htmkrnl
.extern __stack_end

.section .text._start, "ax"

_start:
    pop eax
    pop eax
    lea esp, [__stack_end]
    xor ebp, ebp
    push eax
    push 0x00000000
    jmp htmkrnl