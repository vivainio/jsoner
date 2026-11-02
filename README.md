# jsoner

A fast, dependency-free, prettier-style formatter for JSON and JSONC, written in Rust.

## Install

```sh
cargo install --git https://github.com/vivainio/jsoner
```

## Usage

```sh
jsoner file.json              # print formatted output
jsoner --write .              # rewrite all *.json / *.jsonc files in place
jsoner --check .              # exit 1 if anything is unformatted (CI)
cat in.json | jsoner          # stdin -> stdout
```

Directories are searched recursively for `*.json` and `*.jsonc`, skipping
`node_modules`, `target` and dot-directories.

### Options

| Option              | Description                  |
| ------------------- | ---------------------------- |
| `-w`, `--write`     | Rewrite files in place       |
| `-c`, `--check`     | Exit 1 if any file differs   |
| `--print-width N`   | Line width (default 80)      |
| `--tab-width N`     | Spaces per indent (default 2)|
| `--use-tabs`        | Indent with tabs             |

Exit codes: `0` ok, `1` unformatted files (`--check`), `2` parse or I/O error.

## Formatting rules

Modeled on prettier's JSON behavior:

- A group stays on one line if it fits in the print width, otherwise it breaks.
- Objects written across multiple lines (newline after `{`) stay expanded, and
  so do their ancestors. Flat objects use brace spacing: `{ "a": 1 }`.
- Arrays of numbers are packed several per line.
- Key order, number literals and string escapes are preserved exactly.
- `//` and `/* */` comments are kept; trailing commas are accepted and removed.

## Not supported

YAML (including CloudFormation YAML), JSON5 syntax, and preserving blank lines
between members.

## License

MIT
