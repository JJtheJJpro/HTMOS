if [ -z "$1" ]; then
    echo "err: device not specified"
else
    sudo dd if=./build/disk.img of="/dev/$1" bs=4M status=progress && sync
    sudo umount "/dev/$1"
fi