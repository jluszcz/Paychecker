# Paychecker

A terminal application for recording paychecks and seeing where each one goes: per-paycheck and
year-to-date amounts, net pay, and each line as a percentage of income. It replaces a personal
spreadsheet.

The design lives in [`docs/superpowers/specs/2026-09-29-paychecker-design.md`](docs/superpowers/specs/2026-09-29-paychecker-design.md).

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
