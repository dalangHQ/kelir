/* see: cans/11-protocol.md#Trap-fast-path
 * Oracle TU2: emits the auxv vector keys AT_NULL/AT_EXECFN as Rust consts,
 * sourced from glibc <elf.h> (the kernel UAPI tree carries no auxv.h on this
 * platform; the AT_* keys are fixed System V ABI values). Deterministic.
 */
#include <elf.h>
#include <stdio.h>

static void u64c(const char *name, unsigned long value) {
    printf("pub const %s: u64 = %lu;\n", name, value);
}

int main(void) {
    u64c("AT_NULL", (unsigned long)AT_NULL);
    u64c("AT_EXECFN", (unsigned long)AT_EXECFN);
    return 0;
}
