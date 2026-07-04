# AIDAG-Chain — Yayin ve Bakim Komutlari (Hizli Rehber)

Tum komutlar once bu dizine girmeyi gerektirir:
    cd /root/aidag-lsc

---

## 1. YAYINLA (kod degistirdikten sonra GitHub'a guvenli gonder)

    bash yayinla.sh "ne degistigini anlatan mesaj"

NE ZAMAN: Kodda/dokumanda bir sey degistirdin, GitHub'a gondermek istiyorsun.
NE YAPAR (tek komutta):
  1. Testleri calistirir — YESIL DEGILSE DURUR (bozuk kod push edilmez)
  2. README'deki test sayisini otomatik gunceller
  3. Degisiklikleri commit eder (senin mesajinla)
  4. GitHub'a push eder
  5. Saglik kontrolu yapar
ORNEK:
    bash yayinla.sh "Faucet hatasi duzeltildi"
    bash yayinla.sh "Yeni RPC ucu eklendi: eth_getCode"

NOT: Mesaj ZORUNLU. Mesajsiz calistirirsan uyarir, durur.
NOT: Yeni OZELLIK eklediysen, README'yi ELLE guncelle (script sadece
     test sayisini gunceller, "ne eklendi"yi sen yazmalisin), sonra yayinla.

---

## 2. SAGLIK KONTROLU (sistem iyi mi diye bak)

    bash saglik-kontrol.sh

NE ZAMAN: Duzenli (haftada bir), ya da "bir sey ters mi" diye merak edince.
NE YAPAR: Node, testler, guvenlik, git, disk, anahtar tarar.
          Her sorunun altina HAZIR DUZELTME KOMUTU yazar (kendi calistirmaz).

---

## 3. GUVENLIK TARAMASI (bagimlilik aciklari)

    cargo audit

NE ZAMAN: Ayda bir, ya da yeni kutuphane ekledikten sonra.
NE YAPAR: Kutuphanelerdeki bilinen guvenlik aciklarini listeler.
DUZELTME (once yedek):
    cp Cargo.lock Cargo.lock.YEDEK-$(date +%Y%m%d)
    cargo update && cargo build --release && cargo test --lib

---

## 4. SADECE TEST (kod calisyor mu, hizli kontrol)

    cargo test --lib

NE ZAMAN: Kod degistirdin, push etmeden once "bozuldu mu" diye bakmak icin.
BEKLENEN: "test result: ok. NNN passed; 0 failed"
(0 failed degilse: bir sey bozdun, push etme, once duzelt.)

---

## 5. NODE DURUMU (canli mi)

    systemctl is-active lsc-node.service      # aktif mi
    systemctl restart lsc-node.service        # yeniden baslat
    journalctl -u lsc-node.service -n 30 --no-pager   # son loglar

---

## ACIL DURUM (bir sey bozulunca)

    cat ACIL-DURUM.md      # kurtarma rehberi (node coktu, kod bozuldu vb.)

## DETAYLI BAKIM

    cat BAKIM-REHBERI.md   # tum bakim komutlari

---

## GUNLUK/HAFTALIK RUTIN (onerilen)

HER kod degisikliginden sonra:
    bash yayinla.sh "ne yaptin"

HAFTADA BIR:
    bash saglik-kontrol.sh

AYDA BIR:
    cargo audit

---
Ilke: "Once kanit." Test yesil gormeden hicbir sey push edilmez.
yayinla.sh bunu OTOMATIK uygular — testler kirilirsa push'u durdurur.
