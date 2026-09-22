# MarketWatch Bug Fixes

This branch contains the fixes from the read-only application audit. Each
functional fix is isolated in its own commit so it can be reviewed or
cherry-picked independently.

## Fixed issues

### 1. Missed scan slots during catch-up

If the computer returned after 15:30 IST, the previous implementation selected
only the evening slot. The morning slot could then remain unrecorded forever.
Catch-up now collects all due slots in chronological order and runs each one,
recording its completion after a successful scan.

Commit: `fix(scanner): catch up all missed scan slots`

### 2. Scheduled timer timezone

The generated systemd timer used local wall-clock times while describing them
as IST. On a machine configured for another timezone, scans ran at the wrong
times. The timer now explicitly uses `Timezone=Asia/Kolkata`.

Commit: `fix(service): schedule timers in India timezone`

### 3. Incorrect scan total

`total_scanned` counted only successful Yahoo responses, so provider failures
made a scan appear to cover fewer stocks than were actually attempted. It now
reports the configured universe size; failed requests remain visible through
their warning messages.

Commit: `fix(scanner): report configured universe size`

### 4. Silent cache and database failures

The web API and cached provider ignored database errors during freshness checks,
writes, reads, and fallback handling. These paths now return explicit internal
server errors instead of silently returning stale or incomplete data.
Upstream market-data failures without cached data now return `502 Bad Gateway`
instead of `400 Bad Request`.

Commit: `fix(api): surface cache failures and validate stock names`

### 5. Invalid stock entries

The API rejected empty symbols but accepted empty company names. Empty names
are now rejected with a `400 Bad Request`.

Commit: `fix(api): surface cache failures and validate stock names`

### 6. Misreported systemd installation

Service installation ignored failures from `systemctl`, then printed success
messages. Daemon reload and both unit activation steps now fail the command
with an actionable error when systemd reports failure.

Commit: `fix(service): report systemd activation failures`

### 7. Unsafe generated systemd paths

Executable and working-directory paths containing spaces, quotes, or
backslashes could produce invalid unit files. Generated paths are now quoted
and escaped for systemd.

Commit: `fix(service): quote generated unit paths`

### 8. Frontend API error handling

The dashboard treated non-2xx responses from `/api/stocks` and
`/api/scan/cached` as successful JSON payloads. It now checks response status
and displays the server-provided error.

Commit: `fix(web): handle API errors and escape user content`

### 9. User-supplied HTML injection in the dashboard

Stock symbols and names came from editable configuration and were inserted into
HTML templates without escaping. Dynamic stock text and relevant attributes are
now HTML-escaped before insertion.

Commit: `fix(web): handle API errors and escape user content`

## Validation

- `cargo test --all-targets`
- `cargo check --all-targets`
- Targeted scheduler test for both missed slots
- Web server smoke test and API status checks

The final branch also passes strict Clippy. The cleanup is isolated in
`style: satisfy strict clippy checks` so the functional fixes remain easy to
review independently.
