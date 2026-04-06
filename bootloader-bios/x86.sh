cargo build --release -Zbuild-std=core -Zbuild-std-features=compiler-builtins-mem -Zjson-target-spec
nasm -f bin asm/boot.asm -o build/boot.bin
nasm -f elf32 asm/stage2.asm -o build/stage2.o

ld -m elf_i386 -T linker.ld build/stage2.o target/i386-unknown-none/release/libbootloader_bios.a -o build/stage2.bin --oformat binary

cat build/boot.bin build/stage2.bin > build/boot_disk.img
rm build/stage2.o
rm build/boot.bin
rm build/stage2.bin