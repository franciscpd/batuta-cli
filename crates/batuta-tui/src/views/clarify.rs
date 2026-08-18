use crate::app::Overlay;

pub fn content(overlay: &Overlay) -> Option<(String, String)> {
    let Overlay::Clarify {
        question,
        deadline,
        choices,
        selected,
        text,
        hint,
        ..
    } = overlay
    else {
        return None;
    };
    let deadline = deadline.as_ref().map_or(String::new(), |value| {
        format!(" · deadline {}", value.raw())
    });
    let mut body = format!("{question}\n");
    if choices.is_empty() {
        body.push_str(&format!("answer: {text}"));
    } else {
        body.push_str(
            &choices
                .iter()
                .enumerate()
                .map(|(index, choice)| {
                    format!(
                        "{} {}  {}",
                        if index == *selected { ">" } else { " " },
                        index + 1,
                        choice
                    )
                })
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }
    if let Some(hint) = hint {
        body.push_str(&format!("\n{hint}"));
    }
    body.push_str("\nEnter · Esc");
    Some((format!("clarify{deadline}"), body))
}
