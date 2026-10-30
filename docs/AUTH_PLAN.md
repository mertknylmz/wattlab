# Kullanıcı hesapları (login/register) planı

Tarih: 30 Ekim 2026. Durum: **uygulanıyor (30 Ekim 2026).** Uygulanana kadar mevcut paylaşılan token modeli geçerlidir (Electron her
açılışta `WATTLAB_API_TOKEN` üretir; ekran token görmez, `/api/*` istekleri Electron ana
sürecinde Bearer eklenerek iletilir — bkz. `wattlab-desktop/src/renderer-protocol.js`).

## İstek

Masaüstü uygulaması açılınca login/register ekranı gelsin; kullanıcı giriş yapıp
kullanıcıya özel token alsın. Paylaşılan API anahtarıyla uğraşılmasın.

## Kesin kararlar (kullanıcı)

- Hesaplar yerel PostgreSQL'de (internetsiz çalışma kuralı korunur).
- **Tek veritabanı `memberdb`, tek şema `member`**; tüm uygulama verisi orada.
  Test DB'si `memberdb_test` (önerilen ad).
- Hesap tablosu `member.member`: `username`, `password`, `created_at`, `updated_at`
  (+ `id`). `password` kolonu **düz şifre değil Argon2id hash** saklar; kolon adı
  kullanıcının tercihidir, değiştirme.
- Aynı bilgisayarda çok kullanıcı; oturumlar, FTP ve geçmiş kullanıcıya ayrı.
- Kimlik: kullanıcı adı + şifre (e-posta yok).
- "Beni hatırla": uygulama yeniden açılınca giriş açık kalır.
- Mevcut oturum/FTP verisi taşınmaz; eski `wattlab_rust_dev` dokunulmadan kalır
  (silmek ayrıca kullanıcı onayı ister).

## Cevaplanan sorular (kullanıcı, 30 Ekim 2026)

1. Şifremi unuttum: **şimdilik yok** (sıfırlama betiği/akışı ekleme).
2. Aktif antrenman oturumu varken çıkış: **engellenir**; önce oturumu bitirmesi
   istenir (API `POST /auth/logout` sahibin aktif oturumu varsa 409 döner, ekran
   mesaj gösterir).
3. Kayıt: **herkese açık** (bilgisayarı kullanan herkes kayıt olabilir).
4. Mobil ağ erişimi: **karar ertelendi**; API yalnız 127.0.0.1'de kalır.

Kullanıcı planı onayladı ve uygulamayı başlattı (30 Ekim 2026).

## İlerleme

Bu bölümü her adım bitince güncelle (devralan agent buradan devam eder).

- [x] 1. DB: `local_db.py` `memberdb`/`memberdb_test` + `member` şeması oluşturur
  (eski `wattlab_rust_*` dokunulmaz). Rust bağlantısı `search_path=member` kullanır
  (`postgres/config.rs` `SCHEMA`), başlangıçta şemayı yoksa açar; SQL nitelendirilmez.
  Eski 1–4 migration tek `1_training.sql` taban migration'ında birleşti. Test DB
  koruması artık `memberdb_test` adını ister. 60 + 20 PostgreSQL testi geçti.
- [x] 2a. Rust hesaplar: migration `2_members.sql` (`member`, `auth_session`);
  domain `member.rs` (`Username`, `Password`, `Member`, `SessionToken`, `TokenHash`);
  port `member_repository.rs` + async `password_hasher.rs`; `auth_service.rs`
  (register/login/authenticate/logout, 5 hatalı denemede 30 sn kilit, bilinmeyen
  kullanıcıda sahte doğrulama ile eşit süre, 30 gün kayan süre); adapter
  `argon2_hasher.rs` (spawn_blocking) ve `postgres/member_storage.rs`; HTTP
  `account.rs` (`/auth/register|login` public, `/auth/logout|me` korumalı),
  `auth.rs` middleware `Principal::{Agent, Member}` ekler. Config/env artık
  `WATTLAB_AGENT_TOKEN` (`local_db.py` eski `WATTLAB_API_TOKEN` değerini taşır ve
  siler; Electron/Python/betikler yeniden adlandırıldı). Hata kodları:
  `invalid_username`, `invalid_password`, `username_taken` (409),
  `invalid_credentials` (401), `too_many_attempts` (429), ajan `/auth/me` → 403.
  Testler: 70 donanımsız (`tests/accounts.rs` dahil) + 21 PostgreSQL; Clippy temiz.
  **Geçici durum:** ajan token'ı hâlâ tüm uçlara erişir ve Electron ekranı bu token
  ile vekil üzerinden çalışmaya devam eder (4. adıma kadar).
