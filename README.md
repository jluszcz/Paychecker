# Paychecker

A terminal application for recording paychecks and seeing where each one goes: per-paycheck and
year-to-date amounts, net pay, and each line as a percentage of income. It replaces a personal
spreadsheet.

## Usage

```bash
cargo install --path .
pc                      # opens ~/.local/share/paychecker/paychecks.db
pc --db /tmp/demo.db    # a scratch database
pc report               # write the HTML report without opening the application
```

`1` shows the Sheet (a year's paychecks, YTD, net, and percentages) and `2` the Fields list.
`a` adds, `e` edits, and `d` deletes on either screen; `?` lists every key.

## Report

Off until a config file switches it on:

```toml
# ~/.config/paychecker/config.toml
[report]
dir = "~/Dropbox/pay"   # required
```

When `pc` quits, it writes a self-contained HTML page of the Sheet to `<dir>/Paychecks.html`,
one tab per year. The page opens on the current year, or the newest year that has paychecks.
Paychecks run newest first, the reverse of the Sheet screen, and YTD stays pinned to the right
edge while they scroll. On a phone the grid tightens, shows whole dollars (the cents are cut, not
rounded), and snaps each swipe to a whole paycheck column.
It carries no script and loads nothing, so it reads on a phone offline. Pointing `dir` at a
synced folder puts it there.

The write happens after the screen is torn down, and prints `wrote 2 KiB to <dir>/Paychecks.html`.
A quit that writes nothing prints nothing. A failure prints to stderr and does not fail the run. A run given `--db` or `--today` writes nothing on quit: it is a scratch session, and the
configured page belongs to the real database. The file is written beside its name and renamed into place, so a sync client never
uploads half a page.

A quit that changed nothing leaves the page alone if it was already written that day. That check
sees only this run's writes and the file's timestamp, so a session held open across midnight can
leave a stale page until the next quit that changes something.

```bash
pc report                    # into the configured dir, never skipped
pc report --dir /tmp/export  # anywhere, [report] section or not
```

## Development

`pre-commit install` wires up the hooks in `.pre-commit-config.yaml`, which run
`cargo fmt --check` alongside the usual whitespace and YAML/TOML checks. GitHub Actions builds,
tests, and lints every pull request.

## No real data in the repository

This repository is public and the owner's pay is not. Nothing committed here carries a real
amount, employer, or name, and test fixtures use invented figures. `AGENTS.md` states the rule in
full.

## License

[MIT](LICENSE)
