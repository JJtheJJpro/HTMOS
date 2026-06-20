use core::{arch::asm, fmt::Write};

use crate::{ADDR_E820_BASE, ADDR_E820_COUNT};

pub fn print_str(s: &str) {
    for &b in s.as_bytes() {
        if b == b'\n' {
            print_char(b'\r');
        }
        print_char(b);
    }
}
fn print_char(c: u8) {
    unsafe {
        asm!(
            "int 0x10",
            in("ah") 0x0e_u8,
            in("al") c,
            in("bh") 0_u8,
            in("bl") 0x07_u8,
        );
    }
}

core::arch::global_asm!(
    ".code16",
    "check_a20:",
    "pushf",
    "push ds",
    "push es",
    "push di",
    "push si",
    "cli",
    "xor ax, ax",
    "mov es, ax",
    "not ax",
    "mov ds, ax",
    "mov di, 0x0500",
    "mov si, 0x0510",
    "mov al, byte ptr es:[di]",
    "push ax",
    "mov al, byte ptr ds:[si]",
    "push ax",
    "mov byte ptr es:[di], 0x00",
    "mov byte ptr ds:[si], 0xFF",
    "cmp byte ptr es:[di], 0xFF",
    "pop ax",
    "mov byte ptr ds:[si], al",
    "pop ax",
    "mov byte ptr es:[di], al",
    "mov ax, 0",
    "je check_a20__exit",
    "mov ax, 1",
    "check_a20__exit:",
    "pop si",
    "pop di",
    "pop es",
    "pop ds",
    "popf",
    "ret",
);
extern "C" {
    fn check_a20() -> u16;
}

/// Wait for the keyboard controller to be ready for a command (bit 1 of port 0x64)
fn wait_io_write() {
    unsafe {
        let mut status: u8;
        loop {
            asm!("in al, 0x64", out("al") status);
            if (status & 0x02) == 0 {
                break;
            }
        }
    }
}
/// Wait for the keyboard controller to have data ready to read (bit 0 of port 0x64)
fn wait_io_read() {
    unsafe {
        let mut status: u8;
        loop {
            asm!("in al, 0x64", out("al") status);
            if (status & 0x01) != 0 {
                break;
            }
        }
    }
}
/// Method 1: The Keyboard Controller (8042 Chip)
fn enable_a20_keyboard() {
    wait_io_write();
    outb(0x64, 0xAD); // Disable keyboard port

    wait_io_write();
    outb(0x64, 0xD0); // Command: Read Output Port

    wait_io_read();
    let status = inb(0x60);

    wait_io_write();
    outb(0x64, 0xD1); // Command: Write Output Port

    wait_io_write();
    outb(0x60, status | 0x02); // Set Bit 1 (A20)

    wait_io_write();
    outb(0x64, 0xAE); // Enable keyboard port
    wait_io_write();
}
/// Method 2: Fast A20 Gate (Port 0x92)
fn enable_a20_fast() {
    let val = inb(0x92);
    if (val & 0x02) == 0 {
        outb(0x92, (val | 0x02) & 0xFE); // Set bit 1, clear bit 0 (reset bit)
    }
}
/// Method 3: BIOS Interrupt 15h
fn enable_a20_bios() -> bool {
    let mut success: u16;
    unsafe {
        asm!(
            "int 0x15",
            inout("ax") 0x2401_u16 => success,
        );
    }
    (success & 0xFF00) == 0
}
/// Returns true if A20 is enabled; otherwise, false.
pub fn enable_a20() -> bool {
    unsafe {
        // 1. Check if already enabled
        if check_a20() == 1 {
            return true;
        }
        // 2. Try BIOS
        enable_a20_bios();
        if check_a20() == 1 {
            return true;
        }
        // 3. Try Keyboard Controller
        enable_a20_keyboard();
        if check_a20() == 1 {
            return true;
        }
        // 4. Try Fast A20
        enable_a20_fast();
        if check_a20() == 1 {
            return true;
        }
    }

    false
}
// Helpers for port I/O
fn outb(port: u16, val: u8) {
    unsafe {
        asm!("out dx, al", in("dx") port, in("al") val);
    }
}
fn inb(port: u16) -> u8 {
    let res: u8;
    unsafe {
        asm!("in al, dx", out("al") res, in("dx") port);
    }
    res
}

pub static GDT: GdtProtectedMode = GdtProtectedMode::new();

#[repr(C)]
pub struct GdtProtectedMode {
    zero: u64,
    code: u64,
    data: u64,
}