- [x] 2b. Veri sahipliği uygulandı (önceki turda kod, bu turda testler): üye
  uçları `MemberId`, ajan uçları `AgentOnly`, `Scope` ajan+sahip; `command_id`
  ile sahip; `trainer_in_use` / `active_session` 409. Postgres testleri komut
  akışı + üye token kullanır; çapraz üye 404/409 testi eklendi.
- [x] 3. Python ajanı `command_id`'yi oturum başlatma isteğine koyar.
- [x] 4. Electron: üye token'ı ana süreçte; login/register yanıtından token
  çıkarılır; `safeStorage` (basic_text yazılmaz); 401/logout unutur.
- [x] 5. UI: giriş/kayıt, beni hatırla, çıkış (aktif oturumda 409).
- [x] 6. `docs/LOCAL_RUN.md` giriş/kayıt; smoke betikleri üye+komut akışı;
  entegrasyon testleri `--test-threads=1` (tek trainer). Electron duman testi
  bu turda yeniden koşulmadı — `wattlab-desktop` içinde `npm start`.

## Önerilen tasarım

### Akış

1. Electron DB/API/ajanı başlatır.
2. Saklı token varsa `/auth/me` ile doğrulanır → Canlı görünüm; yoksa/geçersizse
   Giriş/Kayıt ekranı.
3. Giriş/kayıt yanıtındaki token'ı **Electron ana süreci** alır, ekrana token'sız
   yanıt (`{ username }`) döner, sonraki `/api/*` isteklerine Bearer ekler.
4. Çıkış: API'de token iptal edilir, saklı token silinir, giriş ekranına dönülür.

### Veritabanı (`memberdb`, şema `member`)

Yeni DB boş başladığı için migration seti baştan yazılır (eski 1–4 public şemaya
yazıyordu; yeni set `member.` nitelikli statik SQL kullanır).

| Tablo | Alanlar / kural |
|---|---|
| `member.member` | `id UUID`, `username` (lower(username) üzerinde tekil indeks), `password` (Argon2id PHC string), `created_at`, `updated_at` |
| `member.auth_session` | `id`, `member_id` FK, `token_hash` (SHA-256, tekil), `created_at`, `last_used_at`, `expires_at` |
| `member.training_session` | mevcut alanlar + `member_id` FK, `ftp_w` |
| `member.telemetry_event`, `member.connection_event` | mevcut yapı, oturuma FK |
| `member.athlete_settings` | `member_id` PK/FK, `ftp_w` (1–2000, varsayılan 200), `updated_at` |

Tek aktif oturum kuralı global kalır (tek fiziksel trainer): `finished_at IS NULL`
için kısmi tekil indeks veya mevcut atomik SQL korunur.
`scripts/local_db.py`: `wattlab_rust_dev/_rust_test` yerine `memberdb/memberdb_test`,
`.env` URL'leri ve test DB koruması (şu an `_rust_test` soneki arar) güncellenir.

### Rust API (hexagonal yapı korunur, `wattlab-api/AGENTS.md`)

- Domain: `Username` (3–32, `[a-z0-9_.-]`, küçük harfe normalize), `Password`
  politikası (8–128 karakter, kompozisyon kuralı yok), `Member`, auth hataları.
- Application: `PasswordHasher` portu (adapter: `argon2` crate, Argon2id,
  `tokio::task::spawn_blocking`), `MemberRepository`, `AuthSessionRepository`,
  `AuthService` (register/login/logout/authenticate).
- Token: 32 bayt rastgele, base64url, opak (JWT değil); DB'de yalnız hash. 30 gün
  kayan süre (kullanıldıkça `last_used_at`/`expires_at` uzar). Çıkışta silinir.
- Uçlar:
  - `POST /api/v1/auth/register` `{username, password}` → 201 `{member, token, expires_at}`; tekrar ad → 409.
  - `POST /api/v1/auth/login` → 200 aynı gövde; hatada her zaman aynı 401 mesajı
    ("kullanıcı adı veya şifre hatalı"); bellek içi deneme yavaşlatma.
  - `POST /api/v1/auth/logout` → 204.
  - `GET /api/v1/auth/me` → `{id, username}`.
