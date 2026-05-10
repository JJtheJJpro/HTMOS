mkdir build

nasm -f bin ./asm/boot.asm -o ./build/boot.bin

cd real-mode
cargo build --release -Zjson-target-spec
objcopy -I elf32-i386 -O binary ./target/i386-unknown-none-code16/release/bootloader-bios ../build/real-mode.bin
cd ..

cd protected-mode
cargo build --release -Zjson-target-spec
objcopy -I elf32-i386 -O binary ./target/i386-unknown-none/release/bootloader-bios ../build/protected-mode.bin
cd ..

cd ../../kernel
./build-scripts/x86.sh
./build-scripts/x86_64.sh
cd ../bootloader-bios/x86

truncate -s %512 ./build/real-mode.bin
truncate -s %512 ./build/protected-mode.bin

cat ./build/real-mode.bin ./build/protected-mode.bin > ./build/stage2.bin

dd if=/dev/zero of=./build/disk.img bs=1M count=64 status=none
dd if=./build/stage2.bin of=./build/disk.img bs=512 seek=34 conv=notrunc status=none
sgdisk --clear --new=1:2048:0 --typecode=1:ef00 --change-name=1:"EFI System Partition" ./build/disk.img
dd if=./build/boot.bin of=./build/disk.img bs=446 count=1 conv=notrunc status=none

LOOP=$(sudo losetup --find --partscan --show ./build/disk.img)
sudo mkfs.fat -F32 -n "EFI" "${LOOP}p1"

MNT=$(mktemp -d)
sudo mount "${LOOP}p1" "$MNT"

sudo mkdir -p "$MNT/EFI/BOOT"
sudo cp ../../kernel/target/i386-unknown-none/release/htmkrnl "$MNT/HTMKRNL.X86"
sudo cp ../../kernel/target/x86_64-unknown-none/release/htmkrnl "$MNT/HTMKRNL.X64"
sudo cp ../../bootloader-uefi/target/xi686-unknown-uefi/release/bootloader-uefi.efi "$MNT/EFI/BOOT/BOOTA32.EFI"
sudo cp ../../bootloader-uefi/target/x86_64-unknown-uefi/release/bootloader-uefi.efi "$MNT/EFI/BOOT/BOOTX64.EFI"

sudo umount "$MNT"
rmdir "$MNT"
sudo losetup -d "$LOOP"

# RUN 32-BIT BIOS: qemu-system-i386 -drive format=raw,file=./build/disk.img -d cpu_reset -no-reboot -no-shutdown -monitor stdio
# RUN 64-BIT BIOS: TBD
# RUN 32-BIT UEFI: TBD
# RUN 64-BIT UEFI: qemu-system-x86_64 -bios /usr/share/edk2/ovmf/OVMF_CODE.fd -drive format=raw,file=./build/disk.img -d cpu_reset -no-reboot -no-shutdown -monitor stdio
# RUN ARM32 UEFI: TBD
# RUN ARM64 UEFI: TBD
# RUN RISCV32 UEFI: TBD
# RUN RISCV64 UEFI: TBD
# RUN RISCV128 UEFI: TBD
# RUN LOONGARCH32 UEFI: TBD
# RUN LOONGARCH64 UEFI: TBD
# RUN ITANIUM UEFI: TBD