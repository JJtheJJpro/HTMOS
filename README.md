![HTMOS Logo](./logo.svg "HTMOS")

# HTMOS

HTMOS stands for HyperText Markup Operating System.

HTMOS is an Operating System, with an HTML parser and JavaScript engine (both yet to be implemented), that gives a wide variety of settings for any user, new or experienced.

This is a project never to be forgotten.

# Latest Version: Pre-Alpha v0.2.1

- Fixed allocation limitation (hopefully).
- Introduced a little bit of framebuffer drawing (although this version is currenlty not using them at the moment).
- Created a new logo, pretty cool, right?

# Previous Versions:

**Pre-Alpha v0.2**

- Got a rust allocation system working (not sure what O(x) it is, but it works and that's all I care about right now).
    - This means anything from the extern crate 'alloc' should work.
    - However, there is a known limitation of how much allocation is made.

**Pre-Alpha v0.1**

- Basically just a long but promising start.
- Got a minimal system set up.
- Kernel in ELF, easy to run from GRUB.