use compozy_client::{
    StreamCursor,
    types::{Entry, TranscriptDelta, TranscriptPage, TranscriptSnapshot},
};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Applied {
    Ok,
    UnknownStart(i64),
    FenceMismatch,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InlineMarker {
    pub reason: String,
}

#[derive(Clone, Debug, Default)]
pub struct TranscriptState {
    entries: BTreeMap<i64, Entry>,
    pub epoch: i64,
    pub generation: i64,
    pub max_sequence: i64,
    pub has_older: bool,
    pub next_before_sequence: Option<i64>,
    pub markers: Vec<InlineMarker>,
}

impl TranscriptState {
    pub fn entries(&self) -> Vec<&Entry> {
        self.entries.values().collect()
    }

    pub fn entry(&self, start_sequence: i64) -> Option<&Entry> {
        self.entries.get(&start_sequence)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn apply_snapshot(&mut self, snapshot: TranscriptSnapshot) -> Applied {
        self.entries = snapshot
            .entries
            .into_iter()
            .map(|entry| (entry.start_sequence, entry))
            .collect();
        self.epoch = snapshot.epoch;
        self.generation = snapshot.generation;
        self.max_sequence = snapshot.max_sequence;
        self.has_older = snapshot.has_older;
        self.next_before_sequence = snapshot.next_before_sequence;
        if snapshot.reset {
            self.markers.push(InlineMarker {
                reason: snapshot.reason.unwrap_or_else(|| "reset".to_owned()),
            });
        }
        Applied::Ok
    }

    pub fn apply_delta(&mut self, delta: TranscriptDelta) -> Applied {
        if delta.epoch != self.epoch || delta.generation != self.generation {
            return Applied::FenceMismatch;
        }
        if let Some(entry) = delta
            .entries
            .iter()
            .find(|entry| !self.entries.contains_key(&entry.start_sequence))
        {
            return Applied::UnknownStart(entry.start_sequence);
        }
        for entry in delta.entries {
            self.entries.insert(entry.start_sequence, entry);
        }
        self.max_sequence = self.max_sequence.max(delta.max_sequence).max(delta.cursor);
        Applied::Ok
    }

    pub fn prepend_page(&mut self, page: TranscriptPage) {
        if self.entries.is_empty() {
            self.epoch = page.epoch;
            self.generation = page.generation;
        }
        self.max_sequence = self.max_sequence.max(page.max_sequence);
        self.has_older = page.has_older;
        self.next_before_sequence = page.next_before_sequence;
        for entry in page.entries {
            self.entries.entry(entry.start_sequence).or_insert(entry);
        }
    }

    pub const fn cursor(&self) -> StreamCursor {
        StreamCursor {
            after_sequence: self.max_sequence,
            epoch: self.epoch,
            generation: self.generation,
        }
    }
}
