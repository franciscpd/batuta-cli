#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Context {
    Global,
    Lists,
    Sessions,
    Attention,
    SessionDetail,
    Composer,
    RunDetail,
    Overlays,
    Logs,
    Chooser,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Binding {
    pub context: Context,
    pub keys: &'static str,
    pub action: &'static str,
}

pub const BINDINGS: &[Binding] = &[
    Binding {
        context: Context::Global,
        keys: "1-4",
        action: "focus",
    },
    Binding {
        context: Context::Global,
        keys: "Tab/Shift+Tab",
        action: "next/previous",
    },
    Binding {
        context: Context::Global,
        keys: "w",
        action: "workspace",
    },
    Binding {
        context: Context::Global,
        keys: "L",
        action: "logs",
    },
    Binding {
        context: Context::Global,
        keys: "?/F1",
        action: "help",
    },
    Binding {
        context: Context::Global,
        keys: "q/Ctrl+C",
        action: "quit",
    },
    Binding {
        context: Context::Lists,
        keys: "j/k/↑/↓/PgUp/PgDn/g/G",
        action: "move",
    },
    Binding {
        context: Context::Lists,
        keys: "Enter",
        action: "open",
    },
    Binding {
        context: Context::Lists,
        keys: "/",
        action: "filter",
    },
    Binding {
        context: Context::Lists,
        keys: "*/r",
        action: "all/refresh",
    },
    Binding {
        context: Context::Lists,
        keys: "y",
        action: "yank id",
    },
    Binding {
        context: Context::Sessions,
        keys: "n",
        action: "new session",
    },
    Binding {
        context: Context::Attention,
        keys: "a/A/x/X/r/Enter/o",
        action: "respond/open",
    },
    Binding {
        context: Context::SessionDetail,
        keys: "j/k/PgUp/PgDn/g/G/Enter/t/y/n/N",
        action: "navigate",
    },
    Binding {
        context: Context::SessionDetail,
        keys: "i/Ctrl+X",
        action: "compose/cancel",
    },
    Binding {
        context: Context::SessionDetail,
        keys: "/",
        action: "search",
    },
    Binding {
        context: Context::Composer,
        keys: "Enter/Alt+Enter/Ctrl+U/Esc",
        action: "send/newline/clear/back",
    },
    Binding {
        context: Context::RunDetail,
        keys: "p/u/k/a/x/Enter",
        action: "control/open",
    },
    Binding {
        context: Context::Overlays,
        keys: "Esc",
        action: "close",
    },
    Binding {
        context: Context::Logs,
        keys: "e/L",
        action: "errors/close",
    },
    Binding {
        context: Context::Chooser,
        keys: "j/k/1-9/Enter/Esc",
        action: "choose/cancel",
    },
];

pub const HELP_ORDER: &[(Context, &str)] = &[
    (Context::Global, "Global"),
    (Context::Lists, "Lists"),
    (Context::SessionDetail, "Session detail"),
    (Context::Composer, "Composer"),
    (Context::RunDetail, "Run detail"),
    (Context::Attention, "Attention"),
    (Context::Overlays, "Overlays"),
];

pub fn bindings(context: Context) -> impl Iterator<Item = &'static Binding> {
    BINDINGS
        .iter()
        .filter(move |binding| binding.context == context)
}

pub fn footer(contexts: &[Context]) -> String {
    BINDINGS
        .iter()
        .filter(|binding| contexts.contains(&binding.context))
        .map(|binding| format!("{} {}", binding.keys, binding.action))
        .collect::<Vec<_>>()
        .join("  ")
}

pub fn help_lines() -> Vec<String> {
    let mut lines = Vec::new();
    for (context, title) in HELP_ORDER {
        let mut rows: Vec<_> = bindings(*context).collect();
        if *context == Context::Lists {
            rows.extend(bindings(Context::Sessions));
        }
        if *context == Context::Overlays {
            rows.extend(bindings(Context::Logs));
            rows.extend(bindings(Context::Chooser));
        }
        if rows.is_empty() {
            continue;
        }
        lines.push((*title).to_owned());
        for row in rows {
            lines.push(format!("  {:<24} {}", row.keys, row.action));
        }
        lines.push(String::new());
    }
    lines
}
