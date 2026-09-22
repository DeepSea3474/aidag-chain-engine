# AIDAG-Chain — Bakim ve Guvenlik Rehberi

Bu dosya, sistemi saglikli tutmak icin gereken komutlari icerir.
Her komutu terminalde calistirabilirsin. Aciklama + komut seklinde.

---

## 1. GUVENLIK TARAMASI (bagimlilik aciklari)

### Bagimliliklardaki bilinen guvenlik aciklarini tara
cd /root/aidag-lsc && cargo audit

# Cikti: "0 vulnerabilities" = temiz. Acik varsa liste + cozum onerisi verir.
# Periyodik calistir (haftada/ayda bir). Yeni acik cikabilir.

---

## 2. KUTUPHANE GUNCELLEME (aciklari kapatmak icin)

### ADIM 1: Once yedek al (geri donebilmek icin) - HER ZAMAN
cd /root/aidag-lsc && cp Cargo.lock Cargo.lock.YEDEK-$(date +%Y%m%d)

### ADIM 2: Neyin guncellenecegini GOR (degistirmez, sadece gosterir)
cd /root/aidag-lsc && cargo update --dry-run

### ADIM 3: Guncelle
cd /root/aidag-lsc && cargo update

### ADIM 4: KRITIK - guncelleme bir sey bozdu mu KONTROL ET
cd /root/aidag-lsc && CARGO_TARGET_DIR=/root/aidag-build cargo build --release 2>&1 | tail -3
cd /root/aidag-lsc && CARGO_TARGET_DIR=/root/aidag-build cargo test --release -p lsc-engine -p lsc-net -p soulware-core 2>&1 | grep "test result:"

# Test YESIL degilse (0 failed degilse) -> yedekten don:
# cp Cargo.lock.YEDEK-TARIH Cargo.lock && CARGO_TARGET_DIR=/root/aidag-build cargo build --release
# (Canliya alma HER ZAMAN bolum 7'deki prosedurle.)

---

## 3. KOD KALITESI TARAMASI (kendi kodundaki sorunlar)

### Clippy: kendi kodundaki hatalar, kotu pratikler
cd /root/aidag-lsc && cargo clippy --release 2>&1 | tail -30

### Unsafe kod taramasi (guvenli olmayan blok kullanimi)
# Once kur (bir kez): cargo install cargo-geiger
cd /root/aidag-lsc && cargo geiger 2>&1 | tail -30

---

## 4. TESTLER (motor saglikli mi)

### UYARI: ayri derleme klasoru kullan
# Sunucudaki target/release/lsc-node, CANLI iki mainnet dugumunun calistirdigi dosyadir.
# `cargo build/test --release -p lsc-net` onu EZER; dugum yeniden baslarsa onaysiz kod canliya gecer.
# Gelistirme ve test icin HER ZAMAN:
export CARGO_TARGET_DIR=/root/aidag-build

### Tum testleri calistir (lsc-engine 339 + lsc-net + soulware-core 19 yesil olmali)
cd /root/aidag-lsc && CARGO_TARGET_DIR=/root/aidag-build cargo test --release -p lsc-engine -p lsc-net -p soulware-core 2>&1 | grep "test result:"

### Konsensus degisikligi varsa: MAINNET REPLAY (zorunlu) — bkz. TESTLER.md "MAINNET REPLAY"

---

## 5. SERVIS DURUMU (calisiyor mu, saglikli mi)

### Mainnet dugumleri (IKISI de ayni binary'yi calistirmali)
systemctl is-active lsc-node aidag-mainnet
curl -s http://127.0.0.1:8645/status ; echo ; curl -s http://127.0.0.1:8655/status
# vertex_count ve genesis (b82345008ae109d8) iki dugumde AYNI olmali; orphan_count 0.
for s in lsc-node aidag-mainnet; do sha256sum /proc/$(systemctl show -p MainPID --value $s)/exe; done

### KUBRA ve ogreniciler
systemctl is-active soulware-kubra soulware-bilim soulware-learn soulware-github kubra-watchdog
curl -s http://127.0.0.1:8646/health
journalctl -u soulware-github -n 10 --no-pager     # GitHub ogrenici (kod calistirmaz)

