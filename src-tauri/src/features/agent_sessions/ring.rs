//! A bounded byte buffer that keeps the newest bytes: the scrollback `agent_snapshot` returns.

use std::collections::VecDeque;

pub struct Ring {
    bytes: VecDeque<u8>,
    cap: usize,
}

impl Ring {
    pub fn new(cap: usize) -> Self {
        Self {
            bytes: VecDeque::with_capacity(cap),
            cap,
        }
    }

    pub fn push(&mut self, data: &[u8]) {
        let data = &data[data.len().saturating_sub(self.cap)..];
        let overflow = (self.bytes.len() + data.len()).saturating_sub(self.cap);
        self.bytes.drain(..overflow);
        self.bytes.extend(data);
    }

    pub fn snapshot(&self) -> Vec<u8> {
        self.bytes.iter().copied().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_only_the_newest_bytes_up_to_the_cap() {
        let mut ring = Ring::new(8);
        ring.push(b"abc");
        assert_eq!(ring.snapshot(), b"abc");
        ring.push(b"defgh");
        assert_eq!(ring.snapshot(), b"abcdefgh");
        ring.push(b"ij");
        assert_eq!(ring.snapshot(), b"cdefghij");
        ring.push(b"0123456789xyz");
        assert_eq!(
            ring.snapshot(),
            b"56789xyz",
            "a push bigger than the cap keeps its tail"
        );
    }
}
