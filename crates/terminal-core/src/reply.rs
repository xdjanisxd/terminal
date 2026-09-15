/// Maximum number of terminal replies retained until the caller consumes them.
pub const MAX_PENDING_REPLIES: usize = 16;

const PRIMARY_DEVICE_ATTRIBUTES_BYTES: &[u8] = b"\x1b[?1;0c";

/// A terminal-generated reply intended to be written back to the host.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalReply {
    /// Primary Device Attributes: VT100-class identity with no optional capabilities.
    PrimaryDeviceAttributes,
}

impl TerminalReply {
    /// Returns the complete encoded reply bytes.
    pub const fn as_bytes(&self) -> &'static [u8] {
        match self {
            Self::PrimaryDeviceAttributes => PRIMARY_DEVICE_ATTRIBUTES_BYTES,
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
