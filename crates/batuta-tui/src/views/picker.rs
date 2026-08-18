use crate::app::{Model, Overlay};

pub fn content(model: &Model) -> Option<(String, String)> {
    let Overlay::WorkspacePicker {
        selected, items, ..
    } = model.overlay.as_ref()?
    else {
        return None;
    };
    let text = if items.is_empty() {
        "no workspaces — register one with `compozy workspace add`".to_owned()
    } else {
        items
            .iter()
            .enumerate()
            .map(|(index, item)| {
                format!(
                    "{} {}  {}",
                    if Some(index) == *selected { ">" } else { " " },
                    item.name,
                    item.root_dir
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    Some(("workspaces".into(), text))
}