### On satis izleyici (timer ile periyodik calisir)
systemctl list-timers on-satis-izleyici.timer --no-pager
journalctl -u on-satis-izleyici -n 20 --no-pager   # "IADE gerekir" satirlarini kontrol et

### Web sitesi
pm2 list | grep aidag-web

---

## 6. YEDEKLEME (onemli dosyalar)

### Owner anahtari yedegi (COK ONEMLI - guvenli, OFFLINE yere kopyala)
# aidag-kurucu.key -> kaybolursa on satis/TGE yonetimi kaybolur.

### Canli binary + veri yedekleri
ls -la /root/lsc-node-canli-yedek/     # her deploy'da tarihli klasor (eski binary + iki veri dosyasi)

### Kod GitHub'da — degisiklikler PR ile main'e girer (dogrudan main'e push YOK)

---

## 7. MAINNET'E YUKLEME (DEPLOY) — GUVENLI PROSEDUR

Kural: iki mainnet dugumu HER ZAMAN AYNI ANDA ayni binary'ye gecer; karisik surum
(biri eski biri yeni) konsensus ayrismasina yol acar.

1. Degisiklik main'de (PR birlesmis), testler yesil, konsensus degistiyse MAINNET REPLAY birebir ayni.
2. Ayri klasorde derle:
   cd /root/aidag-lsc && git switch main && git pull --ff-only
   CARGO_TARGET_DIR=/root/aidag-build cargo build --release -p lsc-net
3. Yedek al:
   Y=/root/lsc-node-canli-yedek/deploy-$(date +%Y%m%d-%H%M%S); mkdir -p $Y
   cp /root/aidag-mainnet/aidag-data-mainnet.log aidag-mainnet-40001.log $Y/
   cp target/release/lsc-node $Y/lsc-node-eski
4. Oncesi durumu kaydet: curl -s 127.0.0.1:8645/status ; curl -s 127.0.0.1:8645/on-satis-ozet
5. Binary'yi atomik degistir ve IKI dugumu birlikte yeniden baslat:
   cp /root/aidag-build/release/lsc-node target/release/lsc-node.yeni && mv target/release/lsc-node.yeni target/release/lsc-node
   systemctl restart lsc-node aidag-mainnet
6. Dogrula: iki /status ayni (vertex_count, genesis), on satis ozeti ve TGE oncesiyle ayni, loglarda panic yok.
7. Geri donus (gerekirse): cp $Y/lsc-node-eski target/release/lsc-node && systemctl restart lsc-node aidag-mainnet

### KUBRA (soulware-core) — zincire dokunmaz, tek servis
cd /root/aidag-lsc && cargo build --release -p soulware-core && systemctl restart soulware-kubra

### Web sitesi
cd /var/www/aidag-chain && npm run build && pm2 restart aidag-web

---

## DUZENLI BAKIM RUTINI (onerilen)

HAFTALIK:
  1. cargo audit                         (yeni guvenlik acigi var mi)
  2. testler (bolum 4)                   (hepsi yesil mi)
  3. servis durumu (bolum 5)             (iki dugum ayni binary + ayni durum mu)
  4. on satis izleyici loglari           ("IADE gerekir" var mi)

GUNCELLEME YAPARKEN (her zaman bu sirayla):
  1. Yedek al (Cargo.lock)
  2. cargo update --dry-run (gor)
  3. cargo update (yap)
  4. build + test (DOGRULA; ayri CARGO_TARGET_DIR)
  5. Test yesil degilse yedekten don

ACIK KALAN:
  - Profesyonel bagimsiz audit (Rust L1 bilen firma)
  - Farkli sunucu/konumlarda dagitik cok-dugumlu ag
  - Owner anahtari donanim cuzdanina

---
Not: Her guncelleme sonrasi MUTLAKA test calistir.
"Once kanit, sonra vaat" — test yesil gormeden hicbir degisikligi kabul etme.
