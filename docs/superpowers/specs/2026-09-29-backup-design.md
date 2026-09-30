# Backups to S3 — design

## Goal

Paychecker's database cannot be regenerated, so it is copied to S3 on a schedule, the way
MisterManager's is. This is a port of MisterManager's `src/backup/`, `mistermanager.tf`, and the
wiring around them, renamed for this application and trimmed to what Paychecker needs.

Success: with a `[backup]` section configured and the `paychecker` profile holding the Terraform
key, quitting `pc` uploads `paychecks-<timestamp>.db` to the application's own bucket at most once
per `interval_days`; `pc backup --status` says when; nothing happens on an unconfigured machine.

## Configuration

```toml
# ~/.config/paychecker/config.toml
[backup]
bucket        = "..."          # required, no default
profile       = "paychecker"   # default
interval_days = 7              # default
```

`config::Config` gains `backup: Option<Backup>`. The rules the `[report]` section already follows
hold: an absent section is off, a file that does not parse is an error, `bucket` has no default so
a misspelling of it is an error rather than a silently disabled backup, and keys nothing reads are
ignored (so a `prefix` line does nothing).

## Modules

### `src/backup/mod.rs` — the schedule and the run

- `is_due(last: Option<DateTime<Utc>>, now, interval_days) -> bool` and `next_due(last,
  interval_days)`. `interval` clamps `interval_days` to `MAX_INTERVAL_DAYS = 3653` before
  `TimeDelta::days`, since chrono's addition panics outside its calendar. A `last` in the future is
  not due; an interval of 0 is always due.
- `key_for(now) -> String`: `paychecks-%Y%m%dT%H%M%SZ.db`, at the bucket root, no prefix.
- `create_snapshot_dir(dir)`: on unix, removes our own leftover and creates the leaf
  non-recursively with mode 0700, so a squatted `/tmp/paychecker-backup-<pid>` cannot receive the
  snapshot.
- `Outcome { Disabled, NotDue { next }, BackedUp { key, bytes } }`.
- `run_if_due(db_path, cfg, state_path, now, force) -> Result<Outcome>`: returns `Disabled` with no
  `[backup]`; reads state (an unreadable file warns and counts as never backed up); returns
  `NotDue` before touching the database; otherwise snapshots into the temp dir, measures, uploads,
  removes the temp dir on both paths, and writes state only after a successful upload.
- Byte counts print through the existing `report::human_bytes` rather than a second copy.

### `src/backup/s3.rs` — the upload

`upload(profile, bucket, key, file)`: a current-thread tokio runtime built for the one call and
dropped; `aws_config` loaded for `profile`; a missing region is an error that names the profile and
the `aws configure set region` fix; one `PutObject` with content type `application/vnd.sqlite3`,
no SSE header, errors rendered through `DisplayErrorContext`. The only file naming `aws_config`,
`aws_sdk_s3`, `aws_smithy_types` or `tokio`.

### `src/backup/state.rs` — when the last upload succeeded

`State { last_backup_at: DateTime<Utc>, last_key: String }` in
`$XDG_STATE_HOME/paychecker/backup.toml` (`~/.local/state` when unset or empty). `read` returns
`Ok(None)` for a missing file and `Err` for an unparseable one; `write` creates its directory. This
is the second module naming `serde`/`toml`, a deliberate exception AGENTS.md records.

### `src/db/` — the snapshot

`db::snapshot(src, dest)`: opens `src` and runs `VACUUM INTO ?1`, so `rusqlite` stays inside
`src/db/`.

## CLI

`pc backup [--force | --status]` (`--status` conflicts with `--force`):

- `--status` prints the last upload's time and key and the next due date, or that none has run, or
  that backups are off.
- Otherwise `run_if_due(.., Utc::now(), force)`; a failure is an error exit, since it was asked for.
  Prints `backed up 2 KiB to s3://<bucket>/<key>`, `not due until <date>`, or that backups are off.

Every other arm — the TUI and `pc report` — then runs the scheduled check, which never fails the
run (it prints `backup failed: …` to stderr) and prints a line only when it uploads. It runs only
against the default database: `--db` skips it, because the one state file records when rather than
what, so a scratch upload would take the real database's turn. `--today` does not skip it: the due
check reads the real clock, and the file uploaded is still the real database.

## Infrastructure — `paychecker.tf`

A copy of `mistermanager.tf` with names changed:

- State backend: bucket `jluszcz-tf-state`, key `paychecker`, region `us-east-2`, `use_lockfile`.
- Bucket `paychecker-<account id>-<region>-an` (`account-regional` namespace), public access blocked
  four ways, SSE-KMS under the AWS-managed `aws/s3` key with bucket keys, SSE-C blocked.
- Lifecycle: Standard-IA at 30 days, expire at 365, noncurrent versions at 30, abort multipart at 7.
- IAM user `paychecker` with an access key and a policy allowing only `s3:PutObject` on
  `<bucket arn>/*`.
- Outputs: `backup_bucket`, `paychecker_access_key_id`, `paychecker_access_key_secret` (sensitive).

Also: the `terraform` job in `.github/workflows/ci.yml` (with `**/*.tf` and `.terraform.lock.hcl`
in the push paths), the Terraform `fmt`/`validate` pre-commit hooks, `.terraform/` in
`.gitignore`, and the generated `.terraform.lock.hcl` committed.

## Dependencies

`aws-config`, `aws-sdk-s3` and `aws-smithy-types` with MisterManager's feature selection (defaults
off, `behavior-version-latest`, `rt-tokio`, `rustls` on the S3 client), `tokio` with `rt`, `net`,
`time`, and chrono's `serde` feature.

## Documentation

- README: a "Backups" section — what runs and when, the `--db` opt-out, the config block, the
  one-time `terraform apply` and `aws configure` setup (region line required), the bucket naming
  and lifecycle, static-key requirement, `--status`/`--force`, and restore (quit, remove `-wal`/`-shm`,
  `aws s3 cp` over the database under the owner's own identity).
- AGENTS.md: the module's place in the layout, the `aws_*`/`tokio` naming rule, the `serde`/`toml`
  exception, and the invariants: PutObject-only identity, composed bucket name, no prefix, real
  clock, default database only, advisory state file, clamped interval.

## Testing

Unit tests in `mod tests` at the bottom of each file, full-sentence names, nothing touching S3:

- `backup`: due / not due / exactly due / future `last` / zero interval / `next_due` / clamp; key
  format; snapshot dir mode 0700 and a squatted dir replaced; `Disabled` with no section; `NotDue`
  against a nonexistent database path.
- `state`: round trip, creates its directory, missing is `None`, garbage is `Err`.
- `config`: full `[backup]` parses, bucket-only takes defaults, misspelled `bucket` is an error, a
  `prefix` key is ignored, a `[report]`-only file leaves backups off.
- `db::snapshot`: a snapshot of a file database opens and holds the same paychecks.

## Outside this repository

The dotfiles already template `[backup] bucket` for Paychecker; they gain a `[profile paychecker]`
AWS profile and skip Paychecker's files on non-macOS hosts. Access keys stay out of the dotfiles.
