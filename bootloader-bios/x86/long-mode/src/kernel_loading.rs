#[repr(C)]
struct Elf64Header {
    e_ident:     [u8; 16],
    e_type:      u16,
    e_machine:   u16,
    e_version:   u32,
    e_entry:     u64,
    e_phoff:     u64,
    e_shoff:     u64,
    e_flags:     u32,
    e_ehsize:    u16,
    e_phentsize: u16,
    e_phnum:     u16,
    e_shentsize: u16,
    e_shnum:     u16,
    e_shstrndx:  u16,
}

#[repr(C)]
struct Elf64ProgramHeader {
    p_type:   u32,
    p_flags:  u32,   // ← moved up vs ELF32 (before offsets, not after)
    p_offset: u64,
    p_vaddr:  u64,
    p_paddr:  u64,
    p_filesz: u64,
    p_memsz:  u64,
    p_align:  u64,
}

const PT_LOAD: u32 = 1;

/// Manually parse and load an ELF64 binary from kbuf into memory.
/// Returns the entry point virtual address, or None if the ELF is invalid.
pub unsafe fn load_elf64(kbuf: &[u8]) -> Option<u64> {
    if kbuf.len() < core::mem::size_of::<Elf64Header>() {
        return None;
    }

    let base = kbuf.as_ptr();
    let ehdr = &*(base as *const Elf64Header);

    // Validate magic: 0x7F 'E' 'L' 'F'
    if ehdr.e_ident[0] != 0x7F
        || ehdr.e_ident[1] != b'E'
        || ehdr.e_ident[2] != b'L'
        || ehdr.e_ident[3] != b'F'
    {
        return None;
    }

    // Must be 64-bit (class = 2)
    if ehdr.e_ident[4] != 2 {
        return None;
    }

    // Must be little-endian (data = 1)
    if ehdr.e_ident[5] != 1 {
        return None;
    }

    let phoff    = ehdr.e_phoff    as usize;
    let phentsize = ehdr.e_phentsize as usize;
    let phnum    = ehdr.e_phnum    as usize;
    let entry    = ehdr.e_entry;

    // Sanity check program header table is within kbuf
    let ph_table_end = phoff + phentsize * phnum;
    if ph_table_end > kbuf.len() {
        return None;
    }

    for i in 0..phnum {
        let ph_offset = phoff + i * phentsize;
        let ph = &*(base.add(ph_offset) as *const Elf64ProgramHeader);

        if ph.p_type != PT_LOAD {
            continue;
        }

        let filesz = ph.p_filesz as usize;
        let memsz  = ph.p_memsz  as usize;
        let vaddr  = ph.p_vaddr  as usize;
        let offset = ph.p_offset as usize;

        if offset + filesz > kbuf.len() {
            return None;
        }

        let src  = base.add(offset);
        let dest = vaddr as *mut u8;

        if filesz > 0 {
            core::ptr::copy_nonoverlapping(src, dest, filesz);
        }

        // Zero BSS
        if memsz > filesz {
            core::ptr::write_bytes(dest.add(filesz), 0, memsz - filesz);
        }
    }

    Some(entry)
}