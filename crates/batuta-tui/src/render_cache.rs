use crate::{
    app::{Model, RenderCacheKey},
    theme::Theme,
    views::{cards::tool_status, markdown::markdown_text},
};
use compozy_client::types::{Entry, Part, Role};
use ratatui::{
    style::Style,
    text::{Line, Span, Text},
};
use serde_json::Value;

pub fn rebuild(model: &mut Model) {
    let width = model.size.0.saturating_sub(4);
    let theme = model.theme.clone();
    let Some(detail) = model.session_detail_mut() else {
        return;
    };
    let mut new_cache = std::collections::HashMap::new();
    for entry in detail.transcript.entries() {
        let key = RenderCacheKey {
            start_sequence: entry.start_sequence,
            sequence: entry.sequence,
            width,
            reasoning_expanded: detail.view.reasoning_expanded,
            raw_debug: detail.view.raw_debug,
            expanded: detail.view.expanded.contains(&entry.start_sequence),
            color: theme.color,
            theme: theme.variant,
        };
        if let Some(cached) = detail.view.render_cache.get(&key) {
            new_cache.insert(key, cached.clone());
        } else {
            new_cache.insert(
                key.clone(),
                render_entry(
                    entry,
                    &detail.session.agent_name,
                    &theme,
                    width,
                    key.reasoning_expanded,
                    key.raw_debug,
                    key.expanded,
                ),
            );
        }
    }
    detail.view.render_cache = new_cache;
    detail.view.cache_dirty = false;
}

fn render_entry(
    entry: &Entry,
    agent: &str,
    theme: &Theme,
    width: u16,
    reasoning_expanded: bool,
    raw_debug: bool,
    expanded: bool,
) -> Text<'static> {
    let mut lines = Vec::new();
    let role = match &entry.message.role {
        Role::User => "user",
        Role::Assistant => agent,
        Role::System => "system",
        Role::Unknown => "unknown",
        Role::Other(value) => value,
    };
    lines.push(Line::from(Span::styled(
        format!("▸ {role}"),
        theme.emphasis,
    )));
    if raw_debug {
        for (index, part) in entry.message.parts.iter().enumerate() {
            lines.push(indented(format!("part[{index}] raw"), theme.muted));
            append_payload(&serialize_part(part), theme.muted, &mut lines);
        }
        lines.push(Line::default());
        return Text::from(wrap_lines(
            lines,
            usize::from(width.saturating_add(3)).max(1),
        ));
    }
    for part in &entry.message.parts {
        render_part(part, theme, reasoning_expanded, expanded, &mut lines);
    }
    lines.push(Line::default());
    Text::from(wrap_lines(
        lines,
        usize::from(width.saturating_add(3)).max(1),
    ))
}

