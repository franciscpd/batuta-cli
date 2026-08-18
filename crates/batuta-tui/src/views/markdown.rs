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
