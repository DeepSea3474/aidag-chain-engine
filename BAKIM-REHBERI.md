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
# FAIL-CLOSED: anahtar dosyasi (SOULWARE_KEY_PATH) yoksa/bozuksa servis ACILMAZ (cikis 2).
# Yeniden baslatmadan sonra: journalctl -u soulware-kubra -n 30 | grep -E 'HATA|imzalayan'
# "imzalayan" adresi mainnet::KUBRA_IMZA_ADRESI ile ayni olmali (bolum 8).

### Web sitesi
cd /var/www/aidag-chain && npm run build && pm2 restart aidag-web

---

## 8. KUBRA IMZA ANAHTARI ROTASYONU (soulware-core / soulware-kubra)

KUBRA'nin zincir imza adresi konsensusteki RWA yasak listesinde PINLIDIR
(lsc-engine/src/mainnet.rs: KUBRA_IMZA_ADRESI + RWA_YASAKLI_ADRESLER). Anahtar
degisirse adres degisir; liste GUNCELLENMEZSE yeni KUBRA adresi yasak listesinde
OLMAZ. (Yeni adrese rol M-of-N verilmedikce yazamaz; ama ikinci koruma katmani
kaybolur.) Ayrinti: RWA_ORACLE_KYC_TASARIM.md "KUBRA anahtar rotasyonu".

KURALLAR:
- ESKI adres listeden CIKARILMAZ. Liste konsensus sabitidir: cikarmak gecmisin
  yeniden oynatilmasini degistirebilir; eski anahtar ele gecmis de olabilir.
  Yeni adres EKLENIR (liste yalniz buyur).
- Liste degisikligi KONSENSUS degisikligidir: PR + testler + MAINNET REPLAY +
  iki mainnet dugumu AYNI ANDA ayni binary (bolum 7). Mainnet'te etkinlestirme onay ister.
- Ozel anahtar ASLA ekrana, log'a, PR'a, sohbete yazilmaz. Yalniz ACIK adres kullanilir.

ADIMLAR:
1. Yeni anahtari YENI bir yola uret ve ACIK adresini ogren (ozel anahtar basilmaz):
     SOULWARE_KEY_PATH=/root/aidag-lsc/.soulware.key.yeni \
       /root/aidag-lsc/target/release/soulware-core --yeni-anahtar-uret
   Cikti yalniz yolu ve "imzalayan : 0x..." adresini basar; servis BASLAMAZ.
   Dosya varsa (bozuk olsa bile) uretim REDDEDILIR; mevcut dosyanin uzerine yazilmaz.
   FAIL-CLOSED: soulware-core anahtar dosyasi YOKSA ya da BOZUKSA acilmayi reddeder
   (cikis kodu 2); sessizce yeni anahtar URETMEZ (soulware-core/src/imza_dosyasi.rs).
2. mainnet.rs: KUBRA_IMZA_ADRESI'ni yeni adresle degistir; ESKI degeri ayri bir
   sabit olarak (orn. KUBRA_IMZA_ADRESI_ESKI_1) RWA_YASAKLI_ADRESLER'de BIRAK.
3. Testi guncelle: node.rs rwa_gercek_kubra_adresi_yasakli_rol_alamaz icindeki
   hex'i yeni adrese cevir; eski adresin de yasakli kaldigini dogrulayan assert ekle.
4. Testler yesil + MAINNET REPLAY birebir (TESTLER.md) -> PR -> main -> bolum 7 deploy.
5. ANCAK deploy sonrasi yeni dosyayi SOULWARE_KEY_PATH yoluna tasi (ya da servis
   ortamindaki SOULWARE_KEY_PATH'i yeni yola cevir), soulware-kubra'yi baslat ve dogrula:
     journalctl -u soulware-kubra -n 50 --no-pager | grep 'imzalayan'
   Basilan adres mainnet.rs'teki KUBRA_IMZA_ADRESI ile AYNI olmali.
6. Yeni adrese rol verilmedigini kontrol et (bos olmali):
     curl -s http://127.0.0.1:8645/kurum/<yeni_adres_hex_0xsiz> | grep roller
7. Eski anahtar dosyasini guvenli sekilde imha et/arsivle (OFFLINE); adresi listede kalir.

Not: mainnet'te RWA kapaliyken (RWA_MAINNET_AKTIVASYON = None) liste davranisi
etkilemez; yine de rotasyonda liste HER ZAMAN guncellenir ki aktivasyonda eksik kalmasin.

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
