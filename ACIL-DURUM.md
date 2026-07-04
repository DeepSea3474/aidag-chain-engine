# AIDAG-Chain — Acil Durum Rehberi

Bir sey ters gittiginde PANIK YAPMA. Once teshis, sonra kurtarma.
Sirayla: NE bozuldu? -> ilgili bolume bak -> kurtarma komutunu calistir.

---

## HIZLI TESHIS (once bunu calistir - ne durumda?)

# Node ayakta mi, zincir calisiyor mu, disk/bellek nasil
systemctl is-active lsc-node.service
curl -s http://127.0.0.1:8645/status
journalctl -u lsc-node.service -n 20 --no-pager
df -h /        # disk dolu mu
free -h        # bellek durumu

---

## DURUM 1: NODE COKTU / CALISMIYOR

### Once logları oku (NEDEN coktu)
journalctl -u lsc-node.service -n 50 --no-pager

### Yeniden baslat
systemctl restart lsc-node.service && sleep 3 && systemctl is-active lsc-node.service

### Hala kalkmiyorsa: son degisiklik neydi? Build bozuk olabilir.
cd /root/aidag-lsc && cargo build --release 2>&1 | tail -20
# Build hata veriyorsa -> DURUM 3 (kod geri alma)

---

## DURUM 2: YANLIS KUTUPHANE GUNCELLEMESI (guncelleme bozdu)

### Yedekten don (guncelleme oncesi Cargo.lock)
cd /root/aidag-lsc && ls Cargo.lock.YEDEK-*     # mevcut yedekleri gor
cd /root/aidag-lsc && cp Cargo.lock.YEDEK-TARIH Cargo.lock   # TARIH'i degistir
cd /root/aidag-lsc && cargo build --release && cargo test --lib 2>&1 | grep "test result:"

---

## DURUM 3: KOD BOZULDU (bir degisiklik her seyi bozdu)

### Son commit'e don (kaydedilmemis degisiklikleri IPTAL et)
cd /root/aidag-lsc && git status              # once ne degismis gor
cd /root/aidag-lsc && git stash               # degisiklikleri gecici kaldir (geri alinabilir)
# ya da tamamen sil: git checkout .

### Belirli bir eski commit'e don (SON CARE - dikkatli)
cd /root/aidag-lsc && git log --oneline -10   # commit gecmisi
# git checkout COMMIT_HASH -- dosya.rs        # tek dosyayi eski haline al

### YEDEK dosyalar (elle alinmis .YEDEK-* dosyalari)
cd /root/aidag-lsc && find . -name "*.YEDEK-*" 2>/dev/null
# ornek geri alma: cp lsc-net/src/rpc.rs.YEDEK-XXX lsc-net/src/rpc.rs

---

## DURUM 4: TESTLER KIRILDI (279 degil, bazilari FAILED)

### Hangi test kirildi gor
cd /root/aidag-lsc && cargo test --lib 2>&1 | grep -A2 "FAILED\|panicked"

### Son calisan haline don (git)
cd /root/aidag-lsc && git stash    # son degisiklikleri kaldir, testi tekrar dene
cd /root/aidag-lsc && cargo test --lib 2>&1 | grep "test result:"

---

## DURUM 5: OWNER ANAHTARI SORUNU (EN KRITIK)

### Anahtar dosyasi duruyor mu
ls -la /root/faucet_anahtar.txt

### Anahtar KAYBOLDUYSA:
# Eger yedegin varsa -> yedekten geri koy (guvenli yerden)
# Yedegin YOKSA -> hazine kontrolu kaybolur. Bu yuzden anahtar
#   MUTLAKA offline/guvenli bir yerde yedekli olmali (BAKIM-REHBERI bolum 6).

### Owner adresini dogrula (env ile eslesmelı)
systemctl show lsc-node.service -p Environment | tr ' ' '\n' | grep FAUCET_OWNER

---

## DURUM 6: DISK DOLDU

### Ne yer kapliyor bul
du -sh /root/aidag-lsc/target 2>/dev/null   # build ciktilari genelde buyuk
df -h /

### Build cache temizle (guvenli - tekrar derlenebilir)
cd /root/aidag-lsc && cargo clean
# Not: sonra ilk build uzun surer (her sey yeniden derlenir)

---

## DURUM 7: SITE (aidag-chain.com) COKTU

### Site process ayakta mi
ss -tlnp | grep 3000

### Yeniden baslat
cd /var/www/aidag-chain && pkill -f "next-server"; sleep 2; nohup npm run start > /tmp/aidag-site.log 2>&1 &
sleep 6 && ss -tlnp | grep 3000

### Site loglari
tail -30 /tmp/aidag-site.log

---

## GENEL KURTARMA ILKESI

1. PANIK YAPMA. Once TESHIS (ne bozuldu), sonra kurtarma.
2. Her zaman GIT var: kaydedilmis her sey geri alinabilir (git log, git checkout).
3. YEDEK dosyalar: .YEDEK-* ve Cargo.lock.YEDEK-* geri donus noktalari.
4. Kurtarmadan sonra MUTLAKA dogrula: cargo build + cargo test + node aktif mi.
5. Emin degilsen: hicbir seyi SILME. Once yedekle, sonra dene.

## GERI DONULEMEZ olanlar (COK DIKKAT)
- Owner anahtari kaybi -> hazine kontrolu gider (yedek sart)
- git push --force -> uzak gecmisi bozabilir (kullanma)
- Zincir verisi silme -> vertexler kaybolur
Bunlarda ISLEM YAPMADAN once iki kez dusun, yedekle.
