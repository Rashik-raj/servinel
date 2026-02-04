use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum LogStream {
    Stdout,
    Stderr,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: u64,
    pub stream: LogStream,
    pub line: String,
}

#[derive(Debug, Clone)]
pub struct LogBuffer {
    entries: VecDeque<LogEntry>,
    capacity: usize,
}

impl LogBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    pub fn push(&mut self, entry: LogEntry) {
        if self.entries.len() == self.capacity {
            self.entries.pop_front();
        }
        self.entries.push_back(entry);
    }

    pub fn tail(&self, count: usize) -> Vec<LogEntry> {
        let count = count.min(self.entries.len());
        self.entries
            .iter()
            .skip(self.entries.len() - count)
            .cloned()
            .collect()
    }

    pub fn all(&self) -> Vec<LogEntry> {
        self.entries.iter().cloned().collect()
    }
}

impl Default for LogBuffer {
    fn default() -> Self {
        Self::new(1000)
    }
}
