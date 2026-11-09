#!/bin/sh
# see: cans/11-protocol.md#Trap-fast-path
# Regenerates src/introspect_trap/uapi_trap.rs from <linux/elf.h> (TU1) + glibc <elf.h>
# (TU2). Deterministic: re-running yields byte-identical output.
set -eu
cd "$(dirname "$0")"
out=../src/introspect_trap/uapi_trap.rs
tmp1=$(mktemp)
tmp2=$(mktemp)
trap 'rm -f "$tmp1" "$tmp2"' EXIT
cc -O0 -o "$tmp1" elf_uapi_kernel.c
cc -O0 -o "$tmp2" elf_uapi_glibc.c
"$tmp1" > "$out"
"$tmp2" >> "$out"
