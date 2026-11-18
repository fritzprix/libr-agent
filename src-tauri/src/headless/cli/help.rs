//! User-facing help and version text for `liblet` (#2889).

/// Help text matching `docs/specs/liblet-cli-interface.md` §3.1 & §3.3.
pub fn help_text() -> String {
    concat!(
        "liblet - LibrAgent terminal & headless runner\n\n",
        "USAGE:\n",
        "  liblet [OPTIONS] [PROMPT]\n",
        "  liblet [OPTIONS] -p, --prompt <TEXT>\n",
        "  <command> | liblet [OPTIONS] [-p <TEXT>]\n\n",
        "CORE FLAGS:\n",
        "  -p, --prompt <text>           Run one prompt in headless mode and exit\n",
        "  -C, --cd <dir>                Workspace directory for a new session (default: cwd)\n",
        "      --assistant <id-or-name>  Assistant preset for a new session\n",
        "      --model <name>            Model override for this invocation\n",
        "      --provider <name>         Provider override for this invocation\n",
        "      --isolation <mode>        Workspace isolation: host (default) or docker\n",
        "      --execution-mode <mode>   Tool approval policy: normal (default), yolo, or unsafe\n",
        "      --output-format <fmt>     Headless stdout format: text (default), json, or stream-json\n",
        "      --timeout <seconds>       Wall-clock limit for a headless run\n",
        "      --headless                Run one command without opening the GUI (with -p or piped stdin)\n",
        "      --session <id>            Continue a persisted session (implies --headless)\n",
        "      --list-sessions           Print persisted sessions for the current directory and exit\n",
        "      --inspect-session <id>    Print a persisted session digest (turn summaries) and exit\n",
        "      --turn <N>                Select turn N (zero-based) of --inspect-session, with full detail\n",
        "      --message <id>            Select message <id> of --inspect-session, with full detail\n",
        "  -h, --help                    Print help and exit\n",
        "  -V, --version                 Print version and exit\n"
    )
    .to_string()
}

pub fn version_text() -> String {
    let dirty = crate::build_provenance::build_dirty();
    let suffix = if dirty { " (dirty)" } else { "" };
    format!(
        "liblet {} (build {}){}",
        env!("CARGO_PKG_VERSION"),
        crate::build_provenance::BUILD_SHA,
        suffix
    )
}
