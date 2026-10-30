# WattLab API

Local Rust backend using Axum, Tokio and SQLx/PostgreSQL. The package/binary is
`wattlab-api`; the library is `wattlab_api`. Bluetooth remains in Python.

Implemented: validated local Bearer authentication, status endpoints, session
create/read/finish, embedded SQL migrations, parameterized SQL and graceful shutdown.
Telemetry/connection-event ingestion is implemented. Session history/summary/series,
persistent athlete settings (FTP), an in-memory trainer-agent command queue and an
SSE live stream are implemented. The API serves no pages: the screens
(`../wattlab-ui`) run only inside the desktop app (`../wattlab-desktop`), whose main
process forwards their `/api` requests here with the per-launch Bearer token. A
future mobile app will use the same HTTP/SSE contract.

## Structure

```text
src/
├── main.rs / lib.rs        # Composition, startup and shutdown
├── config.rs / config/    # Validated process settings
├── auth.rs                # Independent token type
├── domain/                # Session, events, live view, history summary,
│                          # FTP/zones, trainer commands and agent status
├── application/
│   ├── *_service.rs       # Session, event, history, settings, command use cases
│   ├── error.rs
│   └── ports/             # Repository/queue ports and driver-free errors
└── adapters/
    ├── inbound/http/      # Routing, auth, DTOs, SSE, shutdown, error mapping
    └── outbound/
        ├── postgres/      # SQLx rows, repositories, pool and migrations
        └── memory/        # In-process trainer command queue
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
| GET /api/v1/live/stream | SSE: `snapshot` event `{live, agent}` every second; `unavailable` on storage failure |
| GET /api/v1/sessions?limit=&before= | Newest-first summaries; `limit` 1–100 (default 20), `before` is the previous page's `next_before` |
| GET /api/v1/sessions/{id}/summary | Session, captured `ftp_w`, `zone_reference`, summary |
| GET /api/v1/sessions/{id}/series | Summary plus per-second points `{t, power_w, power_source, cadence_rpm, speed_kmh}` |
| GET, PUT /api/v1/settings | FTP `{ftp_w, ftp_is_default, updated_at}`; PUT `{"ftp_w": 1–2000 or null}` (null resets to 200 W) |
| POST /api/v1/agent/commands | Queue `{kind, target_power_w?}`; 202 with the command |
| GET /api/v1/agent/commands/{id} | Command status: pending, delivered, succeeded, rejected, expired |
| GET /api/v1/agent/commands/next?wait_seconds= | Agent long poll (≤20 s); 204 when idle or shutting down |
| PUT /api/v1/agent/commands/{id}/result | Agent reports `{status: succeeded/rejected, message}` |
| GET, PUT /api/v1/agent/status | Agent heartbeat; GET reports `online` within 3 s of the last heartbeat |

Command kinds: `connect`, `disconnect`, `start_session`, `finish_session`,
`set_target_power` (0–2000 W) and `release_control`. A command is delivered once;
undelivered commands expire after 10 s, delivered ones after 45 s without a result.
The queue is in memory: restarting the API forgets pending commands, which is
intended (stale hardware commands are never replayed). The API never talks to the
trainer; the Python agent executes commands and reports their outcome.

History buckets measurements per second (per-source averages; Cycling Power
preferred for power). Averages exclude missing values and include zeros; work
(kJ) counts only positive power. Migration 4 adds the single-row
`athlete_settings` table (default 200 W) and `training_session.ftp_w`, captured
when a session starts. Older sessions keep null and are summarized with the
current FTP, reported as `zone_reference: "current"`. Graceful shutdown ends long
polls and SSE streams first so the server can stop promptly.

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
# Screen behavior tests live with the UI source (Node.js 18.18+, build-time only):
(cd ../wattlab-ui && npm test)
```

60 unit/application/architecture/HTTP tests pass without PostgreSQL. Twenty PostgreSQL tests are explicitly
ignored by default; the integration command runs them using WATTLAB_TEST_DB_URL,
WATTLAB_TEST_DB_USER and WATTLAB_TEST_DB_PASSWORD. Test database names must end in
`_rust_test`; tests never fall back to development settings. Integration tests create
UUID-scoped fixture rows without truncating or resetting the database.

All twenty integration tests passed, covering history bucketing/paging, FTP
persistence and capture at session start, session/event persistence,
concurrent replay/conflict/finish races, batch rollback, post-finish retries,
404 behavior, and defensive database constraints. Real loopback HTTP create/read/finish and SIGTERM shutdown
also passed. The unchanged Python sender delivered two measurements and three
connection events to the local Rust API, then finished its session. Legacy Flyway startup refusal was exercised; earlier Java databases'
row counts and migration history remained unchanged. No trainer was contacted.