fn render_part(
    part: &Part,
    theme: &Theme,
    reasoning_expanded: bool,
    expanded: bool,
    lines: &mut Vec<Line<'static>>,
) {
    match part {
        Part::Text { text, state } => {
            let mut rendered = markdown_text(text);
            if !theme.color {
                for line in &mut rendered.lines {
                    line.style = modifiers_only(line.style);
                    for span in &mut line.spans {
                        span.style = modifiers_only(span.style);
                    }
                }
            }
            for line in &mut rendered.lines {
                line.spans.insert(0, Span::raw("  "));
            }
            lines.extend(rendered.lines);
            if state.as_deref() == Some("streaming")
                && let Some(line) = lines.last_mut()
            {
                line.spans.push(Span::styled("▍", theme.emphasis));
            }
        }
        Part::Reasoning { text, .. } if !reasoning_expanded => {
            lines.push(indented(
                format!("▸ reasoning ({} lines)", text.lines().count().max(1)),
                theme.muted,
            ));
        }
        Part::Reasoning { text, .. } => {
            lines.push(indented("▾ reasoning".to_owned(), theme.muted));
            lines.extend(
                text.lines()
                    .map(|line| indented(format!("  {line}"), theme.muted)),
            );
        }
        Part::Tool {
            name,
            state,
            input,
            output,
            error_text,
            ..
        } => {
            let (activity, style) = tool_status(state.as_deref(), theme);
            let summary = input.as_ref().and_then(first_scalar).unwrap_or_default();
            lines.push(indented(
                format!(
                    "{} {} {name}{}",
                    activity.marker(),
                    activity.label(),
                    if summary.is_empty() {
                        String::new()
                    } else {
                        format!("   {summary}")
                    }
                ),
                style,
            ));
            if expanded {
                if let Some(input) = input {
                    payload("input", input, theme.muted, lines);
                }
                if let Some(output) = output {
                    payload("output", output, theme.muted, lines);
                }
                if let Some(error) = error_text {
                    lines.push(indented("  error".to_owned(), theme.error));
                    append_payload(error, theme.error, lines);
                }
            }
        }
        Part::Marker { kind, summary, .. } => {
            let kind = kind.as_deref().unwrap_or("marker");
            let summary = summary.as_deref().unwrap_or("");
            if kind == "file_mutation_unverified" {
                lines.push(indented(format!("! {kind} · {summary}"), theme.warning));
            } else {
                lines.push(indented(format!("─ {kind} · {summary} ─"), theme.muted));
            }
        }
        Part::Permission { data } => {
            let request = data
                .get("request_id")
                .and_then(Value::as_str)
                .unwrap_or("?");
            let turn = data.get("turn_id").and_then(Value::as_str).unwrap_or("?");
            let summary = data
                .get("title")
                .or_else(|| data.get("action"))
                .and_then(Value::as_str)
                .unwrap_or("approval requested");
            let input = data
                .get("raw")
                .and_then(|raw| raw.get("tool_input"))
                .and_then(first_scalar)
                .unwrap_or_default();
            let always = data
                .get("raw")
                .and_then(|raw| raw.get("options"))
                .and_then(Value::as_array)
                .is_some_and(|options| {
                    options.iter().any(|option| {
                        matches!(
                            option.get("decision").and_then(Value::as_str),
                            Some("allow-always" | "reject-always")
                        )
                    })
                });
            let decision = data
                .get("decision")
                .and_then(Value::as_str)
                .map(|value| format!("  {value}"))
                .unwrap_or_default();
            lines.push(indented(
                format!("▪ approval  request {request}  turn {turn}{decision}"),
                theme.warning,
            ));
            lines.push(indented(
                format!(
                    "  {summary}{}",
                    if input.is_empty() {
                        String::new()
                    } else {
                        format!(" · {input}")
                    }
                ),
                theme.default,
            ));
            lines.push(indented(
                if always {
                    "  a allow once · A allow always · x reject once · X reject always"
                } else {
                    "  a allow once · x reject once"
                }
                .to_owned(),
                theme.muted,
            ));
            if expanded {
                payload("request", data, theme.muted, lines);
            }
        }
        Part::File {
            filename,
            media_type,
            ..
        } => lines.push(indented(
            format!(
                "file: {} ({})",
                filename.as_deref().unwrap_or("?"),
                media_type.as_deref().unwrap_or("unknown")
            ),
            theme.default,
        )),
        Part::Event { data } => lines.push(indented(
            format!(
                "event: {}",
                data.get("type")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
            ),
            theme.muted,
        )),
        Part::Unknown { type_ } => {
            lines.push(indented(format!("unknown part: {type_}"), theme.muted))
        }
    }
}

fn indented(value: String, style: Style) -> Line<'static> {
    Line::from(Span::styled(format!("  {value}"), style))
}

fn modifiers_only(style: Style) -> Style {
    Style::default()
        .add_modifier(style.add_modifier)
        .remove_modifier(style.sub_modifier)
}

fn first_scalar(value: &Value) -> Option<String> {
    value
        .as_object()?
        .iter()
        .find_map(|(_, value)| match value {
            Value::String(value) => Some(value.clone()),
            Value::Number(value) => Some(value.to_string()),
            Value::Bool(value) => Some(value.to_string()),
            Value::Null => Some("null".into()),
            _ => None,
        })
}

