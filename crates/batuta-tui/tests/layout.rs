use batuta_tui::{
    app::{AppMode, Model, Overlay, Panel, Settings},
    msg::Msg,
    update,
    views::{
        self,
        layout::{self, LayoutMode},
    },
};
use ratatui::{Terminal, backend::TestBackend};
fn render(model: &Model, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| views::view(model, frame)).unwrap();
    let buffer = terminal.backend().buffer();
    (0..height)
        .map(|y| {
            let mut line = String::new();
            for x in 0..width {
                line.push_str(buffer[(x, y)].symbol());
            }
            line.trim_end().to_owned()
        })
        .collect::<Vec<_>>()
        .join("\n")
}
#[test]
fn ut_430_ut_431_ut_432_and_ut_434_layout_modes() {
    assert_eq!(layout::mode(72, 20), LayoutMode::TooSmall);
    assert_eq!(layout::mode(80, 24), LayoutMode::Compact);
    assert_eq!(layout::mode(99, 40), LayoutMode::Compact);
    assert_eq!(layout::mode(100, 24), LayoutMode::Full);
    let model = Model::new(Settings::default(), AppMode::Full);
    for (name, width, height) in [
        ("too_small", 72, 20),
        ("compact", 80, 24),
        ("full_100", 100, 30),
        ("full_120", 120, 40),
        ("full_200", 200, 60),
    ] {
        insta::assert_snapshot!(format!("layout_{name}"), render(&model, width, height));
    }
}
#[test]
fn ut_433_resize_closes_overlay_and_preserves_panels() {
    let mut model = Model::new(Settings::default(), AppMode::Full);
    model.focus = Panel::Runs;
    model.overlay = Some(Overlay::Help { scroll: 0 });
    update(&mut model, Msg::Resize(72, 20));
    assert!(model.overlay.is_none());
    update(&mut model, Msg::Resize(100, 30));
    assert_eq!(model.focus, Panel::Runs);
}

#[test]
fn compact_layout_allocates_content_to_only_the_focused_panel() {
    let mut model = Model::new(Settings::default(), AppMode::Full);
    let content = layout::areas(ratatui::layout::Rect::new(0, 0, 90, 30), &model);
    assert_eq!(content.sessions.width, 90);
    assert_eq!(content.detail.width, 0);

    model.focus = Panel::Detail;
    let content = layout::areas(ratatui::layout::Rect::new(0, 0, 90, 30), &model);
    assert_eq!(content.sessions.width, 0);
    assert_eq!(content.runs.width, 0);
    assert_eq!(content.attention.width, 0);
    assert_eq!(content.detail.width, 90);
}
