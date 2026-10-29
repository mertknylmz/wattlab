# WattLab API

Local Rust backend using Axum, Tokio and SQLx/PostgreSQL. The package/binary is
`wattlab-api`; the library is `wattlab_api`. Bluetooth remains in Python.

Implemented: validated local Bearer authentication, status endpoints, session
create/read/finish, embedded SQL migrations, parameterized SQL and graceful shutdown.
Telemetry/connection-event ingestion is implemented. A local browser dashboard is implemented with one-second HTTP polling; SSE is not implemented.

## Structure

```text
src/
├── main.rs / lib.rs        # Composition, startup and shutdown
├── config.rs / config/    # Validated process settings
├── auth.rs                # Independent token type
├── domain/                # Session model, invariants and domain errors
├── application/
│   ├── session_service.rs # Use cases
│   ├── error.rs
│   └── ports/             # SessionRepository and driver-free errors
└── adapters/
    ├── inbound/http/      # Routing, auth, DTOs and HTTP error mapping
    └── outbound/postgres/ # SQLx rows, repository, pool and migrations
```

The single-crate hexagonal design points dependencies inward: HTTP calls the
application service; the service depends on a repository port and domain types.
PostgreSQL implements that port. Domain/application do not import Axum, SQLx,
Serde or Tokio. DTOs, domain models and database rows are explicitly mapped.
Enums/errors live with their owning layer rather than in a shared enum bucket.
Atomic idempotency belongs to the repository contract and is implemented in SQL;
it must not become an application-side read-then-write race.

Tests separate domain invariants, application behavior with fake ports, HTTP
contracts, source dependency boundaries, and opt-in real PostgreSQL behavior.
The architecture source check guards ordinary imports/paths; it is not a
compiler-enforced multi-crate isolation boundary.


## Local setup and RustRover

From the repository root, after installing Python requirements, Rust and PostgreSQL
server/client tools:

```bash
.venv/bin/python scripts/local_db.py start
.venv/bin/python scripts/run_api.py
```

The database helper uses an isolated cluster under ignored .wattlab, loopback port
55432, and generates credentials in ignored root .env. It now provisions separate
`wattlab_rust_dev` and `wattlab_rust_test` databases. Earlier Java databases/data are
preserved. Known helper-generated JDBC settings are updated to PostgreSQL URLs;
custom database settings are not silently overwritten. Existing passwords/tokens
are retained. No system PostgreSQL clusters are altered.

In RustRover open this folder or Cargo.toml, wait for Cargo sync, then Run next to
main in src/main.rs. The server reads root .env relative to Cargo.toml, independent
of the IDE working directory. Process environment overrides file values.
WATTLAB_ENV_FILE can select an explicit file; it must exist and parse correctly.
Deployments should use explicit configuration rather than the build machine path.

Required settings:

- WATTLAB_API_TOKEN: 32–64 ASCII letters/digits/hyphens/underscores.
- WATTLAB_DB_URL: PostgreSQL URL, not a JDBC URL.
- WATTLAB_DB_USER and WATTLAB_DB_PASSWORD: nonempty database credentials.
- WATTLAB_API_PORT: optional, default 8080 (1–65535), loopback only.

Startup connects to PostgreSQL and applies embedded SQLx migrations. It refuses a
database containing the old public Flyway history table before attempting migration;
it never resets a schema. Pool acquisition/connection is bounded, as are statement
and lock waits. /health remains process liveness rather than a per-request database
readiness check. Ctrl+C and Ubuntu SIGTERM request graceful shutdown and pool closure.

## HTTP contract

| Route | Result |
|---|---|
| GET /health | Public process liveness |
| GET /api/v1/status | Authenticated process liveness |
| GET /api/v1/live | Latest session and fresh display metrics |
| GET /api/v1/sessions/{id}/live | A specific session live projection |
| POST /api/v1/sessions | Create or retrieve by request_id |
| GET /api/v1/sessions/{id} | Read a persisted session |
| POST /api/v1/sessions/{id}/finish | Finish idempotently |
| POST /api/v1/sessions/{id}/telemetry | Atomically accept measurement events |
| POST /api/v1/sessions/{id}/connection-events | Atomically accept connection events |

