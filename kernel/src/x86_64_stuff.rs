pub fn hlt_loop() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}

use crate::{print, println};
use lazy_static::lazy_static;
use spin::Mutex;
use x86_64::instructions::port::Port;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};

// --- CONSTANTS ---
// Verified via your MADT parsing
const IOAPIC_BASE: u64 = 0xFEC00000;
const LAPIC_BASE: u64 = 0xFEE00000;
const KEYBOARD_VECTOR: u8 = 33;
const SPURIOUS_VECTOR: u8 = 255;

// --- GDT MOD ---
mod gdt {
    use super::*;
    use x86_64::VirtAddr;
    use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector};
    use x86_64::structures::tss::TaskStateSegment;

    pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;

    lazy_static! {
        static ref TSS: TaskStateSegment = {
            let mut tss = TaskStateSegment::new();
            tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = {
                const STACK_SIZE: usize = 4096 * 5;
                // Aligned to 16 bytes for modern CPU requirements
                #[repr(align(16))]
                struct Stack([u8; STACK_SIZE]);
                static mut STACK: Stack = Stack([0; STACK_SIZE]);

                let stack_start = VirtAddr::from_ptr(unsafe { &raw const STACK.0 });
                stack_start + STACK_SIZE as u64
            };
            tss
        };
        static ref GDT: (GlobalDescriptorTable, Selectors) = {
            let mut gdt = GlobalDescriptorTable::new();
            let code_selector = gdt.append(Descriptor::kernel_code_segment());
            let tss_selector = gdt.append(Descriptor::tss_segment(&TSS));
            (gdt, Selectors { code_selector, tss_selector })
        };
    }

    struct Selectors {
        code_selector: SegmentSelector,
        tss_selector: SegmentSelector,
    }

    pub fn init() {
        use x86_64::instructions::segmentation::{CS, DS, ES, SS, Segment};
        use x86_64::instructions::tables::load_tss;

        GDT.0.load();
        unsafe {
            CS::set_reg(GDT.1.code_selector);
            load_tss(GDT.1.tss_selector);

            // CRITICAL: Clear segment registers for Long Mode stability.
            // Some UEFI environments leave garbage here that causes Double Faults on IRQs.
            SS::set_reg(SegmentSelector(0));
            DS::set_reg(SegmentSelector(0));
            ES::set_reg(SegmentSelector(0));
        }
    }
}

// --- INTERRUPTS MOD ---
mod interrupts {
    use super::*;

    lazy_static! {
        static ref IDT: InterruptDescriptorTable = {
            let mut idt = InterruptDescriptorTable::new();
            idt.breakpoint.set_handler_fn(breakpoint_handler);

            unsafe {
                // Set Double Fault to use the IST stack
                idt.double_fault.set_handler_fn(double_fault_handler)
                    .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);

                // Hardware Handlers
                idt[KEYBOARD_VECTOR].set_handler_fn(keyboard_interrupt_handler);
                idt[SPURIOUS_VECTOR].set_handler_fn(spurious_handler);

                // Diagnostic Handlers
                idt.page_fault.set_handler_fn(page_fault_handler);
            }
            idt
        };
    }

    pub fn init_idt() {
        IDT.load();
    }

    extern "x86-interrupt" fn page_fault_handler(sf: InterruptStackFrame, err: PageFaultErrorCode) {
        use x86_64::registers::control::Cr2;
        panic!("PAGE FAULT at {:?}\nErr: {:?}\n{:#?}", Cr2::read(), err, sf);
    }

    extern "x86-interrupt" fn spurious_handler(_sf: InterruptStackFrame) {
        // No EOI needed for true spurious interrupts
    }

    extern "x86-interrupt" fn breakpoint_handler(sf: InterruptStackFrame) {
        println!("EXCEPTION: BREAKPOINT\n{:#?}", sf);
    }

    extern "x86-interrupt" fn double_fault_handler(sf: InterruptStackFrame, _err: u64) -> ! {
        panic!("EXCEPTION: DOUBLE FAULT\n{:#?}", sf);
    }

    extern "x86-interrupt" fn keyboard_interrupt_handler(_sf: InterruptStackFrame) {
        use pc_keyboard::{DecodedKey, HandleControl, PS2Keyboard, ScancodeSet1, layouts};

        lazy_static! {
            static ref KEYBOARD: Mutex<PS2Keyboard<layouts::Us104Key, ScancodeSet1>> =
                Mutex::new(PS2Keyboard::new(
                    ScancodeSet1::new(),
                    layouts::Us104Key,
                    HandleControl::Ignore
                ));
        }

        let mut keyboard = KEYBOARD.lock();
        let mut port = Port::new(0x60);
        let scancode: u8 = unsafe { port.read() };

        if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
            if let Some(key) = keyboard.process_keyevent(key_event) {
                match key {
                    DecodedKey::Unicode(character) => print!("{}", character),
                    DecodedKey::RawKey(key) => print!("{:?}", key),
                }
            }
        }

        // Send EOI to Local APIC
        unsafe {
            let eoi_ptr = (LAPIC_BASE + 0xB0) as *mut u32;
            eoi_ptr.write_volatile(0);
        }
    }
}

// --- APIC HELPERS ---
unsafe fn io_apic_write(reg: u32, value: u32) {
    let ioregsel = IOAPIC_BASE as *mut u32;
    let iowin = (IOAPIC_BASE + 0x10) as *mut u32;
    unsafe {
        ioregsel.write_volatile(reg);
        iowin.write_volatile(value);
    }
}

