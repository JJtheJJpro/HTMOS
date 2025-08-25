//! Provides a limited selection of bootup-use driver selection needed.

extern crate alloc;

/// Possible device types on an AHCI port
#[derive(Debug, Clone, Copy)]
pub enum PortType {
    None,
    SATA,
    SATAPI,
    SEMB,
    PMP,
}

/// AHCI HBA memory layout (only the fields we need)
#[repr(C)]
pub struct HbaMem {
    _cap: u32,
    _ghc: u32,
    _is: u32,
    /// Port Implemented bitmask: bit N set means port N exists
    pub pi: u32,
    _r: [u32; 9],
    /// Array of 32 port registers
    pub ports: [HbaPort; 32],
}

/// Only the Signature register is needed to identify the device
#[repr(C)]
pub struct HbaPort {
    _clb: u32,
    _clbu: u32,
    _fb: u32,
    _fbu: u32,
    _is: u32,
    _ie: u32,
    _cmd: u32,
    _r: u32,
    _tfd: u32,
    /// Device signature (identifies SATA/SATAPI/etc)
    pub sig: u32,
    // …other registers omitted…
}

/// Read a volatile u32 from MMIO
fn mmio_read32(addr: &u32) -> u32 {
    // SAFETY: caller must ensure `addr` is valid MMIO
    unsafe { core::ptr::read_volatile(addr) }
}

/// Scan ports and return (port_index, PortType)
pub fn scan_ports(hba: &HbaMem) -> alloc::vec::Vec<(usize, PortType)> {
    let mut found = alloc::vec::Vec::new();
    let pi = mmio_read32(&hba.pi) as u32;

    for port_idx in 0..32 {
        // skip ports not implemented
        if (pi & (1 << port_idx)) == 0 {
            continue;
        }

        let port = &hba.ports[port_idx];
        let sig = mmio_read32(&port.sig);

        let ptype = match sig {
            0x0000_0101 => PortType::SATA,
            0xEB14_0101 => PortType::SATAPI,
            0xC33C_0101 => PortType::SEMB,
            0x9669_0101 => PortType::PMP,
            _ => PortType::None,
        };

        found.push((port_idx, ptype));
    }

    found
}