impl GdtProtectedMode {
    const fn new() -> Self {
        let limit = {
            let limit_low = 0xffff;
            let limit_high = 0xf << 48;
            limit_high | limit_low
        };
        let access_common = {
            let present = 1 << 47;
            let user_segment = 1 << 44;
            let read_write = 1 << 41;
            present | user_segment | read_write
        };
        let protected_mode = 1 << 54;
        let granularity = 1 << 55;
        let base_flags = protected_mode | granularity | access_common | limit;
        let executable = 1 << 43;
        Self {
            zero: 0,
            code: base_flags | executable,
            data: base_flags,
        }
    }

    pub fn clear_interrupts_and_load(&'static self) {
        let pointer = GdtPointer {
            base: self,
            limit: (3 * size_of::<u64>() - 1) as u16,
        };

        unsafe {
            asm!("cli", "lgdt [{}]", in(reg) &pointer, options(readonly, nostack, preserves_flags));
        }
    }
}

#[repr(C, packed(2))]
pub struct GdtPointer {
    /// Size of the DT.
    pub limit: u16,
    /// Pointer to the memory region containing the DT.
    pub base: *const GdtProtectedMode,
}

unsafe impl Send for GdtPointer {}
unsafe impl Sync for GdtPointer {}

//const E820_MAGIC: u32 = 0x534D4150; // "SMAP"
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct E820Entry {
    pub base: u64,
    pub length: u64,
    pub entry_type: u32,
    pub attrs: u32,
}
#[inline(never)]
pub unsafe fn init_memory_map() -> usize {
    asm!(
        "push es",
        // ES = 0 for flat addressing
        "xor ax, ax",
        "mov es, ax",
        // EBX = continuation = 0
        "xor ebx, ebx",
        // DI = destination buffer
        "mov di, 0x0500",
        // BP = entry count = 0
        "xor bp, bp",

        "42:",                              // loop start (42 to avoid conflicts)
        "mov eax, 0xE820",
        "mov edx, 0x534D4150",
        "mov ecx, 24",
        "int 0x15",
        "jc 43f",                          // carry = done/error
        "cmp eax, 0x534D4150",
        "jne 43f",                         // bad signature = done
        "inc bp",                          // count++
        "add di, 24",                      // advance dest by sizeof(E820Entry)
        "test ebx, ebx",
        "jnz 42b",                         // continuation != 0, keep going

        "43:",                             // done
        // Store count at 0x7C52
        "mov word ptr es:[0x7C52], bp",
        "pop es",

        // no inputs or outputs — everything done in registers/memory directly
        out("eax") _,
        out("ebx") _,
        out("ecx") _,
        out("edx") _,
        out("edi") _,
        //out("ebp") _,
    );

    // Read back the count Rust-side
    unsafe { (ADDR_E820_COUNT as *const u16).read_volatile() as usize }
}
pub struct MemoryMap {
    entries: *const E820Entry,
    count: usize,
}
impl MemoryMap {
    pub unsafe fn read() -> Self {
        let count = unsafe { (ADDR_E820_COUNT as *const u16).read_volatile() as usize };
        MemoryMap {
            entries: ADDR_E820_BASE as *const E820Entry,
            count,
        }
    }

    pub fn count(&self) -> usize {
        self.count
    }

    pub fn get(&self, index: usize) -> Option<E820Entry> {
        if index >= self.count {
            return None;
        }
        Some(unsafe { self.entries.add(index).read_unaligned() })
    }

    pub fn iter(&self) -> MemoryMapIter<'_> {
        MemoryMapIter {
            map: self,
            index: 0,
        }
    }

    pub fn _total_usable(&self) -> u64 {
        self.iter()
            .filter(|e| e.entry_type == 1)
            .map(|e| e.length)
            .fold(0u64, |acc, l| acc + l)
    }

    pub fn _largest_usable(&self) -> Option<E820Entry> {
        self.iter()
            .filter(|e| e.entry_type == 1)
            .max_by_key(|e| e.length)
    }
}
pub struct MemoryMapIter<'a> {
    map: &'a MemoryMap,
    index: usize,
}
impl<'a> Iterator for MemoryMapIter<'a> {
    type Item = E820Entry;
    fn next(&mut self) -> Option<Self::Item> {
        let entry = self.map.get(self.index)?;
        self.index += 1;
        Some(entry)
    }
}

