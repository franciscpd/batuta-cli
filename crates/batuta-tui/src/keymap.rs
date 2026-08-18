pub const FOOTER: &str = "j/k move  Enter expand  t reasoning  G newest  g oldest  q quit";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Binding {
    pub keys: &'static str,
    pub action: &'static str,
}

pub const BINDINGS: &[Binding] = &[
    Binding {
        keys: "j/k",
        action: "move",
    },
    Binding {
        keys: "Enter",
        action: "expand",
    },
    Binding {
        keys: "t",
        action: "reasoning",
    },
    Binding {
        keys: "G",
        action: "newest",
    },
    Binding {
        keys: "g",
        action: "oldest",
    },
    Binding {
        keys: "q",
        action: "quit",
    },
];

pub fn footer() -> String {
    BINDINGS
        .iter()
        .map(|binding| format!("{} {}", binding.keys, binding.action))
        .collect::<Vec<_>>()
        .join("  ")
}
