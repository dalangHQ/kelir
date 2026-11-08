/* see: cans/05-storage-plane.md#Synthetic-FUSE-inodes
 * Oracle: prints the FUSE opcode table consumed by the storage plane as Rust
 * consts, sourced from the Linux UAPI header <linux/fuse.h>. Deterministic:
 * same header + compiler => byte-identical output (no timestamps).
 */
#include <linux/fuse.h>
#include <stdio.h>

static void opcode(const char *name, int value) {
    printf("pub const %s: u32 = %d;\n", name, value);
}

int main(void) {
    printf("// see: cans/05-storage-plane.md#Synthetic-FUSE-inodes\n");
    opcode("FUSE_LOOKUP", FUSE_LOOKUP);
    opcode("FUSE_READ", FUSE_READ);
    opcode("FUSE_WRITE", FUSE_WRITE);
    opcode("FUSE_SETLK", FUSE_SETLK);
    opcode("FUSE_SETLKW", FUSE_SETLKW);
    return 0;
}
