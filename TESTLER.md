# AIDAG-Chain — Test Suite ve Audit Hazirligi

Durum: Testnet asamasi, bagimsiz denetim (audit) oncesi hazirlik.
Amac: Denetime hazir, seffaf ve tekrar calistirilabilir bir test tablosu sunmak.

## Tum testleri calistirma
- cargo test --release                    (tum birim testleri, 287+)
- cargo test --release -- --ignored       (agir fuzz/kalkan testleri)
Her fuzz testi tur sayisini bir ortam degiskeniyle ayarlar (orn FUZZ_TUR=20000).

## KONSENSUS TESTLERI

### 1. fuzz_dogrula - coloring dogrulugu
Ne yapar: binlerce rastgele DAG'da incremental GHOSTDAG vs referans hesap,
bit-bit ayni sonuc (blue_score, mergeset_blues).
Neden: near-linear optimizasyon hizi artirdi; bu test hizin DOGRULUGU
bozmadigini kanitlar. Son: 20.000 tur, hepsi fark=0 OK.

### 2. fuzz_determinizm - konsensus determinizmi
Ne yapar: ayni vertex kumesi FARKLI ekleme sirasinda bit-bit ayni sonuc.
Neden: node'lar bloklari farkli sirada alir ama ayni zincire varmali (fork
olmamali). BTreeMap seciminin determinizmi korudugunu da kanitlar.
Son: 20.000 tur, farkli sira -> ayni sonuc OK.

### 3. fuzz_invariant - konsensus degismezleri
Ne yapar: uc kural her senaryoda tutmali:
- INV1 blue_score sp-zincirinde monoton (sp_bs <= bs)
- INV2 bir blok hem mavi hem kirmizi olamaz (ayrik)
- INV3 k-cluster: her mavinin anticone boyutu <= k
Neden: GHOSTDAG'in matematiksel temel kurallari; biri ihlal olursa mantik
bozuktur. Son: 2.000 tur, INV1+INV2+INV3 tuttu OK.

## GUVENLIK KALKANI TESTLERI (adversarial)

### 4. fuzz_kalkan_sahte_token - sahte token kalkani
Ne yapar: binlerce sahte token (ayni sembol, farkli adres); kalkan taklidi
yakalamali, gercek token'i yanlis pozitif vermemeli, codec kimligi korumali.
Neden: token kimligi = kanonik adres, sembol degil. "USDC gorunumlu ama sahte
adresli" dolandiricilik en yaygin DEX tuzagi. Son: 2.000 tur OK.

### 5. fuzz_kalkan_sahte_belge - belge/kayit kalkani
Ne yapar: kayitli belge dogrulanir; sahte/tahrif (bir byte degisik) reddedilir;
ilk kayit korunur (uzerine yazilamaz).
Neden: RWA/belge dogrulama (diploma teyidi) temeli. Belge kimligi hash'tir;
tek byte degisse hash tumden degisir -> tahrif yakalanir. Son: 2.000 tur OK.

### 6. fuzz_kalkan_corba - kaos testi (kalkanlar bir arada)
Ne yapar: her turda KARISIK parti (gercek+sahte imza/token/belge ic ice);
kalkanlar her ogeyi dogru ayiklamali (gercek gecer, sahte reddedilir, karismaz).
Neden: gercek dunyada saldirilar ayni anda ve karisik gelir. Kalkanlarin
BIRLIKTE dogru calistigini kanitlar. Son: 2.000 tur, kabul=5550 red=5501 OK.

## BIRIM TESTLERI
287+ birim testi: graph, vertex, reachability, interval, torba, coloring,
total_order, RPC, store I/O. Calistirma: cargo test --release.

## OLCEK / PERFORMANS (olculdu)
- 10.000.000 vertex, ~5000 vertex/sn sabit throughput (near-linear kaniti).
- Bellek lineer (~3.6 GB / 1M vertex). Not: figur imza dogrulama + GHOSTDAG
  icerir; disk kaliciligi ve ag katmani haric.
- ed25519 imza dogrulama: paralel 11.3x hizlanma (18 cekirdek).

## AG (multi-node) DOGRULAMASI (elle)
mDNS peer kesfi, pull/push senkronizasyon, node'lar ayni zincire yakinsiyor
(orphan=0). Near-linear kod ile 2 node senkronu dogrulandi.

