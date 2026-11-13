#!/bin/sh
# see: cans/05-storage-plane.md#Synthetic-FUSE-inodes
# Regenerates ../../kelir-interface/src/storage/uapi_fuse.rs from <linux/fuse.h>. Deterministic.
set -eu
cd "$(dirname "$0")"
out=../../kelir-interface/src/storage/uapi_fuse.rs
tmp=$(mktemp)
trap 'rm -f "$tmp"' EXIT
cc -O0 -o "$tmp" fuse_uapi.c
"$tmp" > "$out"
