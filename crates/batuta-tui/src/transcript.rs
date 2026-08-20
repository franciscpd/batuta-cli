use compozy_client::{
    StreamCursor,
    types::{Entry, Part, Role, TranscriptDelta, TranscriptPage, TranscriptSnapshot},
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PresentationRow {
    Entry {
        entry_index: usize,
    },
    Group {
        entry_indexes: Vec<usize>,
        label: String,
    },
}

impl PresentationRow {
    pub fn first_entry_index(&self) -> usize {
        match self {
            Self::Entry { entry_index } => *entry_index,
            Self::Group { entry_indexes, .. } => entry_indexes[0],
        }
    }
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

    /// Builds a lossless, display-only view of adjacent operational entries.
    /// Raw entries remain authoritative and debug mode always exposes them directly.
    pub fn presentation_rows(&self, raw_debug: bool) -> Vec<PresentationRow> {
        let entries = self.entries();
        if raw_debug {
            return (0..entries.len())
                .map(|entry_index| PresentationRow::Entry { entry_index })
                .collect();
        }

        let mut rows = Vec::new();
        let mut index = 0;
        while index < entries.len() {
            let Some(kind) = operational_kind(entries[index]) else {
                rows.push(PresentationRow::Entry { entry_index: index });
                index += 1;
                continue;
            };
            let mut end = index + 1;
            while end < entries.len()
                && operational_kind(entries[end]).as_deref() == Some(kind.as_str())
            {
                end += 1;
            }
            if end - index >= 2 {
                rows.push(PresentationRow::Group {
                    entry_indexes: (index..end).collect(),
                    label: kind,
                });
            } else {
                rows.push(PresentationRow::Entry { entry_index: index });
            }
            index = end;
        }
        rows
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

fn operational_kind(entry: &Entry) -> Option<String> {
    if matches!(entry.message.role, Role::User) {
        return None;
    }
    if entry.message.parts.len() != 1 {
        return None;
    }
    match &entry.message.parts[0] {
        Part::Tool {
            state, error_text, ..
        } if error_text.is_none() && !is_failed_tool_state(state.as_deref()) => Some(format!(
            "tool updates · {}",
            state.as_deref().unwrap_or("unknown")
        )),
        Part::Event { data } => Some(format!(
            "telemetry events · {}",
            data.get("type")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown")
        )),
        _ => None,
    }
}

fn is_failed_tool_state(state: Option<&str>) -> bool {
    matches!(state, Some("output-error"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use compozy_client::types::{Role, UiMessage};

    fn entry(sequence: i64, part: Part) -> Entry {
        Entry {
            start_sequence: sequence,
            sequence,
            message: UiMessage {
                id: format!("message-{sequence}"),
                role: Role::Assistant,
                metadata: None,
                parts: vec![part],
            },
        }
    }

    fn transcript(entries: impl IntoIterator<Item = Entry>) -> TranscriptState {
        TranscriptState {
            entries: entries
                .into_iter()
                .map(|entry| (entry.start_sequence, entry))
                .collect(),
            ..TranscriptState::default()
        }
    }

    #[test]
    fn groups_adjacent_compatible_tools_losslessly() {
        let transcript = transcript((1..=6).map(|sequence| {
            entry(
                sequence,
                Part::Tool {
                    name: "status".into(),
                    tool_call_id: None,
                    state: Some("completed".into()),
                    input: None,
                    output: None,
                    error_text: None,
                    title: None,
                },
            )
        }));

        assert_eq!(
            transcript.presentation_rows(false),
            vec![PresentationRow::Group {
                entry_indexes: (0..6).collect(),
                label: "tool updates · completed".into(),
            }]
        );
        assert_eq!(transcript.presentation_rows(true).len(), 6);
    }

    #[test]
    fn errors_and_incompatible_states_break_operational_groups() {
        let transcript = transcript([
            entry(
                1,
                Part::Tool {
                    name: "status".into(),
                    tool_call_id: None,
                    state: Some("running".into()),
                    input: None,
                    output: None,
                    error_text: None,
                    title: None,
                },
            ),
            entry(
                2,
                Part::Tool {
                    name: "status".into(),
                    tool_call_id: None,
                    state: Some("completed".into()),
                    input: None,
                    output: None,
                    error_text: None,
                    title: None,
                },
            ),
            entry(
                3,
                Part::Tool {
                    name: "status".into(),
                    tool_call_id: None,
                    state: Some("completed".into()),
                    input: None,
                    output: None,
                    error_text: Some("failed".into()),
                    title: None,
                },
            ),
        ]);

        assert!(
            transcript
                .presentation_rows(false)
                .iter()
                .all(|row| matches!(row, PresentationRow::Entry { .. }))
        );
    }

    #[test]
    fn failed_tools_without_error_text_are_not_grouped() {
        let transcript = transcript((1..=2).map(|sequence| {
            entry(
                sequence,
                Part::Tool {
                    name: "deploy".into(),
                    tool_call_id: None,
                    state: Some("output-error".into()),
                    input: None,
                    output: None,
                    error_text: None,
                    title: None,
                },
            )
        }));

        assert_eq!(
            transcript.presentation_rows(false),
            vec![
                PresentationRow::Entry { entry_index: 0 },
                PresentationRow::Entry { entry_index: 1 },
            ]
        );
    }
}