const VBE_INFO_ADDR: u16 = 0xE000; // scratch for VbeInfoBlock
const MODE_INFO_ADDR: u16 = 0xE200; // scratch for ModeInfoBlock
const FB_INFO_ADDR: *mut FramebufferInfo = 0x04D0 as *mut FramebufferInfo;

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct FramebufferInfo {
    pub addr: u64,
    pub width: u32,
    pub height: u32,
    pub pitch: u32,
    pub bpp: u8,
    // color channel layout — needed for 16bpp especially
    pub red_pos: u8,
    pub red_mask: u8,
    pub green_pos: u8,
    pub green_mask: u8,
    pub blue_pos: u8,
    pub blue_mask: u8,
}
/// VBE Controller Info Block (returned by INT 10h/AX=4F00h)
#[repr(C, packed)]
struct VbeInfoBlock {
    signature: [u8; 4], // "VESA"
    version: u16,
    oem_string_ptr: u32,
    capabilities: u32,
    mode_list_ptr: u32, // far pointer (seg:off) to mode list
    total_memory: u16,
    _reserved: [u8; 492],
}
/// VBE Mode Info Block (returned by INT 10h/AX=4F01h)
#[repr(C, packed)]
struct ModeInfoBlock {
    attributes: u16,
    window_a: u8,
    window_b: u8,
    granularity: u16,
    window_size: u16,
    segment_a: u16,
    segment_b: u16,
    win_func_ptr: u32,
    pitch: u16,
    width: u16,
    height: u16,
    w_char: u8,
    y_char: u8,
    planes: u8,
    bpp: u8,
    banks: u8,
    memory_model: u8,
    bank_size: u8,
    image_pages: u8,
    reserved0: u8,

    red_mask: u8,
    red_position: u8,
    green_mask: u8,
    green_position: u8,
    blue_mask: u8,
    blue_position: u8,
    reserved_mask: u8,
    reserved_position: u8,
    direct_color_attributes: u8,

    // VBE2+ INFO ONLY
    framebuffer: u32,
    off_screen_mem_off: u32,
    off_screen_mem_size: u16,

    // VBE3 INFO ONLY
    lin_bytes_per_scan_line: u16,
    bank_number_of_image_pages: u8,
    lin_number_of_image_pages: u8,
    linear_red_mask_size: u8,
    linear_red_field_position: u8,
    linear_green_mask_size: u8,
    linear_green_field_position: u8,
    linear_blue_mask_size: u8,
    linear_blue_field_position: u8,
    linear_reserved_mask_size: u8,
    linear_reserved_field_position: u8,
    max_pixel_clock: u32,