All /api/v1 routes and its fallback require exactly one header
`Authorization: Bearer <token>`. Missing/wrong/duplicate headers return 401 with
WWW-Authenticate: Bearer and a generic JSON error. Token-in-query/cookie is ignored.
Token/config Debug output is redacted; SQLx statement logging is disabled. This is
local shared-token authentication, not a multi-user account/login system.

Create request: `{ "request_id": "<UUID>" }`. Response fields are `id`, `request_id`,
`started_at` (UTC RFC3339) and `finished_at` (null until finished). Successful session
calls return 200. Invalid JSON/UUID returns 400, missing sessions 404, unavailable
storage 503, and unexpected storage failures a sanitized 500. No SQL or credentials
are returned to clients.

A request_id uniquely identifies a creation request. Concurrent retries return the
same session ID/start time. Finish preserves the first finish time, including
concurrent finish requests. Retrying create never reopens a finished session. SQL
statements enforce these transitions atomically; there is no in-memory session state.
The schema also rejects finish-before-start. Ingestion transactions lock
the same session row to serialize against finish.

## Event ingestion contract

Both event endpoints accept `{ "events": [...] }` with 1–100 events and return
HTTP 200 `{ "accepted": N }`. N counts acknowledged input entries, including
identical retries, rather than newly inserted rows. Missing sessions return 404;
invalid input returns 400; altered retries or new data after finish return 409.
The existing Bearer middleware and sanitized 500/503 storage errors apply.

Every event requires integer `schema_version: 1`, UUID `stream_id` and
`connection_id`, integer `sequence` in 0..=9223372036854775807, and RFC3339
`observed_at` with an explicit timezone (UTC year 1–9999). Times are normalized
to UTC and truncated to microseconds for PostgreSQL identity comparisons.
Sequence order and clock monotonicity are not assumed; out-of-order arrival is
permitted. Keys are `(session_id, event_kind, stream_id, sequence)`; the two
kinds use separate tables, matching the existing Python/Java contract.

Measurements require `source` (`ftms` or `cycling_power`) and at least one
non-null metric. Optional/omitted metrics remain null; zero is a measurement.
`speed_kmh` is finite 0–655.35, `cadence_rpm` finite 0–32767.5, `power_w` an
integer -32768–32767. Cycling power only carries power: speed and cadence must
be null/absent. Both sources are retained independently; no merged display-power
selection is implemented yet.

Connection events require `state`: `connecting`, `subscribing`, `streaming`,
`retry_wait` or `stopped`. They record observations; they do not change the
backend session lifecycle or control the trainer.

A session row lock covers checking, inserting and committing the whole batch,
serializing ingestion against finish and other batches. An existing key with
identical normalized fields is acknowledged even after finish. Changed contents
conflict. New keys require an active session. Any validation/conflict/storage
failure rolls back the entire batch, including earlier new entries. Repeating a
key within a batch follows the same rule. Validation lives in domain types;
HTTP DTOs map into validated batches, and the PostgreSQL adapter implements the
atomic repository port. Migration 2 adds separate event tables with foreign keys,
unique keys and defensive value constraints; migration 1 is unchanged.

## Verification

From the repository root:

```bash
.venv/bin/python scripts/run_api.py --test --offline
.venv/bin/python scripts/run_api.py --integration --offline
# With the API running:
.venv/bin/python scripts/smoke_sessions.py
.venv/bin/python scripts/smoke_telemetry.py
```

From this directory:

```bash
cargo test --locked
cargo test --locked --test postgres -- --ignored
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
# Optional frontend behavior checks, Node.js 18+ (not needed to run the app):
node tests/web.cjs
```

