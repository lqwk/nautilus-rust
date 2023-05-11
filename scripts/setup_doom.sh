#!/bin/bash
while [[ $# -gt 0 ]]; do
  key="$1"

  case $key in
    --mount)
      mount="$2"
      shift # past argument
      shift # past value
      ;;
    --wad)
      wad="$2"
      shift # past argument
      shift # past value
      ;;
    *)    # unknown option
      echo "Unknown option: $key"
      exit 1
      ;;
  esac
done

if [[ -z "$mount" || -z "$wad" ]]; then
  echo "Usage: $0 --mount /path/to/mount --wad /path/to/wad"
  exit 1
fi

set -euo pipefail

dd if=/dev/zero of=disk.img bs=1024 count=65536
mkfs.fat -F 32 disk.img
sudo mount -o loop disk.img "$mount"
sudo mkdir "$mount"/home
sudo mcopy "$wad" "$mount"/home/
sudo umount "$mount"
