# WattLab’ı yerelde çalıştırma

Bu akış Ubuntu içindir. Rust toolchain, PostgreSQL araçları, proje `.venv` ortamı
ve `requirements.txt` bağımlılıkları önceden kurulmuş olmalı. WattLab yalnızca masaüstü
uygulamasıdır; tarayıcıdan açılan bir ekran yoktur.

Parçalar:

- **Masaüstü uygulaması** (`wattlab-desktop/`, Electron): pencereyi açar, aşağıdaki
  süreçleri başlatıp kapatır. Ekranları uygulamanın içinden gösterir.
- **Ekranlar** (`wattlab-ui/`, React): derlenip masaüstü uygulamasının içine konur.
- **API** (`wattlab-api/`, Rust): oturumlar, ayarlar ve PostgreSQL. Ekran sunmaz;
  yalnızca yerel HTTP/SSE API’sidir (ileride mobil uygulama da aynı API’yi kullanır).
- **Trainer ajanı** (Python): Bluetooth bağlantısı ve trainer komutları.

## İlk kurulum

Node 22+ gerekir (yalnızca geliştirme ve derleme için; örn. `nvm install 22`).

```bash
(cd wattlab-api && cargo build)
(cd wattlab-ui && npm ci)
(cd wattlab-desktop && npm ci)
```

Ubuntu 24.04+ Chromium sandbox’ı için Electron’un yardımcı dosyasının root sahipli
olması gerekir (her `npm ci` sonrası tekrarla):

```bash
cd wattlab-desktop
sudo chown root:root node_modules/electron/dist/chrome-sandbox
sudo chmod 4755 node_modules/electron/dist/chrome-sandbox
```

## Çalıştırma

Trainer kullanacaksan trainer açık olmalı; Zwift gibi diğer trainer istemcilerini kapat.
Trainer adresi artık `.env` ile sabitlenmez. Ayarlar’dan Bluetooth taraması
yapılır, listeden cihaz seçilir; son seçim `localStorage`’da kalır.

```bash
cd wattlab-desktop
npm start
```

`npm start` önce ekranları derler (`wattlab-desktop/renderer/`), sonra uygulamayı açar.
Uygulama yerel veritabanını hazırlar, API’yi boş bir yerel portta ve her açılışta yeni
üretilen bir erişim anahtarıyla başlatır, trainer ajanını (bağlanmadan) açar ve
pencereyi gösterir. Anahtar girmen gerekmez ve ekranlar anahtarı hiç görmez: ekranın
API istekleri uygulamanın ana sürecinden geçer, anahtarı ana süreç ekler. `.env`
içindeki anahtar değişmez. Ajanı başlatmamak için `WATTLAB_DESKTOP_TRAINER=0 npm start`.
API kodunu değiştirdiysen önce `cargo build` çalıştır.

İlk açılışta **Giriş / Kayıt ol** ekranı gelir (kullanıcı adı + şifre). “Beni
hatırla” işaretliyse ve işletim sistemi şifreli depolama sunuyorsa (Linux’ta
anahtarlık) sonraki açılışta giriş atlanır. Aktif bir antrenman varken çıkış
engellenir. Ekranlar API anahtarı görmez ve sormaz.

Ekranlar: **Canlı görünüm** (anlık değerler, son 60 saniyelik güç grafiği, trainer
kontrolleri), **Geçmiş** (kayıtlı oturumlar ve özetleri), oturum detayı (saniyelik
güç grafiği, ortalamalar, iş/kJ, zon dağılımı), **Profil** (yıllık antrenman
takvimi: gün başına antrenman sayısı ve süre) ve **Ayarlar** (FTP, Bluetooth
bağlan/kes). Profil, Ayarlar ve Çıkış sol panelin altındadır; uzun sayfada yalnızca
sağ taraf kayar. Aynı yerde TR/EN ve koyu/açık tema vardır; varsayılan
dil İngilizce’dir, tercih `localStorage` ile bu bilgisayarda kalır (veritabanına
yazılmaz). Profil takvimi Pazar’dan Cumartesi’ye gider.

## Trainer paneli

Ajan başlarken trainer’a **bağlanmaz**; ekrandan komut bekler. Ayarlar’da **Tara**,
listeden cihaz seç, sonra bağlan. Son seçim bu bilgisayarda kalır. Sağ üstteki bağlantı simgesi duruma gider;
tıklayınca **Ayarlar → Bağlantı** açılır. Canlı ekranda yalnızca oturum
başlat/bitir vardır. ERG ve Bluetooth **Ayarlar → Bağlantı**’dadır; Genel sekmesi FTP içindir:

1. **Trainer’a bağlan** — ajan seçilen adrese Bluetooth bağlantısını kurar ve veri kanallarına abone olur. Trainer yanıt verene kadar “bağlı” sayılmaz. Son seçilen trainer bu bilgisayarda hatırlanır (seçili ≠ bağlı). Uygulama açılınca, cihaz açıksa o adrese bir kez yeniden bağlanır; oturum otomatik başlamaz. Koparınca aynı açılışta tekrar bağlanılmaz.
2. **Oturumu başlat** — yalnızca trainer bağlıyken (veri akıyorken) açılır.
   API’de yeni oturum oluşur; ölçümler bu oturuma kaydedilir. Oturum o anki FTP’yi saklar.
3. Pedal çevir. Pedalı bırakınca gerçek 0 W ölçümlerinin gelmesini bekle; veri
   yokluğu sıfır değildir, kesinti grafikte boşluk bırakır.
