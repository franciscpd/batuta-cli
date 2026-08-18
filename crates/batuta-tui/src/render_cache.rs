use crate::{
    app::{Model, RenderCacheKey},
    views::{cards::tool_status, markdown::markdown_text},
};
use compozy_client::types::{Entry, Part, Role};
use ratatui::{
    style::Style,
    text::{Line, Span, Text},
};
use serde_json::Value;

const PART_LIMIT: usize = 1024 * 1024;
const PAYLOAD_LINE_LIMIT: usize = 200;

pub fn rebuild(model: &mut Model) {
    let width = model.size.0.saturating_sub(4);
    let mut new_cache = std::collections::HashMap::new();
    for entry in model.transcript.entries() {
        let key = RenderCacheKey {
            start_sequence: entry.start_sequence,
            sequence: entry.sequence,
            width,
            reasoning_expanded: model.reasoning_expanded,
            expanded: model.expanded.contains(&entry.start_sequence),
            color: model.theme.color,
        };
        if let Some(cached) = model.render_cache.get(&key) {
            new_cache.insert(key, cached.clone());
        } else {
            new_cache.insert(key.clone(), render_entry(entry, model, key.expanded));
        }
    }
    model.render_cache = new_cache;
}

fn render_entry(entry: &Entry, model: &Model, expanded: bool) -> Text<'static> {
    let mut lines = Vec::new();
    let role = match &entry.message.role {
        Role::User => "user",
        Role::Assistant => model.header.agent.as_str(),
        Role::System => "system",
        Role::Unknown => "unknown",
        Role::Other(value) => value,
    };
    lines.push(Line::from(Span::styled(
        format!("▸ {role}"),
        model.theme.emphasis,
    )));
    for part in &entry.message.parts {
        render_part(part, model, expanded, &mut lines);
    }
    lines.push(Line::default());
    Text::from(wrap_lines(
        lines,
        usize::from(model.size.0.saturating_sub(1)).max(1),
    ))
}

fn render_part(part: &Part, model: &Model, expanded: bool, lines: &mut Vec<Line<'static>>) {
    if part_size(part) > PART_LIMIT {
        lines.push(indented(
            format!("[part too large: {} bytes]", part_size(part)),
            model.theme.warning,
        ));
        return;
    }
    match part {
        Part::Text { text, state } => {
            let mut rendered = markdown_text(text);
            if !model.theme.color {
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
                line.spans.push(Span::styled("▍", model.theme.emphasis));
            }
        }
        Part::Reasoning { text, .. } if !model.reasoning_expanded => {
            lines.push(indented(
                format!("▸ reasoning ({} lines)", text.lines().count().max(1)),
                model.theme.muted,
            ));
        }
        Part::Reasoning { text, .. } => {
            lines.push(indented("▾ reasoning".to_owned(), model.theme.muted));
            lines.extend(
                text.lines()
                    .map(|line| indented(format!("  {line}"), model.theme.muted)),
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
            let (glyph, style) = tool_status(state.as_deref(), &model.theme);
            let summary = input.as_ref().and_then(first_scalar).unwrap_or_default();
            lines.push(indented(
                format!(
                    "{glyph} {name}{}",
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
                    payload("input", input, model.theme.muted, lines);
                }
                if let Some(output) = output {
                    payload("output", output, model.theme.muted, lines);
                }
                if let Some(error) = error_text {
                    lines.push(indented("  error".to_owned(), model.theme.error));
                    append_truncated(error, model.theme.error, lines);
                }
            }
        }
        Part::Marker { kind, summary, .. } => {
            let kind = kind.as_deref().unwrap_or("marker");
            let summary = summary.as_deref().unwrap_or("");
            if kind == "file_mutation_unverified" {
                lines.push(indented(
                    format!("! {kind} · {summary}"),
                    model.theme.warning,
                ));
            } else {
                lines.push(indented(
                    format!("─ {kind} · {summary} ─"),
                    model.theme.muted,
                ));
            }
        }
        Part::Permission { data } => {
            let request = data
                .get("request_id")
                .and_then(Value::as_str)
                .unwrap_or("?");
            let turn = data.get("turn_id").and_then(Value::as_str).unwrap_or("?");
            let summary = data
                .get("summary")
                .or_else(|| data.get("title"))
                .and_then(Value::as_str)
                .unwrap_or("approval requested");
            let decision = data
                .get("decision")
                .and_then(Value::as_str)
                .map(|value| format!("  {value}"))
                .unwrap_or_default();
            lines.push(indented(format!("▪ approval  request_id {request}  turn {turn}  {summary}  read-only here{decision}"), model.theme.warning));
            if expanded {
                payload("request", data, model.theme.muted, lines);
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
            model.theme.default,
        )),
        Part::Event { data } => lines.push(indented(
            format!(
                "event: {}",
                data.get("type")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
            ),
            model.theme.muted,
        )),
        Part::Unknown { type_ } => lines.push(indented(
            format!("unknown part: {type_}"),
            model.theme.muted,
        )),
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
    append_truncated(&text, style, lines);
}

fn append_truncated(text: &str, style: Style, lines: &mut Vec<Line<'static>>) {
    for line in text.lines().take(PAYLOAD_LINE_LIMIT) {
        lines.push(indented(format!("    {line}"), style));
    }
    if text.lines().count() > PAYLOAD_LINE_LIMIT {
        lines.push(indented("    … truncated".to_owned(), style));
    }
}

fn part_size(part: &Part) -> usize {
    match part {
        Part::Text { text, .. } | Part::Reasoning { text, .. } => text.len(),
        _ => serde_json::to_vec(part).map_or(0, |bytes| bytes.len()),
    }
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
