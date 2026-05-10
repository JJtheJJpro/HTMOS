![HTMOS Logo](./logo.svg "HTMOS")

# HTMOS

HTMOS stands for HyperText Markup Operating System.

HTMOS is an Operating System, with an HTML parser and JavaScript engine (both yet to be implemented), that gives a wide variety of settings for any user, new or experienced.

This is a project never to be forgotten.

# Latest Version: Pre-Alpha v0.3

- Finally got a working BIOS Bootloader to work!
    - It currently only works under 32-bit mode.  You can run it on 64-bit computers, but expect a RSOD (red screen of death).
    - Pre-Alpha v0.3.1 will have a working 64-bit BIOS bootloader alongside, don't worry.
    - Which reminds me, I may need to test the UEFI code under 32-bit.  It compiles, but idk if it runs correctly.
- The kernel now handles the E820 Memory Map given from BIOS.
    - I did a vec test with it, and it passed, so there ya go.
- You'll also notice, there is a little bit of RSDP and ACPI parsing...not much though.
- Also, this month marks a full year since I first pushed this project onto github.  Happy Anniversary!

# Previous Versions:

**Pre-Alpha v0.2.1**

- Fixed allocation limitation (hopefully).
- Introduced a little bit of framebuffer drawing (although this version is currenlty not using them at the moment).
- Created a new logo, pretty cool, right?

**Pre-Alpha v0.2**

- Got a rust allocation system working (not sure what O(x) it is, but it works and that's all I care about right now).
    - This means anything from the extern crate 'alloc' should work.
    - However, there is a known limitation of how much allocation is made.

**Pre-Alpha v0.1**

- Basically just a long but promising start.
- Got a minimal system set up.
- Kernel in ELF, easy to run from GRUB.