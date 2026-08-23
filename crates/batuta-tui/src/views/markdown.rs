use ratatui::text::{Line, Span, Text};

pub fn markdown_text(input: &str) -> Text<'static> {
    let rendered = tui_markdown::from_str(input);
    Text::from(
        rendered
            .lines
            .into_iter()
            .map(|line| Line {
                style: line.style,
                alignment: line.alignment,
                spans: line
                    .spans
                    .into_iter()
                    .map(|span| Span::styled(span.content.into_owned(), span.style))
                    .collect(),
            })
            .collect::<Vec<_>>(),
    )
}

/// Markdown collapses single newlines into one paragraph; the web app
/// preserves the line breaks a person typed into the composer. Appending
/// the two-space hard-break marker outside fenced code blocks keeps real
/// markdown rendering while preserving those breaks.
pub fn preserve_line_breaks(input: &str) -> String {
    let mut in_fence = false;
    let mut out = Vec::new();
    for line in input.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            out.push(line.to_owned());
        } else if in_fence || line.trim().is_empty() {
            out.push(line.to_owned());
        } else {
            out.push(format!("{line}  "));
        }
    }
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::preserve_line_breaks;

    #[test]
    fn ut_786_preserve_line_breaks_marks_prose_and_skips_fences() {
        assert_eq!(preserve_line_breaks("a\nb"), "a  \nb  ");
        assert_eq!(
            preserve_line_breaks("a\n```\nx\ny\n```\nb"),
            "a  \n```\nx\ny\n```\nb  "
        );
        assert_eq!(preserve_line_breaks("a\n\nb"), "a  \n\nb  ");
    }
}
