[bits 16]
[org 0x7c00]

start:
    jmp 0:init              ; Far jump to fix CS:IP to 0000:7C00

init:
    cli                     ; Disable interrupts
    xor ax, ax
    mov ds, ax
    mov es, ax
    mov ss, ax
    mov sp, 0x7c00          ; Stack grows down from MBR

    ; --- 1. ENABLE A20 (Fast Gate A20) ---
    in al, 0x92
    or al, 2
    out 0x92, al

    ; --- 2. LOAD MEMORY MAP (E820) ---
    ; Loading to 0x500 (safe conventional memory spot)
    mov di, 0x0504          ; Leave room for entry count at 0x500
    xor ebx, ebx
    xor bp, bp              ; Entry counter
.e820_lp:
    mov edx, 0x534D4150     ; 'SMAP'
    mov eax, 0xE820
    mov ecx, 24
    int 0x15
    jc .e820_done
    cmp eax, 0x534D4150
    jne .e820_done
    test ebx, ebx
    jz .e820_done
    add di, 24
    inc bp
    jmp .e820_lp
.e820_done:
    mov [0x0500], bp        ; Store number of entries

    ; --- 3. ENABLE FRAMEBUFFER (VBE) ---
    ; Get VBE Mode Info for 1024x768x32bit (Mode 0x4117 includes LFB bit)
    mov ax, 0x4F01
    mov cx, 0x4117          ; Mode number
    mov di, 0x7000          ; Temporary buffer for mode info
    int 0x10

    ; Set the mode
    mov ax, 0x4F02
    mov bx, 0x4117          ; BIT 14 set for Linear Framebuffer
    int 0x10

    ; --- 4. LOAD NEXT SECTORS (LBA) ---
    ; Loading to 0x7E00 (immediately after MBR)
    mov ah, 0x42            ; Extended Read
    mov dl, 0x80            ; Drive (C:)
    mov si, dap             ; Disk Address Packet
    int 0x13
    jc $                    ; Hang on error

    ; --- 5. SETUP GDT ---
    lgdt [gdt_descriptor]

    ; --- 6. ENTER PROTECTED MODE ---
    mov eax, cr0
    or eax, 1               ; Set PE bit
    mov cr0, eax

    jmp 0x08:protected_mode ; Far jump to clear pipeline and set CS

[bits 32]
protected_mode:
    mov ax, 0x10            ; Update segment registers
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    mov ss, ax

    jmp 0x7E00              ; Jump to the loaded sectors

; --- DATA STRUCTURES ---

align 4
dap:                        ; Disk Address Packet
    db 0x10                 ; Size of DAP
    db 0                    ; Unused
    dw 16                   ; Number of sectors to read
    dw 0x7E00               ; Offset
    dw 0x0000               ; Segment
    dq 1                    ; Start LBA (Sector 1)

gdt_start:
    dq 0x0                  ; Null descriptor
gdt_code:                   ; 0x08: Code segment
    dw 0xFFFF, 0x0000
    db 0x00, 10011010b, 11001111b, 0x00
gdt_data:                   ; 0x10: Data segment
    dw 0xFFFF, 0x0000
    db 0x00, 10010010b, 11001111b, 0x00
gdt_end:

gdt_descriptor:
    dw gdt_end - gdt_start - 1
    dd gdt_start

times 510-($-$$) db 0
dw 0xAA55