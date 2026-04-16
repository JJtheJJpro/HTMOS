cargo build --release -Zbuild-std=core -Zbuild-std-features=compiler-builtins-mem -Zjson-target-spec
nasm -f bin asm/boot.asm -o build/boot.bin