4. **Oturumu bitir** — ajan bekleyen ölçümleri gönderir ve oturumu kapatır.
5. **Bağlantıyı kes** (Ayarlar) — varsa ERG hedefi bırakılır, sonra bağlantı kapanır.

Her komutun sonucu panelin altında görünür: bekleniyor, tamamlandı, gerekçeli ret
(örneğin “aktif oturum yok”) veya zaman aşımı. Ajan çalışmıyorsa panel bunu söyler ve
düğmeler devre dışıdır. Ekran trainer’ın komutu uyguladığını varsaymaz.

### Hedef güç (ERG)

Trainer bağlı ve FTMS hedef güç desteğini bildiriyorsa **Ayarlar → Bağlantı** içindeki
ERG alanı etkinleşir; trainer’ın bildirdiği güç aralığı gösterilir. Hedefi watt olarak
girip **Uygula**’ya bas; trainer direnci bu güce göre ayarlar. **Direnci bırak** hedefi
kaldırır. Bağlantı koparsa
ajan kontrol durumunu unutur; bağlanırken ve direnç bırakılınca leftover ERG silinir, trainer 0% simülasyona (serbest sürüş) alınır. Eski hedef otomatik gönderilmez.
ERG komutları D500 üzerinde henüz doğrulanmadı; ilk denemeyi düşük bir hedefle yap.

## Kapatma

Pencereyi kapat (veya terminalde Ctrl+C). Önce ajan kontrollü kapanır (aktif oturum
bitirilir, ERG bırakılır, bağlantı kesilir), ardından API. Her biri için 30 sn bütçe
vardır; takılan süreç sonra zorla sonlandırılır ve bu durumda oturum bitişi garanti
edilemez. PostgreSQL çalışır kalır. Ayrıca kapatmak için, onu kullanan diğer
çalışmaları bitirdikten sonra:

```bash
.venv/bin/python scripts/local_db.py stop
```

## Paket

`npm run dist` ekranları derleyip bir AppImage üretir. Paket henüz API binary’sini,
Python ajanını ve PostgreSQL’i içermez; çalıştırırken `WATTLAB_ROOT` hazırlanmış WattLab
klasörünü göstermelidir. Bu bileşenlerin paketlenmesi açık bir karardır.

## Arayüzsüz geliştirici başlatıcısı

Ekran olmadan yalnızca DB + API (isteğe bağlı ajan) gerektiğinde, örneğin betik veya
`curl` ile deneme için:

```bash
.venv/bin/python scripts/run_local.py --offline            # DB + API
.venv/bin/python scripts/run_local.py --offline --trainer  # + trainer ajanı
```

API http://127.0.0.1:8080/api/v1 adresindedir (`WATTLAB_API_PORT` ile değişir) ve
`.env` içindeki `WATTLAB_API_TOKEN` ile Bearer kimlik doğrulaması ister. Aynı portta
çalışan başka bir API’yi başlatıcı devralmaz veya kapatmaz. Ctrl+C sahip olduğu
süreçleri sırayla kapatır.

## FTP ve renkli zonlar

FTP **Ayarlar** ekranından 1–2000 arasında tam sayı watt olarak kaydedilir ve
PostgreSQL’de kalıcıdır. Başlangıç değeri 200 W’tır ve kişisel ölçüm değildir;
“Varsayılana dön” bu değeri geri getirir. FTP trainer direncini değiştirmez.

Her yeni oturum başladığı andaki FTP’yi saklar; oturum detayındaki zon dağılımı bu
değere göredir, sonradan FTP değiştirmek eski oturumları değiştirmez. FTP’si
kaydedilmemiş eski oturumlar güncel FTP ile gösterilir ve bu belirtilir. Canlı
grafik ve güç kartı güncel FTP ile renklenir; ham watt değişmez.

Sıfır watt Z0’dır; eksik/eski/negatif güç bir zona atanmaz. Z1–Z6 sırasıyla gri,
mavi, yeşil, sarı, turuncu ve kırmızıdır; renk yanında metin etiketi ve yüzde vardır.
Yüzde sınırları: Z1 `(0,60)`, Z2 `[60,76)`, Z3 `[76,90)`, Z4 `[90,105)`,
Z5 `[105,119)`, Z6 `[119,∞)`; [Zwift’in güç zonu renklerinden](https://support.zwift.com/es/using-the-heads-up-display-when-cycling-rJepA4beS)
esinlenir.

## Kontroller ve mevcut sınırlar

```bash
(cd wattlab-ui && npm test)
(cd wattlab-desktop && npm test)
(cd experiments/bluetooth && ../../.venv/bin/python -m unittest discover -s tests)
.venv/bin/python -m unittest discover -s scripts/tests -v
.venv/bin/python scripts/run_api.py --test --offline
.venv/bin/python scripts/run_api.py --integration --offline
```

Sentetik veriyle doğrulananlar: API + trainer ajanı komut akışı (oturum başlat/bitir,
gerekçeli retler, sıralı kapanış), masaüstü uygulamasında canlı akış, ayarlar ve
açılış/kapanış. Gerçek trainer ile yeni ajan, uygulamadan bağlanma, gerçek 0 W duruşu
ve ERG komutları henüz denenmedi; bunlar ayrıca onaylanan cihaz testi gerektirir.
Mobil uygulama ve bileşenleri içeren tam masaüstü paketi yoktur.
