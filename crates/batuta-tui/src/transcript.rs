use compozy_client::{
    StreamCursor,
    types::{Entry, Part, Role, TranscriptDelta, TranscriptPage, TranscriptSnapshot},
};
use std::collections::BTreeMap;

/// Lossless-enough plain text for clipboard: text parts verbatim; tool parts
/// as "name\ninput\noutput\nerror"; everything else via its JSON value.
pub fn entry_plain_text(entry: &Entry) -> String {
    entry
        .message
        .parts
        .iter()
        .map(|part| match part {
            Part::Text { text, .. } => text.clone(),
            Part::Tool {
                name,
                input,
                output,
                error_text,
                ..
            } => {
                let mut text = name.clone();
                for value in [input, output].into_iter().flatten() {
                    text.push('\n');
                    text.push_str(
                        &value
                            .as_str()
                            .map(str::to_owned)
                            .unwrap_or_else(|| value.to_string()),
                    );
                }
                if let Some(error) = error_text {
                    text.push('\n');
                    text.push_str(error);
                }
                text
            }
            other => serde_json::to_string(other).unwrap_or_default(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

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
        // The stream resumes right after our cursor, so an unknown entry that
        // begins exactly at the next sequence is new tail content and can be
        // appended: forcing a page refetch there is what delayed new messages.
        // Any other unknown start is a real gap and still refetches.
        let mut frontier = self.max_sequence;
        for entry in &delta.entries {
            if self.entries.contains_key(&entry.start_sequence) {
                frontier = frontier.max(entry.sequence);
                continue;
            }
            // Observed against a live daemon: a following entry begins either at
            // the previous entry's sequence or one past it, never further ahead.
            if !(frontier..=frontier.saturating_add(1)).contains(&entry.start_sequence) {
                return Applied::UnknownStart(entry.start_sequence);
            }
            frontier = frontier.max(entry.sequence).max(entry.start_sequence);
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
    use compozy_client::types::{Role, TranscriptDelta, UiMessage};

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
    fn ut_763_entry_plain_text_extracts_text_and_tool_parts() {
        let text_entry = entry(
            1,
            Part::Text {
                text: "hello *world*".into(),
                state: None,
            },
        );
        assert_eq!(entry_plain_text(&text_entry), "hello *world*");
        let tool = entry(
            2,
            Part::Tool {
                name: "bash".into(),
                tool_call_id: None,
                state: Some("completed".into()),
                input: None,
                output: Some(serde_json::json!("ls -la")),
                error_text: None,
                title: None,
            },
        );
        let text = entry_plain_text(&tool);
        assert!(text.contains("bash"));
        assert!(text.contains("ls -la"));
    }

    #[test]
    fn ut_788_tail_appends_apply_and_history_gaps_still_refetch() {
        let mut state = transcript((1..=3).map(|sequence| {
            entry(
                sequence,
                Part::Text {
                    text: format!("entrada {sequence}"),
                    state: None,
                },
            )
        }));
        state.max_sequence = 3;

        // A brand new message arrives at the tail: append it, do not refetch.
        let applied = state.apply_delta(TranscriptDelta {
            entries: vec![entry(
                4,
                Part::Text {
                    text: "mensagem nova".into(),
                    state: None,
                },
            )],
            max_sequence: 4,
            cursor: 4,
            ..TranscriptDelta::default()
        });
        assert_eq!(applied, Applied::Ok);
        assert_eq!(state.len(), 4);
        assert!(state.entry(4).is_some());

        // Some entries begin exactly at the previous entry's sequence.
        let applied = state.apply_delta(TranscriptDelta {
            entries: vec![entry(
                4,
                Part::Text {
                    text: "mesma sequencia".into(),
                    state: None,
                },
            )],
            max_sequence: 4,
            cursor: 4,
            ..TranscriptDelta::default()
        });
        assert_eq!(applied, Applied::Ok);

        // A hole below the tail is a real gap: keep forcing a page refetch.
        let mut holed = transcript([entry(
            9,
            Part::Text {
                text: "tarde".into(),
                state: None,
            },
        )]);
        holed.max_sequence = 9;
        let applied = holed.apply_delta(TranscriptDelta {
            entries: vec![entry(
                5,
                Part::Text {
                    text: "faltando".into(),
                    state: None,
                },
            )],
            max_sequence: 9,
            cursor: 9,
            ..TranscriptDelta::default()
        });
        assert_eq!(applied, Applied::UnknownStart(5));

        // Without a baseline snapshot we must still refetch.
        let mut empty = TranscriptState::default();
        let applied = empty.apply_delta(TranscriptDelta {
            entries: vec![entry(
                2,
                Part::Text {
                    text: "sem base".into(),
                    state: None,
                },
            )],
            max_sequence: 2,
            cursor: 2,
            ..TranscriptDelta::default()
        });
        assert_eq!(applied, Applied::UnknownStart(2));
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
