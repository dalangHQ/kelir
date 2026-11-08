#!/bin/sh
# see: cans/05-storage-plane.md#Synthetic-FUSE-inodes
# Regenerates src/planes/storage/fuse/uapi.rs from <linux/fuse.h>. Deterministic.
set -eu
cd "$(dirname "$0")"
out=../src/planes/storage/fuse/uapi.rs
tmp=$(mktemp)
trap 'rm -f "$tmp"' EXIT
cc -O0 -o "$tmp" fuse_uapi.c
"$tmp" > "$out"
