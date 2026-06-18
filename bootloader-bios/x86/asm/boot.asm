; HTMOS-specific mbr
; 
; Given on how difficult it was to manage the mbr in rust,
; I've decided to create the entire mbr in nasm.
; Ranges as follows (from 0x000 to 0x1FF):
; 
; 0x000 - 0x07F : code to run on boot, later used to save boot info for the kernel
; 0x080 - 0x0D7 : GDT and IDT info
; 0x0D8 - 0x0FF : misc 16-bit functions and mutable DAP info for bios disk loading
; 0x100 - 0x17F : code to switch from real mode to protected mode
; 0x180 - 0x1BD : code to switch from protected mode to long mode (should not be used for 32-bit cpus)
; 0x1BE - 0x1FD : protected area for valid EFI file systems
; 0x1FE - 0x1FF : mbr magic

[BITS 16]
[ORG 0x7C00]

start:
    xor ax, ax
    mov ds, ax
    mov es, ax
    mov ss, ax
    mov sp, 0x7c00

    mov ah, 0x41
    mov bx, 0x55AA
    int 0x13
    jc disk_error
    cmp bx, 0xAA55
    jnz disk_error

    mov ah, 0x42
    mov si, dap
    int 0x13
    jc disk_error

    mov ah, 0x0e
    mov al, 'G'
    int 0x10

    jmp 0x0000:0x7e00

times 128-($-$$) db 0x90

align 4                         ; Align to 4 bytes for performance

gdt_start:
    ; 1. Null Descriptor (Required by the CPU, must be 8 bytes of zeros)
    dd 0x0
    dd 0x0

    ; 2. Code Segment Descriptor (Offset 0x08)
    ; Base = 0x00000000, Limit = 0xFFFFF
    ; Access byte = 0x9A (Present, Ring 0, Code, Executable, Readable)
    ; Flags = 0xCF (Granularity=4KB, 32-bit Protected Mode)
    dw 0xFFFF                   ; Limit (bits 0-15)
    dw 0x0000                   ; Base (bits 0-15)
    db 0x00                     ; Base (bits 16-23)
    db 0x9A                ; Access byte (0x9A)
    db 11001111b                ; Flags (0xC) + Limit (bits 16-19) (0xF)
    db 0x00                     ; Base (bits 24-31)

    ; 3. Data Segment Descriptor (Offset 0x10)
    ; Base = 0x00000000, Limit = 0xFFFFF
    ; Access byte = 0x92 (Present, Ring 0, Data, Writable)
    ; Flags = 0xCF (Granularity=4KB, 32-bit Protected Mode)
    dw 0xFFFF                   ; Limit (bits 0-15)
    dw 0x0000                   ; Base (bits 0-15)
    db 0x00                     ; Base (bits 16-23)
    db 0x92                ; Access byte (0x92)
    db 11001111b                ; Flags (0xC) + Limit (bits 16-19) (0xF)
    db 0x00                     ; Base (bits 24-31)

gdt_end:

; The actual descriptor pointer you pass to the 'lgdt' instruction
gdt_descriptor:
    dw gdt_end - gdt_start - 1   ; GDT size - 1
    dd gdt_start                 ; GDT linear start address

times 216-($-$$) db 0x90

disk_error:
    mov ah, 0x0e
    mov al, 'E'
    int 0x10
    hlt

loop16:
    jmp loop16

tplf16:
    lidt [zero_idtr]
    int3
align 4
zero_idtr:
    dw 0x0000
    dd 0x00000000

align 4
dap:
    db 0x10
    db 0
    dw 30 ; count
    dw 0x7E00
    dw 0x0000
    dq 34 ; start

times 256-($-$$) db 0x90
; 0x7d00

mov dl, [0x7c00 + 81]
mov ah, 0x42
mov dword [dap + 0x8], 34 + 9 ; UNSTABLE!!! '9' must be watched for and changed when the real mode rust code changes at any time.
mov si, dap
int 0x13
jc loop16
cli
xor ax, ax
mov ds, ax
lgdt [gdt_descriptor]
mov eax, cr0
or al, 1
mov cr0, eax
jmp 08h:prot
prot:
[BITS 32]
mov eax, 0x10
mov ds, ax
mov es, ax
mov fs, ax
mov gs, ax
mov ss, ax
mov esp, 0x00007C00
jmp 0x7E00

loop32:
    jmp loop32

tplf32:
    lidt [zero_idtr]
    int3

; 0x7d80
times 360-($-$$) db 0x90

and esp, 0xffffff00
jmp 08h:lng
lng:
[BITS 64]
mov rax, 0x10
mov ds, ax
mov es, ax
mov ss, ax
mov rsp, 0x00007C00
jmp 0x8400

times 446-($-$$) db 0x90 ; room for fs

times 510-($-$$) db 0
dw 0xAA55