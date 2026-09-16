/// Maximum number of terminal replies retained until the caller consumes them.
pub const MAX_PENDING_REPLIES: usize = 16;

const PRIMARY_DEVICE_ATTRIBUTES_BYTES: &[u8] = b"\x1b[?1;0c";
const SECONDARY_DEVICE_ATTRIBUTES_BYTES: &[u8] = b"\x1b[>0;0;0c";
const TERMINAL_STATUS_BYTES: &[u8] = b"\x1b[0n";
const MAX_ENCODED_REPLY_BYTES: usize = 12;

/// A bounded wire encoding of a project-owned terminal reply.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalReplyBytes {
    bytes: [u8; MAX_ENCODED_REPLY_BYTES],
    len: u8,
}

impl TerminalReplyBytes {
    /// Returns the encoded reply without any unused buffer capacity.
    pub fn as_slice(&self) -> &[u8] {
        &self.bytes[..usize::from(self.len)]
    }

    fn from_static(bytes: &[u8]) -> Self {
        let mut encoded = Self {
            bytes: [0; MAX_ENCODED_REPLY_BYTES],
            len: bytes.len() as u8,
        };
        encoded.bytes[..bytes.len()].copy_from_slice(bytes);
        encoded
    }

    fn cursor_position(row: u16, column: u16) -> Self {
        let mut encoded = Self {
            bytes: [0; MAX_ENCODED_REPLY_BYTES],
            len: 0,
        };
        encoded.push(b'\x1b');
        encoded.push(b'[');
        encoded.push_decimal(row);
        encoded.push(b';');
        encoded.push_decimal(column);
        encoded.push(b'R');
        encoded
    }

    fn push(&mut self, byte: u8) {
        let index = usize::from(self.len);
        self.bytes[index] = byte;
        self.len += 1;
    }

    fn push_decimal(&mut self, mut value: u16) {
        let mut divisor = 10_000;
        while divisor > 1 && value / divisor == 0 {
            divisor /= 10;
        }
        loop {
            self.push(b'0' + (value / divisor) as u8);
            if divisor == 1 {
                break;
            }
            divisor /= 10;
            value %= divisor * 10;
        }
    }
}

/// A terminal-generated reply intended to be written back to the host.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalReply {
    /// Primary Device Attributes: VT100-class identity with no optional capabilities.
    PrimaryDeviceAttributes,
    /// Secondary Device Attributes: stable VT100-class identity, version 0, no options.
    SecondaryDeviceAttributes,
    /// ANSI Device Status Report: terminal status is normal/OK.
    TerminalStatus,
    /// ANSI cursor-position report with captured one-based screen coordinates.
    CursorPosition { row: u16, column: u16 },
}

impl TerminalReply {
    /// Returns a complete bounded wire encoding of this reply.
    pub fn as_bytes(&self) -> TerminalReplyBytes {
        match self {
            Self::PrimaryDeviceAttributes => {
                TerminalReplyBytes::from_static(PRIMARY_DEVICE_ATTRIBUTES_BYTES)
            }
            Self::SecondaryDeviceAttributes => {
                TerminalReplyBytes::from_static(SECONDARY_DEVICE_ATTRIBUTES_BYTES)
            }
            Self::TerminalStatus => TerminalReplyBytes::from_static(TERMINAL_STATUS_BYTES),
            Self::CursorPosition { row, column } => {
                TerminalReplyBytes::cursor_position(*row, *column)
            }
        }
    }
}

/// Fixed-capacity FIFO storage for terminal replies.
#[derive(Debug)]
pub(crate) struct PendingReplies {
    entries: [Option<TerminalReply>; MAX_PENDING_REPLIES],
    head: usize,
    len: usize,
}

impl PendingReplies {
    pub(crate) const fn len(&self) -> usize {
        self.len
    }

    pub(crate) fn push(&mut self, reply: TerminalReply) -> bool {
        if self.len == MAX_PENDING_REPLIES {
            return false;
        }

        let index = (self.head + self.len) % MAX_PENDING_REPLIES;
        self.entries[index] = Some(reply);
        self.len += 1;
        true
    }

    pub(crate) fn pop(&mut self) -> Option<TerminalReply> {
        if self.len == 0 {
            return None;
        }

        let reply = self.entries[self.head].take();
        self.head = (self.head + 1) % MAX_PENDING_REPLIES;
        self.len -= 1;
        reply
    }
}

impl Default for PendingReplies {
    fn default() -> Self {
        Self {
            entries: [None; MAX_PENDING_REPLIES],
            head: 0,
            len: 0,
        }
    }
}
