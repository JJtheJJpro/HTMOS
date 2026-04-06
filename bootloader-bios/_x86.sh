#cargo build --release -Zbuild-std=core -Zbuild-std-features=compiler-builtins-mem -Zjson-target-spec
nasm -f bin asm/boot.asm -o build/boot.bin
nasm -f bin asm/stage2.asm -o build/stage2.bin
cat build/boot.bin build/stage2.bin > build/boot_disk.img

#ld -m elf_i386 -T linker.ld build/stage2.o target/i386-unknown-none/release/libbootloader_bios.a -o build/stage2.bin --oformat binary

rm build/stage2.bin
rm build/boot.bin