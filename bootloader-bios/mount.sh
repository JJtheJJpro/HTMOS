if [ -z "$1" ]; then
    echo "err: device not specified"
else
    sudo dd if=build/boot_disk.img of="/dev/$1" bs=512 conv=notrunc status=progress
    sudo umount "/dev/$1"
fi