## AUDIT ONCESI TODO (henuz yapilmadi)
- Otomatik multi-node entegrasyon testi (CI)
- Genis-olcek/patolojik DAG fuzz
- EVM/AVM uyumluluk (revm) - Ethereum test suite
- cargo clippy, cargo audit, miri
- CI pipeline
- Uzun sureli calisma / bellek sizintisi testi

## BAGIMSIZ UZMAN GEREKTIREN (bize ait DEGIL)
- Konsensus guvenlik / oyun-teorik analiz (balance attack, selfish mining)
- Ekonomik saldiri modellemesi
- Kriptografik derinlik denetimi
- Bagimsiz guvenlik audit'i (para tutan / mainnet oncesi ZORUNLU)

NOT: Ic testler bir audit'in YERINI TUTMAZ. Bunlar audit'e HAZIR girmek icindir.
Gercek deger/para tutan asamadan once bagimsiz denetim sarttir.

## BIRIM TESTLERI
287+ birim testi: graph, vertex, reachability, interval, torba, coloring,
total_order, RPC, store I/O. Calistirma: cargo test --release.

## OLCEK / PERFORMANS (olculdu)
- 10.000.000 vertex, ~5000 vertex/sn sabit throughput (near-linear kaniti).
- Bellek lineer (~3.6 GB / 1M vertex). Not: figur imza dogrulama + GHOSTDAG
  icerir; disk kaliciligi ve ag katmani haric.
- ed25519 imza dogrulama: paralel 11.3x hizlanma (18 cekirdek).

## AG (multi-node) DOGRULAMASI (elle)
mDNS peer kesfi, pull/push senkronizasyon, node'lar ayni zincire yakinsiyor
(orphan=0). Near-linear kod ile 2 node senkronu dogrulandi.

## AUDIT ONCESI TODO (henuz yapilmadi)
- Otomatik multi-node entegrasyon testi (CI)
- Genis-olcek/patolojik DAG fuzz
- EVM/AVM uyumluluk (revm) - Ethereum test suite
- cargo clippy, cargo audit, miri
- CI pipeline
- Uzun sureli calisma / bellek sizintisi testi

## BAGIMSIZ UZMAN GEREKTIREN (bize ait DEGIL)
- Konsensus guvenlik / oyun-teorik analiz (balance attack, selfish mining)
- Ekonomik saldiri modellemesi
- Kriptografik derinlik denetimi
- Bagimsiz guvenlik audit'i (para tutan / mainnet oncesi ZORUNLU)

NOT: Ic testler bir audit'in YERINI TUTMAZ. Bunlar audit'e HAZIR girmek icindir.
Gercek deger/para tutan asamadan once bagimsiz denetim sarttir.

### 7. fuzz_kalkan_replay - replay/cift-harcama kalkani
Ne yapar: NonceRegistry uzerinde binlerce senaryo - dogru nonce kabul edilir,
REPLAY (kullanilmis nonce tekrar) reddedilir, ATLAMA (ileri nonce) reddedilir,
adresler bagimsiz.
Neden: cift-harcama (ayni parayi iki kez kullanmak) bir blockchain'in en temel
saldirisidir. Nonce sirasi bunu engeller. Para tutan sistemin en kritik kalkani.
Son: 2.000 tur, replay ve atlama reddedildi OK.

### 8. fuzz_kalkan_gecersiz_vertex - vertex dogrulama kalkani
Ne yapar: her turda gecerli imzali vertex uretilir (once kabul edildigi kontrol
edilir), sonra rastgele bozulur: bozuk imza, tahrif payload, sahte id, asiri
payload, asiri parent. verify() hepsini reddetmeli.
Neden: vertex dogrulama, zincire giren HER blogun ilk guvenlik kapisidir. Bu
kalkan delinirse kotu/sahte bloklar zincire girer. Blok girisinin temel savunmasi.
Son: 2.000 tur, tum bozuk vertex'ler reddedildi OK.

### 9. fuzz_kalkan_bakiye - bakiye/transfer kalkani
Ne yapar: binlerce rastgele transfer - EN KRITIK invariant: her transfer sonrasi
toplam ARZ SABIT (para yoktan var olmaz, yok olmaz); yetersiz bakiye reddedilir
(bakiye degismez); basarili transferde gonderenden tam duser, aliciya tam eklenir.
Neden: nonce (test 7) cift-harcamayi engeller; bu test paranin BUTUNLUGUNU korur.
Ikisi birlikte para tutan sistemin temel guvenligi. Son: 2.000 tur, arz korundu OK.
