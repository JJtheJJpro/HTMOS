if [ -z "$1" ]; then
    echo "err: device not specified"
else
    rm boot_disk.img > /dev/null 2>&1
    nasm -f bin boot.asm -o boot.bin
    nasm -f bin stage2.asm -o stage2.bin
    cat boot.bin stage2.bin > boot_disk.img

    sudo dd if=boot_disk.img of="/dev/$1" bs=512 conv=notrunc status=progress
    sudo umount "/dev/$1"
fi

#dd if=/dev/zero of=final.img bs=1k count=128
#dd if=boot.bin of=final.img conv=notrunc
#dd if=stage2.bin of=final.img bs=512 seek=128 conv=notrunc
#sudo dd if=final.img of=/dev/sdb1 bs=512 conv=notrunc,fsync
#sync