    reserved4: [u8; 190],
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FbError {
    VbeNotSupported, // INT 10h/4F00h failed
    BadVbeSignature, // "VESA" not found
    NoModeListPtr,   // mode list pointer is null
    NoSuitableMode,  // no 32/24/16bpp linear mode found
    NoModesAtAll,    // mode list was empty
    ModeSetFailed,   // INT 10h/4F02h failed
}
#[inline(never)]
pub unsafe fn init_framebuffer() -> Result<FramebufferInfo, FbError> {
    // Write "VBE2" to request VBE 2.0+
    let vbe_sig = VBE_INFO_ADDR as *mut u32;
    vbe_sig.write_volatile(u32::from_le_bytes(*b"VBE2"));

    let mut vbe_call_ok: u16 = 0;
    asm!(
        "push es",
        "xor ax, ax",
        "mov es, ax",
        "mov ax, 0x4F00",
        "mov di, {di:x}",
        "int 0x10",
        "pop es",
        di = in(reg) VBE_INFO_ADDR,
        inout("ax") 0x4F00u16 => vbe_call_ok,
        out("di") _,
    );

    if vbe_call_ok != 0x004F {
        return Err(FbError::VbeNotSupported);
    }

    let vbe_info = &*(VBE_INFO_ADDR as *const VbeInfoBlock);
    if &vbe_info.signature != b"VESA" {
        return Err(FbError::BadVbeSignature);
    }

    let mode_ptr_raw = { vbe_info.mode_list_ptr };
    if mode_ptr_raw == 0 {
        return Err(FbError::NoModeListPtr);
    }

    let mode_seg = (mode_ptr_raw >> 16) as u16;
    let mode_off = (mode_ptr_raw & 0xFFFF) as u16;
    let mode_list = ((mode_seg as u32) << 4) + mode_off as u32;
    let mode_list = mode_list as *const u16;

    let mut best_mode: u16 = 0xFFFF;
    let mut best_width: u32 = 0;
    let mut best_height: u32 = 0;
    let mut best_info = core::mem::zeroed::<FramebufferInfo>();
    let mut modes_seen: u16 = 0;
    let mut best_bpp: u8 = 0;

    let mut i = 0usize;
    loop {
        let mode_num = mode_list.add(i).read_volatile();
        if mode_num == 0xFFFF {
            break;
        }
        i += 1;

        let mut mode_call_ok: u16 = 0;
        asm!(
            "push es",
            "xor ax, ax",
            "mov es, ax",
            "mov ax, 0x4F01",
            "mov di, {di:x}",
            "int 0x10",
            "pop es",
            di = in(reg) MODE_INFO_ADDR,
            in("cx") mode_num,
            inout("ax") 0x4F01u16 => mode_call_ok,
            out("di") _,
        );

        if mode_call_ok != 0x004F {
            print_str("mode call not ok\n");
            continue;
        }

        let info = &*(MODE_INFO_ADDR as *const ModeInfoBlock);
        let attrs = { info.attributes };
        let width = { info.width } as u32;
        let height = { info.height } as u32;
        let bpp = { info.bpp };
        let pitch = { info.pitch } as u32;
        let fb_addr = { info.framebuffer } as u64;
        let mem_model = { info.memory_model };

        let supported = attrs & (1 << 0) != 0;
        let color = attrs & (1 << 3) != 0;
        let graphics = attrs & (1 << 4) != 0;
        let linear_fb = attrs & (1 << 7) != 0;
        let direct = mem_model == 6;

        use core::fmt::Write;

        if !supported || !color || !graphics || !linear_fb || !direct {
            continue;
        }
        if fb_addr == 0 {
            continue;
        }

        // Accept 32bpp or 24bpp — prefer 32bpp
        if bpp != 32 && bpp != 24 {
            continue;
        }

        modes_seen += 1;

        let area = width * height;
        let best_area = best_width * best_height;

        // Prefer 32bpp over 24bpp, then higher resolution
        let better_bpp = bpp > best_bpp;
        let same_bpp_bigger = bpp == best_bpp && area > best_area;

        if better_bpp || same_bpp_bigger {
            best_mode = mode_num;
            best_width = width;
            best_height = height;
            best_bpp = bpp;
            best_info = FramebufferInfo {
                addr: fb_addr,
                width,
                height,
                pitch,
                bpp,
                red_pos: { info.red_position },
                red_mask: { info.red_mask },
                green_pos: { info.green_position },
                green_mask: { info.green_mask },
                blue_pos: { info.blue_position },
                blue_mask: { info.blue_mask },
            };
        }
    }

    if modes_seen == 0 && i == 0 {
        return Err(FbError::NoModesAtAll);
    }
    if best_mode == 0xFFFF {
        return Err(FbError::NoSuitableMode);
    }

    let mut set_ok: u16 = 0;
    asm!(
        "mov ax, 0x4F02",
        "int 0x10",
        in("bx") best_mode | 0x4000u16,
        inout("ax") 0x4F02u16 => set_ok,
    );

    if set_ok != 0x004F {
        return Err(FbError::ModeSetFailed);
    }

    FB_INFO_ADDR.write_volatile(best_info);
    Ok(best_info)
}
/*
unsafe fn dump_mode_info_full(mode_num: u16) {
    // Re-query the mode info we selected
    let mut ok: u16 = 0;
    asm!(
        "push es",
        "xor ax, ax",
        "mov es, ax",
        "mov ax, 0x4F01",
        "mov di, {di:x}",
        "int 0x10",
        "pop es",
        di = in(reg) MODE_INFO_ADDR,
        in("cx") mode_num,
        inout("ax") 0x4F01u16 => ok,
        out("di") _,
    );

    if ok != 0x004F {
        writeln!(Writer, "mode info query failed").ok();
        return;
    }

    let info = &*(MODE_INFO_ADDR as *const ModeInfoBlock);

    writeln!(Writer, "mode:         {:04X}", mode_num).ok();
    writeln!(Writer, "attrs:        {:04X}", { info.attributes }).ok();
    writeln!(Writer, "win_a_seg:    {:04X}", { info.segment_a }).ok();
    writeln!(Writer, "win_b_seg:    {:04X}", { info.segment_b }).ok();
    writeln!(Writer, "granularity:  {} KB", { info.granularity }).ok();
    writeln!(Writer, "win_size:     {} KB", { info.window_size }).ok();
    writeln!(Writer, "pitch:        {}", { info.pitch }).ok();
    writeln!(Writer, "resolution:   {}x{}", { info.width }, {
        info.height
    })
    .ok();
    writeln!(Writer, "bpp:          {}", { info.bpp }).ok();
    writeln!(Writer, "mem_model:    {}", { info.memory_model }).ok();
    writeln!(Writer, "fb_addr:      {:08X}", { info.framebuffer }).ok();
    writeln!(Writer, "off_screen:   {:08X}", { info.off_screen_mem_off }).ok();
    writeln!(Writer, "off_scr_size: {} KB", { info.off_screen_mem_size }).ok();

    // VBE 3.0 extended fields (after offset 0x32)
    let ptr = MODE_INFO_ADDR as *const u8;
    let lin_pitch = u16::from_le_bytes([ptr.add(0x32).read(), ptr.add(0x33).read()]);
    let lin_red_mask = ptr.add(0x35).read();
    let lin_red_pos = ptr.add(0x36).read();
    let lin_grn_mask = ptr.add(0x37).read();
    let lin_grn_pos = ptr.add(0x38).read();
    let lin_blu_mask = ptr.add(0x39).read();
    let lin_blu_pos = ptr.add(0x3A).read();
    let max_pixel_clk = u32::from_le_bytes([
        ptr.add(0x3E).read(),
        ptr.add(0x3F).read(),
        ptr.add(0x40).read(),
        ptr.add(0x41).read(),
    ]);

    writeln!(Writer, "lin_pitch:    {}", lin_pitch).ok();
    writeln!(
        Writer,
        "lin_rgb:      R{}/{} G{}/{} B{}/{}",
        lin_red_pos, lin_red_mask, lin_grn_pos, lin_grn_mask, lin_blu_pos, lin_blu_mask,
    )
    .ok();
    writeln!(Writer, "max_pclk:     {}", max_pixel_clk).ok();
}
#[inline(never)]
pub unsafe fn dump_vbe_modes() {
    let vbe_sig = VBE_INFO_ADDR as *mut u32;
    vbe_sig.write_volatile(u32::from_le_bytes(*b"VBE2"));

    let mut vbe_call_ok: u16 = 0;
    asm!(
        "push es",
        "xor ax, ax",
        "mov es, ax",
        "mov ax, 0x4F00",
        "mov di, {di:x}",
        "int 0x10",
        "pop es",
        di = in(reg) VBE_INFO_ADDR,
        inout("ax") 0x4F00u16 => vbe_call_ok,
        out("di") _,
    );

    if vbe_call_ok != 0x004F {
        writeln!(Writer, "VBE call failed (AX={:04X})", vbe_call_ok).ok();
        return;
    }

    let vbe_info = &*(VBE_INFO_ADDR as *const VbeInfoBlock);
    if &vbe_info.signature != b"VESA" {
        writeln!(Writer, "Bad VESA signature").ok();
        return;
    }

    let version = { vbe_info.version };
    writeln!(Writer, "VBE version: {:04X}", version).ok();

    let mode_ptr_raw = { vbe_info.mode_list_ptr };
    let mode_seg = (mode_ptr_raw >> 16) as u16;
    let mode_off = (mode_ptr_raw & 0xFFFF) as u16;
    let mode_list = ((mode_seg as u32) << 4) + mode_off as u32;
    let mode_list = mode_list as *const u16;

    writeln!(Writer, "MODE  WxH          BPP  ATTRS  MDL  LFB  FB").unwrap();
    writeln!(Writer, "----  -----------  ---  -----  ---  ---  --").unwrap();

    let mut i = 0usize;
    loop {
        let mode_num = mode_list.add(i).read_volatile();
        if mode_num == 0xFFFF {
            break;
        }
        i += 1;

        let mut mode_call_ok: u16 = 0;
        asm!(
            "push es",
            "xor ax, ax",
            "mov es, ax",
            "mov ax, 0x4F01",
            "mov di, {di:x}",
            "int 0x10",
            "pop es",
            di = in(reg) MODE_INFO_ADDR,
            in("cx") mode_num,
            inout("ax") 0x4F01u16 => mode_call_ok,
            out("di") _,
        );

        if mode_call_ok != 0x004F {
            continue;
        }

        let info = &*(MODE_INFO_ADDR as *const ModeInfoBlock);
        let attrs = { info.attributes };
        let width = { info.width };
        let height = { info.height };
        let bpp = { info.bpp };
        let mem_model = { info.memory_model };
        let fb_addr = { info.framebuffer };

        let color = (attrs & (1 << 3)) != 0;
        let graphics = (attrs & (1 << 4)) != 0;
        let linear = (attrs & (1 << 7)) != 0;

        // only show color graphics modes
        if !color || !graphics {
            continue;
        }

        writeln!(
            Writer,
            "{:04X}  {:5}x{:<5}  {:3}  {:04X}   {:3}  {:3}  {:08X}",
            mode_num,
            width,
            height,
            bpp,
            attrs,
            mem_model,
            if linear { "LFB" } else { " - " },
            fb_addr,
        )
        .ok();
    }

    writeln!(Writer, "\nTotal modes checked: {}", i).ok();
}
*/

const KERNEL_LOAD_ADDR: u32 = 0x0001_0000;
const KERNEL_SIZE_ADDR: *mut u32 = 0xFFFC as *mut u32;

#[repr(C, packed)]
pub struct DiskAddressPacket {
    pub packet_size: u8,
    pub reserved: u8,
    pub sector_count: u16,
    pub dest_offset: u16,
    pub dest_segment: u16,
    pub lba: u64,
}

/// Read a single sector from disk using INT 13h extended read (AH=0x42)
/// lba: sector number
/// buf: physical address of destination buffer (must be below 1MB)
pub unsafe fn read_sector_rm(drive: u8, lba: u64, buf: u32) -> bool {
    let dap = DiskAddressPacket {
        packet_size: 0x10,
        reserved: 0,
        sector_count: 1,
        dest_offset: (buf & 0xF) as u16,
        dest_segment: (buf >> 4) as u16,
        lba,
    };

    let dap_ptr = &dap as *const DiskAddressPacket as u32;

    let mut carry: u8;
    asm!(
        "push ds",
        "push si",
        "xor ax, ax",
        "mov ds, ax",
        "mov si, {ptr:x}",
        "mov ah, 0x42",
        "int 0x13",
        "setc {carry}",
        "pop si",
        "pop ds",
        ptr   = in(reg) dap_ptr as u16,
        in("dl") drive,
        carry = out(reg_byte) carry,
        out("ax") _,
    );

    carry == 0
}

/// Read sectors from disk using INT 13h extended read (AH=0x42)
/// lba: sector number
/// buf: physical address of destination buffer (must be below 1MB)
pub unsafe fn read_sectors_rm(drive: u8, lba: u64, n_sectors: u16, buf: u32) -> bool {
    let dap = DiskAddressPacket {
        packet_size: 0x10,
        reserved: 0,
        sector_count: n_sectors,
        dest_offset: (buf & 0xF) as u16,
        dest_segment: (buf >> 4) as u16,
        lba,
    };

    let dap_ptr = &dap as *const DiskAddressPacket as u32;

    let mut carry: u8;
    asm!(
        "push ds",
        "push si",
        "xor ax, ax",
        "mov ds, ax",
        "mov si, {ptr:x}",
        "mov ah, 0x42",
        "int 0x13",
        "setc {carry}",
        "pop si",
        "pop ds",
        ptr   = in(reg) dap_ptr as u16,
        in("dl") drive,
        carry = out(reg_byte) carry,
        out("ax") _,
    );

    carry == 0
}

pub unsafe fn load_kernel_data(drive: u8, start_lba: u64, n_sectors: u32, load_addr: u32) -> bool {
    let mut dest = load_addr;
    let mut lba = start_lba;
    let mut remaining = n_sectors;

    while remaining > 0 {
        // Max sectors we can read before hitting a 64KB segment boundary
        // dest_offset = dest & 0xF (always 0 if dest is paragraph-aligned)
        // sectors until 64KB boundary = (0x10000 - (dest & 0xFFFF)) / 512
        let offset_in_seg = dest & 0xFFFF;
        let bytes_to_boundary = 0x10000u32 - offset_in_seg;
        let sectors_to_boundary = bytes_to_boundary / 512;

        let count = remaining.min(sectors_to_boundary).min(127) as u16; // BIOS limit ~127

        if !read_sectors_rm(drive, lba, count, dest) {
            return false;
        }

        dest += count as u32 * 512;
        lba += count as u64;
        remaining -= count as u32;
    }

    true
}

/// Returns true if CPUID instruction is supported.
/// Must be called from 16-bit real mode context.
pub unsafe fn cpuid_supported() -> bool {
    let supported: u32;
    core::arch::asm!(
        // Save original EFLAGS onto stack
        "pushfd",
        // Copy EFLAGS into EAX
        "pop eax",
        // Save original value
        "mov ecx, eax",
        // Toggle bit 21 (ID flag)
        "xor eax, 0x200000",
        // Push modified value back
        "push eax",
        // Load modified value into EFLAGS
        "popfd",
        // Read EFLAGS back into EAX
        "pushfd",
        "pop eax",
        // Restore original EFLAGS
        "push ecx",
        "popfd",
        // XOR to check if bit 21 actually changed
        "xor eax, ecx",
        // Isolate bit 21; nonzero = supported
        "and eax, 0x200000",
        out("eax") supported,
        out("ecx") _,
        options(nostack)   // we manually manage the stack
    );
    supported != 0
}

#[derive(Debug, PartialEq)]
pub enum CpuBitness {
    Bits64, // Long Mode supported (x86-64)
    Bits32, // No Long Mode (IA-32 only)
}

/// Queries CPUID to determine if CPU is 32-bit or 64-bit capable.
/// Only call this after confirming CPUID is supported.
pub unsafe fn cpu_bitness() -> CpuBitness {
    let cpuid = core::arch::x86::__cpuid(0x80000000);
    if cpuid.eax < 0x80000001 {
        return CpuBitness::Bits32;
    }
    let cpuid = core::arch::x86::__cpuid(0x80000001);
    if cpuid.edx & 0x20000000 != 0 {
        CpuBitness::Bits64
    } else {
        CpuBitness::Bits32
    }
    /*
    let edx: u32;

    core::arch::asm!(
        // First check max extended leaf via 0x80000000
        "mov eax, 0x80000000",
        "cpuid",
        // EAX now contains max extended function number
        // If EAX < 0x80000001, extended info not available → 32-bit only
        "cmp eax, 0x80000001",
        "jb 2f",             // jump to fallback if not supported

        // Query extended feature flags
        "mov eax, 0x80000001",
        "cpuid",
        // EDX bit 29 = LM (Long Mode / 64-bit support)
        "and edx, 0x20000000",
        "jmp 3f",

        "2:",                // fallback: extended leaf unavailable
        "xor edx, edx",     // treat as 32-bit

        "3:",
        out("eax") _,
        out("ebx") _,
        out("ecx") _,
        out("edx") edx,
        options(nostack, nomem)
    );

    if edx != 0 {
        CpuBitness::Bits64
    } else {
        CpuBitness::Bits32
    }
    */
}

/// Returns true if CPU is a 486 or later (AC flag exists).
/// On a 386, the AC bit cannot be set — it's hardwired to 0.
unsafe fn is_486_or_later() -> bool {
    let result: u32;
    core::arch::asm!(
        "pushfd",
        "pop eax",
        "mov ecx, eax",
        // Try to set bit 18 (AC flag)
        "xor eax, 0x40000",
        "push eax",
        "popfd",
        // Read back EFLAGS
        "pushfd",
        "pop eax",
        // Restore original EFLAGS
        "push ecx",
        "popfd",
        // Check if bit 18 stuck
        "xor eax, ecx",
        "and eax, 0x40000",
        out("eax") result,
        out("ecx") _,
        options(nostack)
    );
    result != 0
}

const MBR_BUF: u32 = 0xEC00;
const GPT_BUF: u32 = 0xEC00;
const ETR_BUF: u32 = 0xEE00;
const FAT_BUF: u32 = 0xEC00;
const DAT_BUF: u32 = 0xEC00;

#[repr(C)]
struct PartitionTableHeader {
    signature: [u8; 8],
    revision: u32,
    header_size: u32,
    checksum_header: u32,
    reserved: u32,
    lba_header: u64,
    lba_alt_header: u64,
    first_block: u64,
    last_block: u64,
    guid: [u8; 16],
    lba_entry: u64,
    num_entries: u32,
    size_entry: u32,
    checksum_array: u32,
}
#[repr(C)]
struct PartitionEntry {
    type_guid: [u8; 16],
    guid: [u8; 16],
    lba_start: u64,
    lba_end: u64,
    attrs: u64,
    name: [u16; 36],
}

/// Returns true if 64-bit.
pub unsafe fn load_kernel(drive: u8) -> Result<bool, &'static str> {
    // ── Step 1: Read MBR ──────────────────────────────────────────────────
    if !read_sector_rm(drive, 0, MBR_BUF) {
        return Err("MBR read failed");
    }

