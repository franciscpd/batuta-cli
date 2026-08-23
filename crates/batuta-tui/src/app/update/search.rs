use crate::{app::model::SessionDetail, transcript::PresentationRow};

/// Recomputes `detail.view.search.matches` from the current query against
/// the presentation rows' plain text. No-op when search is inactive.
pub(crate) fn recompute_search(detail: &mut SessionDetail) {
    let Some(search) = detail.view.search.as_mut() else {
        return;
    };
    let needle = search.query.to_lowercase();
    let entries = detail.transcript.entries();
    let matches = detail
        .transcript
        .presentation_rows(detail.view.raw_debug)
        .iter()
        .enumerate()
        .filter(|(_, row)| {
            if needle.is_empty() {
                return false;
            }
            let haystack = match row {
                PresentationRow::Entry { entry_index } => entries
                    .get(*entry_index)
                    .map(|entry| crate::transcript::entry_plain_text(entry))
                    .unwrap_or_default(),
                PresentationRow::Group {
                    entry_indexes,
                    label,
                } => {
                    let mut text = label.clone();
                    for index in entry_indexes {
                        if let Some(entry) = entries.get(*index) {
                            text.push('\n');
                            text.push_str(&crate::transcript::entry_plain_text(entry));
                        }
                    }
                    text
                }
            };
            haystack.to_lowercase().contains(&needle)
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    let Some(search) = detail.view.search.as_mut() else {
        return;
    };
    search.current = search.current.min(matches.len().saturating_sub(1));
    search.matches = matches;
}