pub fn init() {
    gdt::init();
    interrupts::init_idt();

    unsafe {
        // 1. Completely mask the legacy PICs
        Port::<u8>::new(0x21).write(0xFF);
        Port::<u8>::new(0xA1).write(0xFF);

        // 2. Enable Local APIC and set Spurious Vector
        let svr_ptr = (LAPIC_BASE + 0xF0) as *mut u32;
        svr_ptr.write_volatile(0x100 | (SPURIOUS_VECTOR as u32));

        // 3. Configure IO-APIC for Keyboard (IRQ 1)
        // Redirection Entry 1 (Registers 0x12/0x13)
        // Set bit 13 (Polarity: Active Low) because your MADT said so!
        let low_bits = (1 << 13) | (KEYBOARD_VECTOR as u32);
        let high_bits = 0 << 24; // Destination: CPU 0
        io_apic_write(0x12, low_bits);
        io_apic_write(0x13, high_bits);

        // 4. PS/2 Keyboard Command: Enable Scanning
        let mut cmd_port = Port::<u8>::new(0x64);
        let mut data_port = Port::<u8>::new(0x60);

        while (cmd_port.read() & 0x02) != 0 {}
        cmd_port.write(0xAE); // Enable Keyboard Port

        while (cmd_port.read() & 0x02) != 0 {}
        data_port.write(0xF4); // Command: Enable Scanning

        // Flush buffer to clear the ACK from 0xF4
        while (cmd_port.read() & 0x01) != 0 {
            data_port.read();
        }
    }

    x86_64::instructions::interrupts::enable();
    println!("APIC Initialized. Polarity: Active Low. Vector: 33.");
}

use raw_acpi::madt::{
    interrupt_source_override::{
        InterruptSourceOverride, InterruptSourceOverridePolarity,
        InterruptSourceOverrideTriggerMode,
    },
    ioapic::IOAPIC,
    proc_local_apic::ProcessorLocalAPIC,
};

pub fn init_madt(madt: usize) {
    let mut sz_rem = unsafe {
        let madt = &*(madt as *const raw_acpi::madt::MADT);
        let ctrl_addr = madt.local_interrupt_controller_address;
        let pcat_compat = madt.flags;
        println!("INTERRUPT CONTROLLER ADDRESS: 0x{:08X}", ctrl_addr);
        println!(
            "PC-AT-compatible dual-8259 setup: {}",
            pcat_compat.pcat_compat()
        );

        madt.header.length as usize - 44
    };

    let mut ptr = madt + 44;

    while sz_rem > 0 {
        let length = unsafe { (ptr as *const u8).offset(1).read_volatile() };
        match unsafe { (ptr as *const u8).read_volatile() } {
            0x00 => {
                assert!(length == 8);
                let info = unsafe { *(ptr as *const ProcessorLocalAPIC) };
                let acpi_processor_uid = info.acpi_processor_uid;
                let apic_id = info.acpi_id;
                let flags = info.flags;

                print!(
                    "Processor Local APIC Info: APIC ID 0x{apic_id:02X} --- {}",
                    if flags.enabled() {
                        "ENABLED"
                    } else {
                        "DISABLED"
                    }
                );
                //println!("  ACPI Processor UID: 0x{acpi_processor_uid:02X}");
                //println!("  APIC ID: 0x{apic_id:02X}");
                //print!("  Enabled: {}", flags.enabled());
                if !flags.enabled() {
                    print!(" (Online Capable: {})", flags.online_capable());
                }
                println!();
            }
            0x01 => {
                assert!(length == 12);
                let info = unsafe { *(ptr as *const IOAPIC) };
                let io_apic_id = info.io_apic_id;
                let io_apic_addr = info.io_apic_address;
                let gsib = info.global_system_interrupt_base;

                println!(
                    "I/O APIC Info: ID 0x{io_apic_id:02X} AT ADDR 0x{io_apic_addr:08X} --- GLOBAL SYSTEM INTERRUPT BASE: 0x{gsib:08X}"
                );
            }
            0x02 => {
                assert!(length == 10);
                let info = unsafe { *(ptr as *const InterruptSourceOverride) };
                let bus = info.bus;
                let source = info.source;
                let gsi = info.global_system_interrupt;
                let flags = info.flags;

                print!(
                    "InterruptSourceOverride: Bus 0x{bus:02X}, Source 0x{source:02X} --- Global System Interrupt: 0x{gsi:08X} ("
                );
                match flags.polarity() {
                    InterruptSourceOverridePolarity::Conform => {
                        print!("Polarity: Conform, ");
                    }
                    InterruptSourceOverridePolarity::ActiveHigh => {
                        print!("Polarity: Active High, ");
                    }
                    InterruptSourceOverridePolarity::ActiveLow => {
                        print!("Polarity: Active Low, ");
                    }
                }
                match flags.trigger_mode() {
                    InterruptSourceOverrideTriggerMode::Conform => {
                        print!("Trigger Mode: Conform)");
                    }
                    InterruptSourceOverrideTriggerMode::EdgeTriggered => {
                        print!("Trigger Mode: Edge)");
                    }
                    InterruptSourceOverrideTriggerMode::LevelTriggered => {
                        print!("Trigger Mode: Level)");
                    }
                }
                println!();
            }
            _ => {}
        }

        ptr += length as usize;
        sz_rem -= length as usize;
    }
}
