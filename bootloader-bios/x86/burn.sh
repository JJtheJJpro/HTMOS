if [ -z "$1" ]; then
    echo "err: device not specified"
else
    growisofs -dvd-compat -dvd-compat -Z /dev/$1=./build/disk.img
    sudo umount "/dev/$1"
fi