# WattLab API

Local Rust backend using Axum, Tokio and SQLx/PostgreSQL. The package/binary is
`wattlab-api`; the library is `wattlab_api`. Bluetooth remains in Python.

Implemented: validated local Bearer authentication, status endpoints, session
create/read/finish, embedded SQL migrations, parameterized SQL and graceful shutdown.
Telemetry/connection-event ingestion is implemented. Live streaming and a frontend are pending.

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
```

31 unit/application/architecture/HTTP tests pass without PostgreSQL. Fourteen PostgreSQL tests are explicitly
ignored by default; the integration command runs them using WATTLAB_TEST_DB_URL,
WATTLAB_TEST_DB_USER and WATTLAB_TEST_DB_PASSWORD. Test database names must end in
`_rust_test`; tests never fall back to development settings. Integration tests create
UUID-scoped fixture rows without truncating or resetting the database.

All fourteen integration tests passed, covering session/event persistence,
concurrent replay/conflict/finish races, batch rollback, post-finish retries,
404 behavior, and defensive database constraints. Real loopback HTTP create/read/finish and SIGTERM shutdown
also passed. The unchanged Python sender delivered two measurements and three
connection events to the local Rust API, then finished its session. Legacy Flyway startup refusal was exercised; earlier Java databases'
row counts and migration history remained unchanged. No trainer was contacted.

## Next step

A user-authorized real D500 → Python → Rust → PostgreSQL check also passed:
274 measurements and four connection events were persisted, all 278 events were
acknowledged without sender drops, and session finish/controlled shutdown succeeded.
Observed values changed while pedaling; measurement calibration and physical
reconnection were not tested in this run. No trainer control commands were sent.

Next add a small local live view. SSE, metric freshness and display-power source
selection still need design/implementation. Durable client buffering, trainer
control commands and cloud sync are not implemented.
The old Java sources remain only in ignored .wattlab/archives/java-api-before-rust.
