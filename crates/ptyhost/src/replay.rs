//! Bounded ring of sequence-numbered output chunks.

use std::collections::VecDeque;
use std::sync::Arc;

use crate::types::{OutputChunk, PtyError, ReplayFrom, Seq};

#[derive(Debug)]
pub struct ReplayRing {
    capacity_bytes: usize,
    chunks: VecDeque<OutputChunk>,
    held_bytes: usize,
    next_seq: u64,
}

impl ReplayRing {
    pub fn new(capacity_bytes: usize) -> Self {
        Self {
            capacity_bytes: capacity_bytes.max(1),
            chunks: VecDeque::new(),
            held_bytes: 0,
            next_seq: 1,
        }
    }

    /// Numbers `data` and stores it, dropping the oldest chunks that no longer fit.
    /// Data larger than the ring is split so that no single chunk exceeds the capacity.
    pub fn push(&mut self, data: &[u8]) -> Vec<OutputChunk> {
        let mut added = Vec::new();
        for piece in data.chunks(self.capacity_bytes) {
            let chunk = OutputChunk {
                seq: Seq(self.next_seq),
                bytes: Arc::from(piece),
            };
            self.next_seq += 1;
            self.held_bytes += piece.len();
            self.chunks.push_back(chunk.clone());
            while self.held_bytes > self.capacity_bytes {
                match self.chunks.pop_front() {
                    Some(old) => self.held_bytes -= old.bytes.len(),
                    None => break,
                }
            }
            added.push(chunk);
        }
        added
    }

    /// Chunks from the requested point on, in order. A point outside the ring is an error,
    /// never a silent skip.
    pub fn replay(&self, from: ReplayFrom) -> Result<Vec<OutputChunk>, PtyError> {
        let requested = match from {
            ReplayFrom::Oldest => return Ok(self.chunks.iter().cloned().collect()),
            ReplayFrom::Seq(seq) => seq,
        };
        if requested > self.next_seq() {
            return Err(PtyError::ReplayAhead {
                requested,
                next: self.next_seq(),
            });
        }
        let oldest = self.oldest_seq();
        if requested < oldest {
            return Err(PtyError::ReplayEvicted { requested, oldest });
        }
        let skip = usize::try_from(requested.0 - oldest.0).unwrap_or(usize::MAX);
        Ok(self.chunks.iter().skip(skip).cloned().collect())
    }

    /// First chunk still held; equals `next_seq` when the ring is empty.
    pub fn oldest_seq(&self) -> Seq {
        self.chunks.front().map_or(self.next_seq(), |c| c.seq)
    }

    pub fn next_seq(&self) -> Seq {
        Seq(self.next_seq)
    }

    pub fn held_bytes(&self) -> usize {
        self.held_bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seqs(chunks: &[OutputChunk]) -> Vec<u64> {
        chunks.iter().map(|c| c.seq.0).collect()
    }

    fn bytes(chunks: &[OutputChunk]) -> Vec<u8> {
        chunks
            .iter()
            .flat_map(|c| c.bytes.iter().copied())
            .collect()
    }

    #[test]
    fn numbers_chunks_from_one_without_gaps() {
        let mut ring = ReplayRing::new(64);
        ring.push(b"ab");
        ring.push(b"cd");
        ring.push(b"ef");
        let all = ring.replay(ReplayFrom::Oldest).unwrap();
        assert_eq!(seqs(&all), [1, 2, 3]);
        assert_eq!(bytes(&all), b"abcdef");
    }

    #[test]
    fn replays_from_a_seq_inclusive() {
        let mut ring = ReplayRing::new(64);
        for part in [b"a", b"b", b"c", b"d"] {
            ring.push(part);
        }
        let tail = ring.replay(ReplayFrom::Seq(Seq(3))).unwrap();
        assert_eq!(seqs(&tail), [3, 4]);
        assert_eq!(bytes(&tail), b"cd");
    }

    #[test]
    fn replay_from_next_seq_is_empty() {
        let mut ring = ReplayRing::new(64);
        ring.push(b"a");
        assert!(ring.replay(ReplayFrom::Seq(Seq(2))).unwrap().is_empty());
    }

    #[test]
    fn empty_push_takes_no_seq() {
        let mut ring = ReplayRing::new(64);
        assert!(ring.push(b"").is_empty());
        assert_eq!(ring.next_seq(), Seq(1));
    }

    #[test]
    fn evicts_oldest_and_stays_within_capacity() {
        let mut ring = ReplayRing::new(10);
        for _ in 0..50 {
            ring.push(b"1234");
            assert!(ring.held_bytes() <= 10);
        }
        let all = ring.replay(ReplayFrom::Oldest).unwrap();
        assert_eq!(seqs(&all), [49, 50]);
        assert_eq!(ring.oldest_seq(), Seq(49));
    }

    #[test]
    fn refuses_a_seq_that_left_the_ring() {
        let mut ring = ReplayRing::new(4);
        ring.push(b"aaaa");
        ring.push(b"bbbb");
        assert_eq!(
            ring.replay(ReplayFrom::Seq(Seq(1))),
            Err(PtyError::ReplayEvicted {
                requested: Seq(1),
                oldest: Seq(2)
            })
        );
    }

    #[test]
    fn refuses_a_seq_ahead_of_the_stream() {
        let mut ring = ReplayRing::new(4);
        ring.push(b"a");
        assert_eq!(
            ring.replay(ReplayFrom::Seq(Seq(9))),
            Err(PtyError::ReplayAhead {
                requested: Seq(9),
                next: Seq(2)
            })
        );
    }

    #[test]
    fn splits_data_larger_than_the_ring() {
        let mut ring = ReplayRing::new(4);
        let added = ring.push(b"0123456789");
        assert_eq!(seqs(&added), [1, 2, 3]);
        assert_eq!(bytes(&added), b"0123456789");
        assert!(ring.held_bytes() <= 4);
        assert_eq!(bytes(&ring.replay(ReplayFrom::Oldest).unwrap()), b"89");
    }
}
