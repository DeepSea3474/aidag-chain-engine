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
cd /root/aidag-lsc && cargo build --release 2>&1 | tail -3
cd /root/aidag-lsc && cargo test --lib 2>&1 | grep "test result:"

# Test YESIL degilse (0 failed degilse) -> yedekten don:
# cp Cargo.lock.YEDEK-TARIH Cargo.lock && cargo build --release

---

## 3. KOD KALITESI TARAMASI (kendi kodundaki sorunlar)

### Clippy: kendi kodundaki hatalar, kotu pratikler
cd /root/aidag-lsc && cargo clippy --release 2>&1 | tail -30

### Unsafe kod taramasi (guvenli olmayan blok kullanimi)
# Once kur (bir kez): cargo install cargo-geiger
cd /root/aidag-lsc && cargo geiger 2>&1 | tail -30

---

## 4. TESTLER (motor saglikli mi)

### Tum testleri calistir (279 test yesil olmali)
cd /root/aidag-lsc && cargo test --lib 2>&1 | grep "test result:"

### Tam test ciktisi (hata detayi icin)
cd /root/aidag-lsc && cargo test --lib 2>&1 | tail -40

---

## 5. NODE DURUMU (calisiyor mu, saglikli mi)

### Node aktif mi
systemctl is-active lsc-node.service

### Node loglari (son 30 satir, hata var mi)
journalctl -u lsc-node.service -n 30 --no-pager

### Zincir durumu (vertex sayisi, tips vb.)
curl -s http://127.0.0.1:8645/status

### Node'u yeniden baslat (gerekirse)
systemctl restart lsc-node.service && sleep 3 && systemctl is-active lsc-node.service

---

## 6. YEDEKLEME (onemli dosyalar)

### Owner anahtari yedegi (COK ONEMLI - guvenli yere kopyala)
# /root/faucet_anahtar.txt  --> bunu GUVENLI, OFFLINE bir yere yedekle
# Bu anahtar kaybolursa hazine kontrolu kaybolur. Bir yedek sart.

### Kod zaten GitHub'da (git push ile yedekli)
cd /root/aidag-lsc && git status
cd /root/aidag-lsc && git push origin main   # degisiklikleri GitHub'a yedekle

---

## 7. BUILD (derleme)

### Release build (production icin)
cd /root/aidag-lsc && cargo build --release 2>&1 | tail -3

### Build + restart (kod degistikten sonra)
cd /root/aidag-lsc && cargo build --release && systemctl restart lsc-node.service

---

## DUZENLI BAKIM RUTINI (onerilen)

HAFTALIK:
  1. cargo audit           (yeni guvenlik acigi var mi)
  2. cargo test --lib      (279 test hala yesil mi)
  3. systemctl is-active lsc-node.service  (node ayakta mi)

GUNCELLEME YAPARKEN (her zaman bu sirayla):
  1. Yedek al (Cargo.lock)
  2. cargo update --dry-run (gor)
  3. cargo update (yap)
  4. cargo build + cargo test (DOGRULA)
  5. Test yesil degilse yedekten don

MAINNET ONCESI (ileride):
  - Profesyonel audit (Rust L1 bilen firma)
  - Cok-node yuk testi
  - Owner anahtari donanim cuzdanina
  - Gercek genesis (21M pinli, tek-sefer)

---
Not: Her guncelleme sonrasi MUTLAKA cargo test calistir.
"Once kanit, sonra vaat" — test yesil gormeden hicbir degisikligi kabul etme.
