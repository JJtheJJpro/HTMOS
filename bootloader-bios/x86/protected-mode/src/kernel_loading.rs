#[repr(C)]
struct Elf32Header {
    e_ident: [u8; 16],
    e_type: u16,
    e_machine: u16,
    e_version: u32,
    e_entry: u32,
    e_phoff: u32, // offset to program header table
    e_shoff: u32,
    e_flags: u32,
    e_ehsize: u16,
    e_phentsize: u16, // size of one program header entry
    e_phnum: u16,     // number of program header entries
    e_shentsize: u16,
    e_shnum: u16,
    e_shstrndx: u16,
}

#[repr(C)]
struct Elf32ProgramHeader {
    p_type: u32,
    p_offset: u32, // offset in file
    p_vaddr: u32,  // virtual address to load to
    p_paddr: u32,  // physical address (usually same as vaddr)
    p_filesz: u32, // size in file
    p_memsz: u32,  // size in memory (>= filesz, difference is BSS)
    p_flags: u32,
    p_align: u32,
}

const PT_LOAD: u32 = 1;

/// Manually parse and load an ELF32 binary from kbuf into memory.
/// Returns the entry point virtual address, or None if the ELF is invalid.
pub unsafe fn load_elf32(kbuf: &[u8]) -> Option<u32> {
    // Need at least enough bytes for the ELF header
    if kbuf.len() < core::mem::size_of::<Elf32Header>() {
        return None;
    }

    let base = kbuf.as_ptr();

    // Read ELF header
    let ehdr = &*(base as *const Elf32Header);

    // Validate magic: 0x7F 'E' 'L' 'F'
    if ehdr.e_ident[0] != 0x7F
        || ehdr.e_ident[1] != b'E'
        || ehdr.e_ident[2] != b'L'
        || ehdr.e_ident[3] != b'F'
    {
        return None;
    }

    // Must be 32-bit (class = 1)
    if ehdr.e_ident[4] != 1 {
        return None;
    }

    // Must be little-endian (data = 1)
    if ehdr.e_ident[5] != 1 {
        return None;
    }

    let phoff = ehdr.e_phoff as usize;
    let phentsize = ehdr.e_phentsize as usize;
    let phnum = ehdr.e_phnum as usize;
    let entry = ehdr.e_entry;

    // Sanity check program header table is within kbuf
    let ph_table_end = phoff + phentsize * phnum;
    if ph_table_end > kbuf.len() {
        return None;
    }

    // Walk program headers
    for i in 0..phnum {
        let ph_offset = phoff + i * phentsize;
        let ph = &*(base.add(ph_offset) as *const Elf32ProgramHeader);

        if ph.p_type != PT_LOAD {
            continue;
        }

        let filesz = ph.p_filesz as usize;
        let memsz = ph.p_memsz as usize;
        let vaddr = ph.p_vaddr as usize;
        let offset = ph.p_offset as usize;

        // Sanity check: segment data must be within kbuf
        if offset + filesz > kbuf.len() {
            return None;
        }

        let src = base.add(offset);
        let dest = vaddr as *mut u8;

        // Copy initialized data from file
        if filesz > 0 {
            core::ptr::copy_nonoverlapping(src, dest, filesz);
        }

        // Zero BSS (memsz > filesz means there's a BSS section)
        if memsz > filesz {
            core::ptr::write_bytes(dest.add(filesz), 0, memsz - filesz);
        }
    }

    Some(entry)
}
