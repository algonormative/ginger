//! `ginger` — the command-line entry point. v0 scaffold: only `--version` and `--help` exist.
//! The application service that backs both the CLI and the MCP server lands in wave 1
//! (see docs/design/ginger-v0-technical-design.md, section 2).

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--version") | Some("-V") => println!("ginger {}", env!("CARGO_PKG_VERSION")),
        _ => {
            println!("ginger {} — scaffold", env!("CARGO_PKG_VERSION"));
            println!("usage: ginger --version");
            println!("v0 commands (planned): validate run stop status prepare apply cancel cue checkpoint capture probe tree trace render mcp");
        }
    }
}
