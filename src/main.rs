mod format;
mod parser;

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USAGE: &str = "\
jsoner - a prettier-like JSON formatter

USAGE:
    jsoner [OPTIONS] [PATH...]     (no PATH, or '-': read stdin, write stdout)

OPTIONS:
    -w, --write            Rewrite files in place
    -c, --check            Exit 1 if any file is not formatted
        --print-width N    Line width (default 80)
        --tab-width N      Spaces per indent (default 2)
        --use-tabs         Indent with tabs
    -h, --help             Show this help

Directories are searched recursively for *.json and *.jsonc files
(skipping node_modules, target and dot-directories).";

fn collect(path: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    if path.is_dir() {
        let mut entries: Vec<_> = std::fs::read_dir(path)?.collect::<Result<_, _>>()?;
        entries.sort_by_key(|e| e.file_name());
        for e in entries {
            let name = e.file_name();
            let name = name.to_string_lossy();
            let p = e.path();
            if p.is_dir() {
                if name.starts_with('.') || name == "node_modules" || name == "target" {
                    continue;
                }
                collect(&p, out)?;
            } else if p.extension().is_some_and(|x| x == "json" || x == "jsonc") {
                out.push(p);
            }
        }
    } else {
        out.push(path.to_path_buf());
    }
    Ok(())
}

fn main() -> ExitCode {
    let mut write = false;
    let mut check = false;
    let mut width = 80usize;
    let mut tab = 2usize;
    let mut tabs = false;
    let mut paths: Vec<String> = Vec::new();

    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let mut num = |name: &str| -> Option<usize> {
            let v = args.next().and_then(|v| v.parse().ok());
            if v.is_none() {
                eprintln!("{name} requires a number");
            }
            v
        };
        match a.as_str() {
            "-h" | "--help" => {
                println!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            "-w" | "--write" => write = true,
            "-c" | "--check" => check = true,
            "--use-tabs" => tabs = true,
            "--print-width" => match num(&a) {
                Some(n) => width = n,
                None => return ExitCode::from(2),
            },
            "--tab-width" => match num(&a) {
                Some(n) => tab = n,
                None => return ExitCode::from(2),
            },
            s if s.starts_with('-') && s != "-" => {
                eprintln!("unknown option: {s}\n\n{USAGE}");
                return ExitCode::from(2);
            }
            _ => paths.push(a),
        }
    }

    let opts = format::Options {
        print_width: width,
        indent: if tabs { "\t".into() } else { " ".repeat(tab) },
    };

    let fmt = |src: &str| -> Result<String, parser::ParseError> {
        parser::parse(src).map(|v| format::format(&v, &opts))
    };

    if paths.is_empty() || paths == ["-"] {
        let mut src = String::new();
        if let Err(e) = std::io::stdin().read_to_string(&mut src) {
            eprintln!("stdin: {e}");
            return ExitCode::from(2);
        }
        return match fmt(&src) {
            Ok(out) if check => {
                if out == src {
                    ExitCode::SUCCESS
                } else {
                    eprintln!("stdin is not formatted");
                    ExitCode::FAILURE
                }
            }
            Ok(out) => {
                let _ = std::io::stdout().write_all(out.as_bytes());
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("stdin:{e}");
                ExitCode::from(2)
            }
        };
    }

    let mut files = Vec::new();
    for p in &paths {
        if let Err(e) = collect(Path::new(p), &mut files) {
            eprintln!("{p}: {e}");
            return ExitCode::from(2);
        }
    }

    let (mut unformatted, mut errors) = (0, 0);
    let multi = files.len() > 1;
    for f in &files {
        let src = match std::fs::read_to_string(f) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("{}: {e}", f.display());
                errors += 1;
                continue;
            }
        };
        let out = match fmt(&src) {
            Ok(o) => o,
            Err(e) => {
                eprintln!("{}:{e}", f.display());
                errors += 1;
                continue;
            }
        };
        let changed = out != src;
        if check {
            if changed {
                eprintln!("{}: not formatted", f.display());
                unformatted += 1;
            }
        } else if write {
            if changed {
                if let Err(e) = std::fs::write(f, &out) {
                    eprintln!("{}: {e}", f.display());
                    errors += 1;
                    continue;
                }
                println!("{}", f.display());
            }
        } else {
            if multi {
                println!("// {}", f.display());
            }
            print!("{out}");
        }
    }

    if errors > 0 {
        ExitCode::from(2)
    } else if unformatted > 0 {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