## Desktop screens

The API has no browser UI: `/`, `/index.html` and asset paths return 404. The
Electron app serves the built `../wattlab-ui` files from its private
`wattlab://app/` scheme and forwards `/api/*` from that origin to this API, adding
`Authorization: Bearer <per-launch token>` in the main process. The screens never
hold the token; their CSP allows connections only to their own origin. No Node,
CDN, external fonts or internet connection is needed at runtime.

The live screen shows power, cadence, speed, session duration/state, and a graph of
power samples observed by this window over the last 60 seconds. It reads the SSE
stream through `fetch`, samples every snapshot
immediately, including a zero just before session finish, and reconnects after
one second if the stream ends or is silent for five seconds. The render timer
does not manufacture measurements. It follows the most recently started session.
History, session detail and settings screens use the endpoints above.

The trainer panel sends commands to the Python trainer agent
(`experiments/bluetooth/trainer_agent.py`) and shows the agent's answer; it never
assumes the trainer complied. Without a running agent the panel says so and its
buttons stay disabled. For a synthetic, hardware-free check of a running API use
`scripts/smoke_telemetry.py`. There is no permanent demo/random-data mode.

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
zero. The screen also ages each source locally between snapshots, clears values
when the stream fails, and reconnects without overlapping streams.
A refused request (401) shows a restart message rather than retrying indefinitely.
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

That real-device run used the earlier polling screen and `live_trainer.py`. The
React/SSE screens, history/settings and the command-driven trainer agent were
verified with synthetic data only (30 October 2026): agent online, start/finish
through commands, reasoned rejections (no active session, ERG unavailable), FTP
captured at start, history listing, and ordered agent→API shutdown. The new agent,
connecting from the screen and ERG commands still need an authorized device run.
Durable client buffering, mobile and cloud sync remain out of scope. The previous
Java source remains only in ignored .wattlab/archives/java-api-before-rust.

### Coasting to zero versus an interrupted stream

Actual 0 W samples are drawn at zero and remain connected to earlier samples.
Missing/stale data, API errors, stopped listening and session completion leave a
gap or end the trace; they never imply measured zero. A caption identifies these
states. In the first real dashboard test, the last 12 recorded samples were still
67–82 W when the test shut down; there was no final measured zero to draw. A later
real coast-down check should keep listening after pedaling stops until zero is
actually observed. One-second snapshots can miss a very short transient between
them; the session detail chart shows the stored per-second history instead.

### FTP and power-zone colors

The default FTP is 200 W, labeled as a reference rather than a measured personal
FTP. The settings screen persists an integer 1–2000 W (or resets to 200 W) through
`/api/v1/settings`; there is a single local athlete, no multi-user profiles. FTP
does not change trainer resistance. Sessions capture FTP at start for their zone
distribution; the live screen uses the current FTP.

Power-card values and graph segments use Z1–Z6 gray/blue/green/yellow/orange/red.
Zero is neutral Z0; missing, stale and negative readings are not assigned a zone.
Explicit percentage intervals are (0,60), [60,76), [76,90), [90,105), [105,119),
and [119,infinity), inspired by [Zwift's zone colors](https://support.zwift.com/es/using-the-heads-up-display-when-cycling-rJepA4beS).
Changing the display FTP recolors the visible history without modifying watts;
missing-data gaps remain gaps. The chart uses each segment's ending sample color.
Text labels/percentages supplement color.

### Headless developer startup

Normal use is the desktop app; see [the local run guide](../docs/LOCAL_RUN.md).
Without screens, from the repository root:

```bash
.venv/bin/python scripts/run_local.py --offline
# Also start the trainer agent (it connects only when the screen asks):
.venv/bin/python scripts/run_local.py --offline --trainer
```

The default starts only the dedicated database and API. The Electron
shell in `../wattlab-desktop` manages the same processes with a per-launch token. Startup waits for
authenticated API readiness, refuses an occupied port/another launcher, and never
kills another API process. Ctrl+C/SIGTERM stops owned children in reverse order so
the sender can drain/finish before the API closes (up to 30 seconds per child).
PostgreSQL remains running.
Tests: `python -m unittest discover -s scripts/tests -v` covers owned process cleanup,
forced shutdown of a stuck child, readiness and the default setup boundary.
The launcher was exercised with actual local DB/API startup and clean Ctrl+C
shutdown, without BLE. The trainer agent option still needs a hardware run.