    let mbr = MBR_BUF as *const u8;

    if mbr.add(510).read() != 0x55 || mbr.add(511).read() != 0xAA {
        return Err("bad MBR signature");
    }

    // ── Step 2: Find FAT32/ESP partition ──────────────────────────────────
    let mut part_lba: u32 = 0;
    for i in 0..4usize {
        let entry = mbr.add(446 + i * 16);
        if entry.offset(0x4).read() != 0xEE {
            continue;
        }
        assert!(entry.offset(0x0).read() == 0x00);
        assert!(entry.offset(0x1).read() == 0x00);
        assert!(entry.offset(0x2).read() == 0x02);
        assert!(entry.offset(0x3).read() == 0x00);
        // skip chs
        part_lba = (entry.offset(0x8) as *const u32).read();
        assert!(part_lba == 0x00000001);
        break;
    }

    if part_lba == 0 {
        return Err("no FAT partition");
    }

    // ── Step 3: Read and parse BPB ────────────────────────────────────────
    if !read_sector_rm(drive, part_lba as u64, GPT_BUF) {
        return Err("GPT part 1 read failed");
    }

    let gpt_header = &*(GPT_BUF as *const PartitionTableHeader);

    const ENTRIES_PER_SECTOR: u32 = 512 / 128;
    for i in 0..gpt_header.num_entries {
        // Read new sector only when needed
        if i % ENTRIES_PER_SECTOR == 0 {
            let sector_lba = gpt_header.lba_entry + (i / ENTRIES_PER_SECTOR) as u64;
            if !read_sector_rm(drive, sector_lba, ETR_BUF) {
                return Err("GPT entry read failed");
            }
        }

        // Each entry is 128 bytes within the sector
        let entry_offset = ((i % ENTRIES_PER_SECTOR) * 128) as usize;
        let entry = &*((ETR_BUF as usize + entry_offset) as *const PartitionEntry);

        // EFI System Partition GUID (mixed-endian)
        let esp_guid: [u8; 16] = [
            0x28, 0x73, 0x2A, 0xC1, 0x1F, 0xF8, 0xD2, 0x11, 0xBA, 0x4B, 0x00, 0xA0, 0xC9, 0x3E,
            0xC9, 0x3B,
        ];

        if entry.type_guid != esp_guid {
            continue;
        }

        part_lba = entry.lba_start as u32;

        break;
    }

