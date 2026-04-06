section .entry
[bits 16]
global entry
extern kmain

entry:
    ; 1. Enable A20 (Fast Gate)
    in al, 0x92
    or al, 2
    out 0x92, al

    ; 2. Load GDT
    cli
    lgdt [gdt_descriptor]

    ; 3. Switch to Protected Mode
    mov eax, cr0
    or eax, 1
    mov cr0, eax

    ; 4. Jump to 32-bit code
    jmp 0x08:init_pm

[bits 32]
init_pm:
    ; Setup segment registers
    mov ax, 0x10 ; Data segment offset in GDT
    mov ds, ax
    mov ss, ax
    mov es, ax
    
    ; Call Rust!
    call kmain
    jmp $

; Minimal GDT inside the entry file
gdt_start:
    dq 0x0
    dw 0xffff, 0x0000, 0x9a00, 0x00cf ; Code
    dw 0xffff, 0x0000, 0x9200, 0x00cf ; Data
gdt_end:
gdt_descriptor:
    dw gdt_end - gdt_start - 1
    dd gdt_start