use batuta_tui::keymap::{self, BINDINGS, Keymap};
#[test]
fn ut_441_keymap_and_help_agree() {
    let help = keymap::help_lines(&Keymap::default()).join("\n");
    for binding in BINDINGS {
        assert!(help.contains(binding.keys), "missing {}", binding.keys);
        assert!(help.contains(binding.action), "missing {}", binding.action);
    }
}
#[test]
fn ut_442_help_scroll_content_is_long() {
    assert!(keymap::help_lines(&Keymap::default()).len() > 20);
}