37 unit/application/architecture/HTTP tests pass without PostgreSQL. Seventeen PostgreSQL tests are explicitly
ignored by default; the integration command runs them using WATTLAB_TEST_DB_URL,
WATTLAB_TEST_DB_USER and WATTLAB_TEST_DB_PASSWORD. Test database names must end in
`_rust_test`; tests never fall back to development settings. Integration tests create
UUID-scoped fixture rows without truncating or resetting the database.

All seventeen integration tests passed, covering session/event persistence,
concurrent replay/conflict/finish races, batch rollback, post-finish retries,
404 behavior, and defensive database constraints. Real loopback HTTP create/read/finish and SIGTERM shutdown
also passed. The unchanged Python sender delivered two measurements and three
connection events to the local Rust API, then finished its session. Legacy Flyway startup refusal was exercised; earlier Java databases'
row counts and migration history remained unchanged. No trainer was contacted.

## Local dashboard

Start the database/API as above, then open http://127.0.0.1:8080/ (or your configured
API port). Rust embeds the HTML/CSS/JavaScript in its binary: no Node, frontend
build step, CDN, external fonts or internet connection is needed at runtime.
Source assets are in `web/`; restart/rebuild the API after editing them.

Enter the local `WATTLAB_API_TOKEN` from ignored root `.env` into the masked access
field. It is sent only as an Authorization header to the same origin; it is not
embedded in HTML, placed in URLs, persisted in browser storage, or logged. Reload
or disconnect clears it. The public page/assets contain no private data; all live
data endpoints retain Bearer authentication. CSP restricts scripts/styles/requests
to the local origin and responses disable caching.

The dashboard shows power, cadence, speed, session duration/state, and a graph of
power samples observed by this tab over the last 60 seconds. Every successful
HTTP reply is captured immediately, including a zero just before session finish;
the render timer does not manufacture additional measurements. It automatically
follows the most recently started session. No session yet is an explicit empty
state. A finished session stays visible as the latest summary with live values
hidden. There is no trainer/session start/stop control; the existing Python sender
still owns creating and finishing its session. “Ekran bağlantısını kes” disconnects
only the viewing tab.

Run the Python listener separately when you want actual trainer readings:

```bash
# Only when explicitly ready for trainer access; close other trainer clients.
.venv/bin/python experiments/bluetooth/live_trainer.py
```

Its `WATTLAB_API_URL` must point at this API and its token must match. Without a
running sender the dashboard correctly shows waiting/stale/completed state. For
an explicitly synthetic, hardware-free check of a running API use
`scripts/smoke_telemetry.py`; it finishes quickly, so the live values may already
be cleared when you look. There is no permanent demo/random-data mode in the UI.

### Live projection and freshness

The API returns `session`, `connection_state`, `fresh`, `speed_kmh`, `cadence_rpm`,
`power_w`, `power_source`, per-source receipt times and `captured_at`. Session and
metrics are read using one PostgreSQL statement/MVCC snapshot. A specific missing
session returns 404; the automatic view with no sessions returns 200 with null
session/metrics and fresh=false. SQL/model details stay in the outbound adapter;
domain code applies display freshness and source preference.

Migration 3 adds server-side receipt times and read indexes. Existing rows retain
null receipt times rather than appearing newly live during migration. Exact
replays never change receipt time. The current stream is the one whose first
connection event was received most recently; within that stream, sequence chooses
the latest connection event and per-source measurement. Only that connection's
measurements can be shown. Late packets from an older connection/stream do not
restore its values, and lower-sequence arrival does not replace a newer sample.
This is a single-user, latest-session view; there is no multi-trainer selector.

