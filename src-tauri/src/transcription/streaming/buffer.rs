//! Ring buffer for accumulating audio samples in the streaming pipeline.

/// A fixed-capacity ring buffer for f32 audio samples.
#[derive(Debug)]
pub(crate) struct RingBuffer {
    data: Vec<f32>,
    capacity: usize,
    write_pos: usize,
    len: usize,
}

impl RingBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            data: vec![0.0; capacity],
            capacity,
            write_pos: 0,
            len: 0,
        }
    }

    /// Push samples into the buffer, overwriting oldest data when full.
    pub fn push(&mut self, samples: &[f32]) {
        for &s in samples {
            self.data[self.write_pos] = s;
            self.write_pos = (self.write_pos + 1) % self.capacity;
            if self.len < self.capacity {
                self.len += 1;
            }
        }
    }

    /// Read the last `count` samples in chronological order.
    /// Returns fewer if the buffer has fewer than `count` samples.
    pub fn read_last(&self, count: usize) -> Vec<f32> {
        let n = count.min(self.len);
        if n == 0 {
            return vec![];
        }
        let start = if self.len < self.capacity {
            // Buffer not yet wrapped.
            self.len.saturating_sub(n)
        } else {
            // Buffer has wrapped; write_pos points to the oldest sample.
            (self.write_pos + self.capacity - n) % self.capacity
        };

        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            out.push(self.data[(start + i) % self.capacity]);
        }
        out
    }

    pub fn len(&self) -> usize {
        self.len
    }

    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.write_pos = 0;
        self.len = 0;
        // No need to zero-fill; push overwrites.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ring_buffer_basic() {
        let mut rb = RingBuffer::new(10);
        assert_eq!(rb.len(), 0);
        rb.push(&[1.0, 2.0, 3.0]);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.read_last(3), vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn test_ring_buffer_wraps() {
        let mut rb = RingBuffer::new(4);
        rb.push(&[1.0, 2.0, 3.0, 4.0]);
        assert_eq!(rb.read_last(4), vec![1.0, 2.0, 3.0, 4.0]);
        rb.push(&[5.0, 6.0]);
        // Should now contain [3, 4, 5, 6]
        assert_eq!(rb.read_last(4), vec![3.0, 4.0, 5.0, 6.0]);
    }

    #[test]
    fn test_ring_buffer_read_more_than_available() {
        let mut rb = RingBuffer::new(10);
        rb.push(&[1.0, 2.0]);
        let out = rb.read_last(5);
        assert_eq!(out, vec![1.0, 2.0]);
    }

    #[test]
    fn test_ring_buffer_clear() {
        let mut rb = RingBuffer::new(10);
        rb.push(&[1.0, 2.0, 3.0]);
        rb.clear();
        assert_eq!(rb.len(), 0);
        assert_eq!(rb.read_last(10), Vec::<f32>::new());
    }
}
