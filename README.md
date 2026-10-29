# WattLab

Ubuntu + Van Rysel D500 BLE exploration. Bluetooth addresses are local configuration, not committed.

```bash
python3 -m venv .venv
source .venv/bin/activate
pip install -r requirements.txt
cp .env.example .env
# Edit .env with your trainer's Bluetooth address
python experiments/bluetooth/scan_trainer.py
python experiments/bluetooth/live_trainer.py
```

Close Zwift and disconnect other trainer clients before running. `scan_trainer.py` connects to the configured address and lists GATT services; it does not scan nearby devices.

`live_trainer.py` prints parsed speed (km/h), cadence (RPM), and power (W). It uses whichever notifying FTMS Indoor Bike Data or Cycling Power characteristics are available; at least one is required. Missing measurements remain `None`. If a selected subscription fails, the whole connection attempt is cleaned up and retried rather than silently dropping that source.

The Python connection manager detects disconnects and permits up to eight retries, waiting 1, 2, 4, 8, 16, then at most 30 seconds between attempts. The budget resets after a connection has remained in streaming state for 30 seconds; this does not imply that fresh telemetry arrived. No automatic reconnect is triggered solely by missing packets. Connection/subscription operations have a 20-second timeout. These settings can be changed through `ConnectionSettings` in Python; no new environment variables are required.

Ctrl+C or SIGTERM requests shutdown during connection, streaming, or retry waits. Successfully opened subscriptions are cleaned up independently, then disconnect is attempted, with a three-second timeout per cleanup operation. Cleanup failures are logged without device addresses; an unsuccessful disconnect disables automatic reconnect to avoid overlapping connections. Cleanup is best effort if the OS or adapter fails. No trainer control commands are sent.

## Hardware-free tests

The lifecycle and parser tests use only Python's standard library and fake clients:

```bash
.venv/bin/python -m unittest discover -s experiments/bluetooth/tests -v
```

These tests cover disconnect/reconnect, stale callbacks, capabilities, partial subscriptions, retry limits/backoff, timeouts, cancellation, and resource cleanup. Real Ubuntu/BlueZ and D500 behavior requires a separate device test; these tests do not establish hardware compatibility.


## WattLab API

The active backend project is Rust/Axum in [wattlab-api](wattlab-api/README.md). Open wattlab-api/Cargo.toml in RustRover. Application naming is language-neutral.

Current Rust implementation provides public process health, protected API status, validated local Bearer authentication, persisted session create/read/finish using SQLx/PostgreSQL, configuration and graceful shutdown. Atomic telemetry and connection-event ingestion is implemented; the existing Python sender was verified against Rust using synthetic events. A local browser dashboard now shows live metrics, connection/session state and a recent power graph using one-second polling. SSE is not implemented. The previous Java source is preserved only in the ignored local archive .wattlab/archives/java-api-before-rust. Existing database data is unchanged.

From the repository root:

```bash
.venv/bin/python scripts/local_db.py start
.venv/bin/python scripts/run_api.py
.venv/bin/python scripts/run_api.py --test --offline
.venv/bin/python scripts/run_api.py --integration --offline
```

Or from wattlab-api:

```bash
cargo run --locked
cargo test --locked
```

GET http://127.0.0.1:8080/health returns process liveness. WATTLAB_API_PORT changes the port. The application reads the root .env for local development; WATTLAB_ENV_FILE can select a different file. WATTLAB_API_TOKEN is required at startup (32–64 URL-safe characters); the existing local .env contains the generated value. GET /api/v1/status requires Authorization: Bearer <token>. Startup requires PostgreSQL and applies SQLx migrations. Status requests do not themselves query the database or contact a trainer.

## Local PostgreSQL

The existing development cluster is managed by the helper below; it does not alter system clusters. It uses loopback port 55432, stores data under ignored .wattlab, and keeps generated credentials in ignored .env. These helpers require installed PostgreSQL binaries and Python dependencies.

```bash
.venv/bin/python scripts/local_db.py start
.venv/bin/python scripts/local_db.py status
.venv/bin/python scripts/local_db.py stop
```

The helper provisions separate wattlab_rust_dev and wattlab_rust_test databases for Rust and preserves earlier Java/Flyway data. PostgreSQL URLs replace known helper-created JDBC settings in local .env. Use scripts/smoke_sessions.py to check the running session API without Bluetooth. Use scripts/smoke_telemetry.py to send two synthetic measurements and three connection events and finish the session without Bluetooth. See wattlab-api/README.md for the contract and opt-in integration tests.

Rust backend tek crate içinde hexagonal yapı kullanır: domain ve uygulama portları merkezde, HTTP ve PostgreSQL adaptörleri dış katmandadır. Yapı ve test ayrıntıları için [API rehberi](wattlab-api/README.md).

## Yerel canlı ekran

API çalışırken http://127.0.0.1:8080/ adresini aç. Maskeli alana yerel `.env` içindeki
API anahtarını gir; anahtar yalnızca sekme belleğinde tutulur. Node veya ayrı bir
frontend kurulumu gerekmez. Python göndericisi çalışınca ölçümler otomatik görünür;
eski/eksik ölçümler sıfır yerine çizgiyle gösterilir. Ekran trainer'ı başlatmaz veya
kontrol komutu göndermez. Ayrıntılar ve testler API rehberindedir.

## Tek komutla yerelde başlat

```bash
.venv/bin/python scripts/run_local.py --offline
# Cihaz erişimini açıkça etkinleştirmek için:
.venv/bin/python scripts/run_local.py --offline --trainer
```

Varsayılan komut trainer'a bağlanmaz. Başlatma, kapatma, FTP ayarı ve sorun giderme
adımları [yerel kullanım rehberinde](docs/LOCAL_RUN.md). 200 W varsayılan FTP yalnızca
görüntüleme referansıdır; ekran ayarından sekme boyunca değiştirilebilir.
