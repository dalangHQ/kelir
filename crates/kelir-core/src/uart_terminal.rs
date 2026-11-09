// see: cans/01-architecture.md#Early-boot-diagnostics
use kelir_interface::telemetry::ExitKind;

pub const UART_RING_CAPACITY: usize = 64 * 1024;

pub struct UartRingBuffer {
    buf: [u8; UART_RING_CAPACITY],
    write_cursor: usize,
    read_cursor: usize,
    len: usize,
}

impl UartRingBuffer {
    pub fn new() -> Self {
        Self {
            buf: [0; UART_RING_CAPACITY],
            write_cursor: 0,
            read_cursor: 0,
            len: 0,
        }
    }

    pub fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.buf[self.write_cursor] = *byte;
            self.write_cursor = (self.write_cursor + 1) % UART_RING_CAPACITY;
            if self.len < UART_RING_CAPACITY {
                self.len += 1;
            } else {
                self.read_cursor = (self.read_cursor + 1) % UART_RING_CAPACITY;
            }
        }
    }

    pub fn read(&mut self, out: &mut [u8]) -> usize {
        let count = out.len().min(self.len);
        for (i, slot) in out.iter_mut().enumerate().take(count) {
            *slot = self.buf[(self.read_cursor + i) % UART_RING_CAPACITY];
        }
        self.read_cursor = (self.read_cursor + count) % UART_RING_CAPACITY;
        self.len -= count;
        count
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn capacity(&self) -> usize {
        UART_RING_CAPACITY
    }

    pub fn snapshot(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.len);
        for i in 0..self.len {
            out.push(self.buf[(self.read_cursor + i) % UART_RING_CAPACITY]);
        }
        out
    }

    pub fn decode_exit_kind(&self) -> Option<ExitKind> {
        decode_early_boot_exit(&self.snapshot())
    }
}

impl Default for UartRingBuffer {
    fn default() -> Self {
        Self::new()
    }
}

pub fn decode_early_boot_exit(bytes: &[u8]) -> Option<ExitKind> {
    let text = String::from_utf8_lossy(bytes);
    // TODO (L3): further kernel panic signatures beyond the canonical "Kernel panic" line.
    text.lines()
        .find(|line| line.contains("Kernel panic"))
        .map(|line| ExitKind::KernelPanic(line.to_string()))
}