fn payload(label: &str, value: &Value, style: Style, lines: &mut Vec<Line<'static>>) {
    lines.push(indented(format!("  {label}"), style));
    let text = match value {
        Value::String(value) => value.clone(),
        _ => serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string()),
    };
    append_payload(&text, style, lines);
}

fn append_payload(text: &str, style: Style, lines: &mut Vec<Line<'static>>) {
    for line in text.lines() {
        lines.push(indented(format!("    {line}"), style));
    }
}

fn serialize_part(part: &Part) -> String {
    serde_json::to_string_pretty(part).unwrap_or_else(|_| format!("{part:?}"))
}

fn wrap_lines(lines: Vec<Line<'static>>, width: usize) -> Vec<Line<'static>> {
    let mut wrapped = Vec::new();
    for line in lines {
        let content = line.to_string();
        let chars = content.chars().collect::<Vec<_>>();
        if chars.len() <= width || content.is_empty() {
            wrapped.push(line);
            continue;
        }
        let style = line
            .spans
            .first()
            .map_or(line.style, |span| line.style.patch(span.style));
        let indent = content
            .chars()
            .take_while(|character| character.is_whitespace())
            .collect::<String>();
        let mut remaining = chars;
        let mut first = true;
        while !remaining.is_empty() {
            let prefix = if first { "" } else { indent.as_str() };
            let available = width.saturating_sub(prefix.chars().count()).max(1);
            let split = if remaining.len() <= available {
                remaining.len()
            } else {
                remaining[..available]
                    .iter()
                    .rposition(|character| character.is_whitespace())
                    .filter(|position| *position > 0)
                    .unwrap_or(available)
            };
            let chunk = remaining.drain(..split).collect::<String>();
            while remaining
                .first()
                .is_some_and(|character| character.is_whitespace())
            {
                remaining.remove(0);
            }
            wrapped.push(Line::styled(format!("{prefix}{}", chunk.trim_end()), style));
            first = false;
        }
    }
    wrapped
}

#[cfg(test)]
mod tests {
    use super::*;
    use compozy_client::types::UiMessage;
    use serde_json::json;

    fn entry(parts: Vec<Part>) -> Entry {
        Entry {
            start_sequence: 1,
            sequence: 2,
            message: UiMessage {
                id: "message-1".into(),
                role: Role::Assistant,
                metadata: None,
                parts,
            },
        }
    }

    #[test]
    fn expanded_payloads_are_never_discarded_or_line_truncated() {
        let large = "x".repeat(1024 * 1024 + 1);
        let output = (0..201)
            .map(|line| format!("line-{line}"))
            .collect::<Vec<_>>()
            .join("\n");
        let rendered = render_entry(
            &entry(vec![
                Part::Text {
                    text: large.clone(),
                    state: None,
                },
                Part::Tool {
                    name: "tool".into(),
                    tool_call_id: None,
                    state: None,
                    input: None,
                    output: Some(json!(output)),
                    error_text: None,
                    title: None,
                },
            ]),
            "agent",
            &Theme::new(true),
            u16::MAX,
            false,
            false,
            true,
        );

        let rendered = rendered
            .lines
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(
            rendered
                .chars()
                .filter(|character| *character == 'x')
                .count(),
            large.len()
        );
        assert!(rendered.contains("line-200"));
        assert!(!rendered.contains("… truncated"));
        assert!(!rendered.contains("part too large"));
    }

    #[test]
    fn raw_debug_serializes_every_part_without_truncation() {
        let part = Part::Tool {
            name: "tool".into(),
            tool_call_id: None,
            state: None,
            input: Some(json!({"nested": {"unicode": "coração"}})),
            output: None,
            error_text: None,
            title: None,
        };
        let expected = serialize_part(&part);
        let rendered = render_entry(
            &entry(vec![part]),
            "agent",
            &Theme::new(true),
            u16::MAX,
            false,
            true,
            false,
        );

        let rendered = rendered
            .lines
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");

        let presented_raw = expected
            .lines()
            .map(|line| format!("      {line}"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered.contains(&presented_raw));
        assert!(!rendered.contains("… truncated"));
    }
}