    if !read_sector_rm(drive, part_lba as u64, FAT_BUF) {
        return Err("FAT BPB read failed");
    }

    let bpb = FAT_BUF as *const u8;

    let bps = u16::from_le_bytes([bpb.add(11).read(), bpb.add(12).read()]) as u32;
    let spc = bpb.add(13).read() as u32;
    //let bpc = bps * spc;
    let rsvd = u16::from_le_bytes([bpb.add(14).read(), bpb.add(15).read()]) as u32;
    let nfats = bpb.add(16).read() as u32;
    let spf16 = u16::from_le_bytes([bpb.add(22).read(), bpb.add(23).read()]) as u32;
    let spf32 = u32::from_le_bytes([
        bpb.add(36).read(),
        bpb.add(37).read(),
        bpb.add(38).read(),
        bpb.add(39).read(),
    ]);
    let root_cluster = u32::from_le_bytes([
        bpb.add(44).read(),
        bpb.add(45).read(),
        bpb.add(46).read(),
        bpb.add(47).read(),
    ]);

    let spf = if spf16 != 0 { spf16 } else { spf32 };
    let fat_lba = part_lba + rsvd;
    let first_data_lba = part_lba + rsvd + nfats * spf;

    let mut krnl_lba = 0;
    let mut krnl_sz = 0;