Metrics require an active session, last connection state streaming, and receipt
age between zero and five seconds. This is a display freshness threshold, not a
BLE disconnect detector. Fresh Cycling Power is preferred for power; fresh FTMS
is the fallback. Speed/cadence use the latest FTMS packet. Null fields remain
missing; older packets are not carried forward to fill them. Both raw sources
remain independently stored. Stale fields display an em dash; a real zero stays
zero. The browser also ages each source locally between replies, clears values
on API failure, aborts requests after three seconds, and retries without overlap.
Wrong auth returns the user to the access form rather than retrying indefinitely.
The UI cannot distinguish a stopped producer from a quiet radio solely by receipt
age; waiting indicates that limitation rather than asserting Bluetooth is connected.

### Verification and continuation

37 Rust unit/application/architecture/HTTP tests, 17 PostgreSQL tests and twelve
Node frontend behavior tests pass. Tests cover source preference, missing/zero,
stale/future receipts, concurrent lifecycle, delayed packets, stream/connection
isolation, auth, API outages and late replies. Actual browser checks at desktop
and narrow widths verified authentication, changing synthetic values, graph
rendering, and completion/clearing. No trainer was accessed during UI development.
A user-authorized real D500 → Python → Rust → PostgreSQL → browser check also
passed: changing metrics and the graph were visible, 309 measurements/four
connection events were persisted, all 313 events were delivered without drops,
and completion cleared live values. No control commands were sent; calibration
and physical reconnection were not tested in this run.

The browser/trainer integration is now functionally verified. SSE, durable client buffering, trainer control commands, desktop/mobile
packaging and cloud sync remain out of scope. The previous Java source remains
only in ignored .wattlab/archives/java-api-before-rust.

### Coasting to zero versus an interrupted stream

Actual 0 W samples are drawn at zero and remain connected to earlier samples.
Missing/stale data, API errors, stopped listening and session completion leave a
gap or end the trace; they never imply measured zero. A caption identifies these
states. In the first real dashboard test, the last 12 recorded samples were still
67–82 W when the test shut down; there was no final measured zero to draw. A later
real coast-down check should keep listening after pedaling stops until zero is
actually observed. One-second polling can miss a very short transient entirely
between HTTP replies; exact event history/replay is not implemented.

### FTP and power-zone colors

The user explicitly selected a default display FTP of 200 W. The dashboard can
override it with an integer 1–2000 W for this tab, or reset to 200 W. Refresh resets
the setting; user profiles/persistent FTP are still planned. The default is labeled
as a reference, not a measured personal FTP. It does not change trainer resistance.

Power-card values and graph segments use Z1–Z6 gray/blue/green/yellow/orange/red.
Zero is neutral Z0; missing, stale and negative readings are not assigned a zone.
Explicit percentage intervals are (0,60), [60,76), [76,90), [90,105), [105,119),
and [119,infinity), inspired by [Zwift's zone colors](https://support.zwift.com/es/using-the-heads-up-display-when-cycling-rJepA4beS).
Changing the display FTP recolors the visible history without modifying watts;
missing-data gaps remain gaps. The chart uses each segment's ending sample color.
Text labels/percentages supplement color.

### One-command local startup

See [the local run guide](../docs/LOCAL_RUN.md). From the repository root:

```bash
.venv/bin/python scripts/run_local.py --offline
# Explicitly enable read-only trainer access:
.venv/bin/python scripts/run_local.py --offline --trainer
```

The default starts only the dedicated database, API and browser. Startup waits for
authenticated API readiness, refuses an occupied port/another launcher, and never
kills another API process. Ctrl+C/SIGTERM stops owned children in reverse order so
the sender can drain/finish before the API closes (up to 30 seconds per child).
PostgreSQL remains running. `--no-browser` prints the URL instead of opening it.
Tests: `python -m unittest discover -s scripts/tests -v` covers owned process cleanup,
forced shutdown of a stuck child, readiness and the default setup boundary.
The launcher was exercised with actual local DB/API startup and clean Ctrl+C
shutdown, without BLE. The explicit trainer option still needs a hardware run.
