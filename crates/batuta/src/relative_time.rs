use time::OffsetDateTime;

pub fn render(value: Option<OffsetDateTime>, now: OffsetDateTime) -> String {
    let Some(value) = value else {
        return "—".to_owned();
    };
    let seconds = (now - value).whole_seconds().max(0);
    match seconds {
        0..=59 => format!("{seconds}s ago"),
        60..=3_599 => format!("{}m ago", seconds / 60),
        3_600..=86_399 => format!("{}h ago", seconds / 3_600),
        _ => format!("{}d ago", seconds / 86_400),
    }
}