    if !read_sector_rm(drive, first_data_lba as u64, DAT_BUF) {
        return Err("First Data LBA read failed");
    }

    let x64 = if is_486_or_later() {
        print_str("CPUID supported\n");
        if cpu_bitness() == CpuBitness::Bits64 {
            print_str("64-bit\n");
            true
        } else {
            print_str("32-bit\n");
            false
        }
    } else {
        print_str("CPUID not supported\n");
        false
    };

    let krnl_name = if x64 { b"HTMKRNL X64" } else { b"HTMKRNL X86" };

    for i in 0..16 {
        let name = core::slice::from_raw_parts((DAT_BUF + i * 0x20) as *const u8, 11);
        if name == krnl_name {
            let data = (DAT_BUF + i * 0x20) as *const u8;
            let hi = (data.offset(20) as *const u16).read();
            let lo = (data.offset(26) as *const u16).read();
            krnl_lba = ((hi as u32) << 16) | (lo as u32);
            krnl_sz = (data.offset(28) as *const u32).read();
            break;
        } else if name == &[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0] {
            break;
        }
    }

    if krnl_lba == 0 {
        return Err("Kernel not found");
    }

    krnl_lba += first_data_lba - root_cluster;
    let krnl_sz_lba = (krnl_sz + 0x1FF) / 0x200;

    if !load_kernel_data(drive, krnl_lba as u64, krnl_sz_lba, KERNEL_LOAD_ADDR) {
        return Err("Kernel read failed");
    }

    KERNEL_SIZE_ADDR.write(krnl_sz);

    Ok(x64)
}
