#[repr(C)]
pub struct BootInfo {
    /// Physical memory map (array of entries)
    pub memory_map_addr: u64,
    pub memory_map_len: u64,
    pub memory_desc_size: u64,

    /// Framebuffer base address
    pub framebuffer_addr: u64,
    /// Width, height, pitch (stride in bytes), and pixel format
    pub framebuffer_width: u32,
    pub framebuffer_height: u32,
    pub framebuffer_pitch: u32,
    pub framebuffer_bpp: u32, // bits per pixel

    /// Boot mode indicator (0 = BIOS, 1 = UEFI)
    pub boot_mode: u32,

    /// Reserved for future extensions
    pub reserved: u32,
}