- İki kimlik türü (middleware Bearer'ı çözer):
  - **Member**: sessions/history/settings/live/stream ve trainer komutu oluşturma;
    sorgular `member_id` ile filtrelenir; başkasının kaydı 404.
  - **Agent** (`WATTLAB_AGENT_TOKEN`, sabit-zaman karşılaştırma): `/agent/*`,
    telemetri/bağlantı olayı kabulü, ajan oturum başlat/bitir. Ajan oturum
    başlatırken `command_id` gönderir; sahip, komutu oluşturan member olur.
  - Aktif oturumu yalnız sahibi (ekrandan) bitirebilir; ajan kapanışta bitirebilir.
- `WATTLAB_API_TOKEN` ve `ApiToken` kaldırılır; config `WATTLAB_AGENT_TOKEN` ister.
  Sırlar Debug/log'da gizli kalır. SQL statik, unwrap/panic yok, fmt/Clippy temiz.

### Python ajanı (`experiments/bluetooth/`)

- `trainer_agent.py`, `agent_client.py`, `telemetry_sender.py`, `live_trainer.py`:
  `WATTLAB_AGENT_TOKEN` kullan; oturum başlatma isteğine komut kimliği ekle.
- BLE davranışı değişmez. Testler güncellenir (44 test bugün geçiyor).

### Electron (`wattlab-desktop/`)

- `renderer-protocol.js`: `/api/v1/auth/login|register` yanıtından token'ı çıkar,
  ekrana token'sız gövde dön; `logout` sonrası token'ı unut; 401'de token'ı unut.
  Ekranın gönderdiği Authorization/cookie yine atılır.
- `main.js`: ajan için açılış başına rastgele `WATTLAB_AGENT_TOKEN`; kullanıcı token'ı
  bellekte. "Beni hatırla" için `safeStorage` ile şifreleyip `app.getPath('userData')`
  altına yaz. Linux'ta `safeStorage.getSelectedStorageBackend()` `basic_text` ise
  **diske yazma**; ekran "beni hatırla kullanılamıyor" der. Açılışta saklı token
  `/auth/me` ile doğrulanır.
- Ekran hâlâ token görmez; preload/IPC köprüsü yoktur.

### Arayüz (`wattlab-ui/`)

- Giriş ve Kayıt ekranları (kayıtta şifre tekrar, "beni hatırla" kutusu), alan
  doğrulama ve API hata mesajları.
- Giriş yoksa diğer rotalar açılmaz (`GET /auth/me` → 401 ise giriş ekranı).
- Üst bilgi: kullanıcı adı + Çıkış. 401 gelirse giriş ekranına dön (şu anki
  "uygulamayı yeniden aç" mesajının yerine).
- Testler: Vitest (`pool: 'threads'`).

### Betikler ve belgeler

- `scripts/run_local.py` (ekransız): `WATTLAB_AGENT_TOKEN` `.env`'den; kullanıcı
  curl ile register/login yapar. `scripts/smoke_*.py` test kullanıcısı kaydeder.
- Güncellenecek: `docs/LOCAL_RUN.md`, `README.md`, `wattlab-api/README.md`,
  `wattlab-api/AGENTS.md`, `AGENTS.md`, `PROJECT_CONTEXT.md`, `RUST_PLAN.md`.

## Uygulama sırası

1. DB: `local_db.py` + yeni migration seti (`memberdb`, `member` şeması).
2. Rust domain/application/adapters + auth uçları + member/agent kimlikleri +
   veri sahipliği; birim ve PostgreSQL testleri.
3. Python ajanı token/komut kimliği değişikliği + testler.
4. Electron auth vekili + safeStorage + testler.
5. UI giriş/kayıt/çıkış + testler.
6. Betikler, belgeler, uçtan uca doğrulama.

## Doğrulama hedefi

- Rust: kayıt/giriş/yanlış şifre/tekrar ad, token iptal ve süre dolması, başka
  kullanıcının oturum/geçmiş/ayarına erişememe (404), ajan sahiplik kuralı.
- Electron açılışı: kayıt → kapat-aç ile beni hatırla → çıkış → ikinci kullanıcı
  birincinin geçmişini görmez; ekranda token yok.
- Gerçek trainer testi otomatik başlatılmaz (ayrı kullanıcı onayı).

## Ortam notları (önceki çalışmadan)

- Node 22+ gerekir; sistemde Node 18 var. Electron kurulumu için
  `npx -y -p node@22 which node` ile Node 22 kullanılabilir. `npm ci` sonrası
  `chrome-sandbox` için `sudo chown root:root` + `chmod 4755` gerekir.
- Cursor sandbox'ı farklı `CARGO_TARGET_DIR` kullanabilir; gerçek binary için
  `unset CARGO_TARGET_DIR` ile sandbox dışında derle.
- DB: `.venv/bin/python scripts/local_db.py start`. Kontroller:
  `scripts/run_api.py --test --offline`, `--integration --offline`,
  `(cd wattlab-ui && npm test)`, `(cd wattlab-desktop && npm test)`,
  `(cd experiments/bluetooth && ../../.venv/bin/python -m unittest discover -s tests)`,
  `.venv/bin/python -m unittest discover -s scripts/tests`.
- Bugünkü durum: 60 Rust + 20 PostgreSQL, 31 UI, 13 desktop, 44+4 Python testi geçiyor.
