/* see: cans/11-protocol.md#Trap-fast-path
 * Oracle TU1: emits ELF layout constants as Rust consts, sourced from the
 * Linux UAPI header <linux/elf.h> (struct offsets via offsetof, sizes via
 * sizeof, ABI constants verbatim). Deterministic: same header + compiler
 * => byte-identical output (no timestamps, no paths).
 */
#include <linux/elf.h>
#include <stddef.h>
#include <stdio.h>

static void usz(const char *name, size_t value) {
    printf("pub const %s: usize = %zu;\n", name, value);
}

static void u8c(const char *name, unsigned int value) {
    printf("pub const %s: u8 = %u;\n", name, value);
}

int main(void) {
    printf("// see: cans/11-protocol.md#Trap-fast-path\n");
    printf("pub const PT_INTERP: u32 = %u;\n", (unsigned int)PT_INTERP);
    usz("EHDR64_SIZE", sizeof(Elf64_Ehdr));
    usz("EHDR64_E_PHOFF", offsetof(Elf64_Ehdr, e_phoff));
    usz("EHDR64_E_PHENTSIZE", offsetof(Elf64_Ehdr, e_phentsize));
    usz("EHDR64_E_PHNUM", offsetof(Elf64_Ehdr, e_phnum));
    usz("PHDR64_SIZE", sizeof(Elf64_Phdr));
    usz("PHDR64_P_TYPE", offsetof(Elf64_Phdr, p_type));
    usz("PHDR64_P_OFFSET", offsetof(Elf64_Phdr, p_offset));
    usz("PHDR64_P_FILESZ", offsetof(Elf64_Phdr, p_filesz));
    usz("EHDR32_E_PHOFF", offsetof(Elf32_Ehdr, e_phoff));
    usz("EHDR32_E_PHENTSIZE", offsetof(Elf32_Ehdr, e_phentsize));
    usz("EHDR32_E_PHNUM", offsetof(Elf32_Ehdr, e_phnum));
    usz("PHDR32_SIZE", sizeof(Elf32_Phdr));
    usz("PHDR32_P_TYPE", offsetof(Elf32_Phdr, p_type));
    usz("PHDR32_P_OFFSET", offsetof(Elf32_Phdr, p_offset));
    usz("PHDR32_P_FILESZ", offsetof(Elf32_Phdr, p_filesz));
    usz("EI_MAG0", EI_MAG0);
    usz("SELFMAG", SELFMAG);
    printf("pub const ELFMAG: [u8; SELFMAG] = [0x%02x, 0x%02x, 0x%02x, 0x%02x];\n",
           (unsigned int)ELFMAG0, (unsigned int)ELFMAG1,
           (unsigned int)ELFMAG2, (unsigned int)ELFMAG3);
    usz("EI_CLASS", EI_CLASS);
    usz("EI_DATA", EI_DATA);
    u8c("ELFCLASS32", ELFCLASS32);
    u8c("ELFCLASS64", ELFCLASS64);
    u8c("ELFDATA2LSB", ELFDATA2LSB);
    return 0;
}
