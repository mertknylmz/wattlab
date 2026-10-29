# WattLab’ı yerelde çalıştırma

Bu akış Ubuntu içindir. Rust toolchain, PostgreSQL araçları, proje `.venv` ortamı
ve `requirements.txt` bağımlılıkları önceden kurulmuş olmalı. Backend Rust/Axum’dur;
Bluetooth bağlantısı Python’da kalır. Desktop veya mobil paket kurulumu gerekmez.

## Ekran ve API

WattLab kök klasöründeki terminalden:

```bash
.venv/bin/python scripts/run_local.py --offline
```

Başlatıcı WattLab’a ait PostgreSQL’i hazırlar, Rust uygulamasını derler, API’nin
hazır olduğunu doğrular ve tarayıcıda yerel ekranı açar. Bu komut **trainer’a bağlanmaz**.
İlk kurulumda eksik Cargo bağımlılıkları varsa internetle bir kez `--offline` olmadan
çalıştır; sonraki çalıştırmalarda önbellekle devam edebilirsin.

Ekranın maskeli alanına kökteki yerel `.env` içindeki `WATTLAB_API_TOKEN` değerini
gir. Anahtarı paylaşma; başlatıcı onu ekrana veya bağlantı adresine yazmaz.
Sayfa ve canlı veri aynı yerel API’den gelir, ayrı frontend sunucusu yoktur.

Tarayıcıyı kendin açmak için:

```bash
.venv/bin/python scripts/run_local.py --offline --no-browser
```

Varsayılan adres http://127.0.0.1:8080/; `WATTLAB_API_PORT` ile değişebilir.
RustRover’dan API’yi zaten çalıştırdıysan başlatıcı aynı portu devralmaz. Önce
kendi API çalıştırmanı durdur veya farklı port seç. Başlatıcı başka süreçleri kapatmaz.

## Gerçek trainer ile çalıştırma

Trainer açık ve erişime hazır olmalı. Zwift gibi diğer trainer istemcilerini kapat.
Yerel `.env` içinde `TRAINER_ADDRESS` doğru olmalı. Cihaz erişimini açıkça etkinleştir:

```bash
.venv/bin/python scripts/run_local.py --offline --trainer
```

Bu seçenek Python dinleyicisini API hazır olduktan sonra başlatır; geçerli yerel
API adresini dinleyiciye süreç ayarı olarak aktarır. Kalıcı `.env` içindeki adresi
ve anahtarı kopyalayıp başka dosyalara yazmaz. Dinleyici oturum açar ve ölçümleri
Rust/PostgreSQL’e gönderir. Direnç/ERG kontrol komutu gönderilmez.

Bağlantı ve veriler hazır olduğunda hafifçe pedal çevir. Pedalı bırakınca grafiğin
sıfıra inişini gözlemek istiyorsan dinleyiciyi hemen kapatma: gerçek 0 W ölçümlerinin
gelmesini bekle. Veri yokluğu sıfır değildir; kesinti grafikte boşluk bırakır.

## Durdurma

Başlatıcının terminalinde **Ctrl+C** kullan. Önce Python dinleyicisine kontrollü
kapanış istenir; böylece kuyruk boşaltılabilir ve oturum API üzerinden bitirilebilir.
Ardından API kapatılır. Her sahip olunan süreç için kapanış bütçesi 30 saniyedir;
takılan süreç sonunda zorla sonlandırılır. Böyle bir durumda oturum bitişi garanti
edilemez. Veritabanı geliştirmeye devam edebilmen için çalışır kalır.

PostgreSQL’i ayrıca kapatmak istersen, onu kullanan diğer çalışmaları bitirdikten sonra:

```bash
.venv/bin/python scripts/local_db.py stop
```

Ekrandaki “Ekran bağlantısını kes” yalnızca sekmenin izlemesini durdurur; Python
veya trainer oturumunu kapatmaz. Tarayıcı sayfasını kapatmak da antrenmanı durdurmaz.

## FTP ve renkli zonlar

Varsayılan **200 W**, yalnızca görüntüleme için bir başlangıç referansıdır; senin
FTP ölçümün değildir. Ekranın FTP alanından 1–2000 arasında tam sayı watt girip
“Uygula”ya basabilirsin. “200 W’a dön” varsayılanı geri getirir. Değer bu sekmede
kalır; sayfayı yenileyince 200 W’a döner. Kullanıcı profiline kaydetme henüz yoktur.

Güç kartı ve grafik bu FTP’ye göre renklenir; ham watt değerleri değişmez. Ayarı
sonradan değiştirince görünür grafik geçmişi de yeni referansla yeniden renklenir.
Sıfır watt Z0’dır; eksik/eski/negatif güç bir antrenman zonuna atanmaz. Z1–Z6
sırasıyla gri, mavi, yeşil, sarı, turuncu ve kırmızıdır. Renk yanında metin etiketi
ve yüzde vardır; bilgi yalnızca renkle verilmez.

Kesintisiz yüzde sınırları: Z1 `(0,60)`, Z2 `[60,76)`, Z3 `[76,90)`,
Z4 `[90,105)`, Z5 `[105,119)`, Z6 `[119,∞)`. Bunlar uygulamanın açık sınırlarıdır;
[Zwift’in güç zonu renklerinden](https://support.zwift.com/es/using-the-heads-up-display-when-cycling-rJepA4beS)
esinlenir. FTP ayarı trainer direncini değiştirmez.

## Kontroller ve mevcut sınırlar

```bash
node wattlab-api/tests/web.cjs
.venv/bin/python -m unittest discover -s scripts/tests -v
```

Node yalnızca arayüz davranış testleri için gereklidir, uygulamayı çalıştırmak için
gerekmez. Başlatıcının DB/API açılışı ve güvenli kapanışı gerçek yerel ortamda,
zonların davranışı sentetik verilerle tarayıcıda doğrulandı. Yeni başlatıcının
`--trainer` akışı bu aşamada gerçek cihazla ayrıca denenmedi; mevcut ayrı Python →
Rust → web akışı daha önce D500 üzerinde doğrulanmıştı. Kullanıcı profili, kalıcı FTP,
masaüstü/mobil paketleme ve trainer donanım kontrolü henüz uygulanmadı.
