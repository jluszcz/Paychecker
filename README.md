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
pc backup               # upload the database to S3 if a backup is due
```

`1` shows the Sheet (a year's paychecks, YTD, net, and percentages) and `2` the Fields list.
`a` adds, `e` edits, and `d` deletes on either screen; `?` lists every key.

## Report

Off until a config file switches it on:

```toml
# ~/.config/paychecker/config.toml
[report]
dir = "~/Dropbox/pay"   # required; absolute, or under ~
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

## Backups

`pc` uploads a copy of the database to S3 when the last upload is older than `interval_days`. The
check runs after the screen is torn down and after the report is written, so a slow network only
holds up the prompt, and a failure prints to stderr without failing the run. A run given `--db`
skips it: the state file records when a backup last ran, not which file, so a scratch copy would
take the real database's turn. `--today` does not skip it, since the schedule reads the real clock.
`pc report --dir` skips it too, since that run does not read the config file.

Off until a config file switches it on:

```toml
# ~/.config/paychecker/config.toml
[backup]
bucket        = "..."          # required
profile       = "paychecker"   # default; a profile in ~/.aws/credentials
interval_days = 7              # default
```

A backup is `paychecks-<timestamp>.db.zst`, a zstd-compressed copy of the database, at the root of a bucket that holds nothing else.

One-time setup: `terraform apply` creates the bucket, the IAM user and its access key, and its
outputs feed the `paychecker` profile and the config file:

```bash
aws configure set aws_access_key_id "$(terraform output -raw paychecker_access_key_id)" --profile paychecker
aws configure set aws_secret_access_key "$(terraform output -raw paychecker_access_key_secret)" --profile paychecker
aws configure set region us-east-2 --profile paychecker
terraform output -raw backup_bucket   # goes in config.toml's `bucket`
```

The region line is required: it is read from the profile, and without it every upload fails. The
profile must carry static access keys; SSO and `credential_process` profiles will not authenticate.

The bucket is `paychecker-<account id>-<region>-an`, composed rather than chosen so its name says
nothing to anyone without the profile. Public access is blocked, and objects move to
Standard-Infrequent Access at 30 days and expire at 365. The `paychecker` profile can only
`PutObject`, and only as a conditional write that refuses to replace an existing object: it cannot
read, overwrite, delete, or list backups.

`pc backup --status` prints the last upload and the next due date; `pc backup --force` uploads
regardless of the schedule. `pc backup` uploads whatever database it is given, `--db` included.

To restore, quit `pc`, then under your own AWS identity (`zstd` must be installed):

```bash
aws s3 ls s3://<bucket>/
rm -f ~/.local/share/paychecker/paychecks.db-wal ~/.local/share/paychecker/paychecks.db-shm
aws s3 cp s3://<bucket>/paychecks-20260820T140305Z.db.zst .
zstd -d -f paychecks-20260820T140305Z.db.zst -o ~/.local/share/paychecker/paychecks.db
```

The `-wal` file goes first because SQLite would replay a leftover one into the restored database.

## Development

`pre-commit install` wires up the hooks in `.pre-commit-config.yaml`, which run
`cargo fmt --check` and the Terraform `fmt`/`validate` hooks (these need `terraform` on `PATH`)
alongside the usual whitespace and YAML/TOML checks. GitHub Actions builds, tests, and lints every
pull request, and checks the Terraform.

## No real data in the repository

This repository is public and the owner's pay is not. Nothing committed here carries a real
amount, employer, or name, and test fixtures use invented figures. `AGENTS.md` states the rule in
full.

## License

[MIT](LICENSE)
