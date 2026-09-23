# AIDAG-Chain — Test Suite ve Audit Hazirligi

Durum: MAINNET CANLI (26 Temmuz 2026, Chain ID 3474); bagimsiz denetim (audit) bekliyor. Guncelleme: 2026-09-22.
Amac: Denetime hazir, seffaf ve tekrar calistirilabilir bir test tablosu sunmak.

## Tum testleri calistirma
- CARGO_TARGET_DIR=/root/aidag-build cargo test --release   (lsc-engine 339 + lsc-net + soulware-core 19)
  UYARI: sunucuda varsayilan target/ klasoru CANLI dugumlerin binary'sidir; -p lsc-net
  derlemesi onu ezer. Gelistirme/test icin HER ZAMAN ayri CARGO_TARGET_DIR kullan.
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

## BAGIMLILIK DENETIMI (cargo audit)
cargo audit calistirildi. Cikan 5 uyari (rand, tokio, derivative, paste,
proc-macro-error2) DOLAYLI bagimliliklardan gelir (revm, tokio, alloy'un ic
bagimliliklari) ya da "unmaintained" bildirimidir. Kendi kodumuzda dogrudan
guvenlik acigi yok. Bu crate'ler ust bagimliliklar (ozellikle revm) guncellendikce
duzelecek; takip ediliyor. cargo clippy: gercek bug yok, sadece olu-kod uyarilari.

## COK-NODE AG KESIF NOTU (10 Tem)
2 node ayri ayri baslatilinca: mDNS kesif, baglanti, pull/push-sync protokolu
CALISIYOR (peer=1, abone oldu). ANCAK her node KENDI genesis'ini urettigi icin
(farkli imza anahtari -> farkli genesis) "ikinci genesis reddedildi" ve senkron
0 vertex entegre etti. Node dogru davraniyor (sahte 2. genesis'i reddetmek guvenli).
MAINNET GEREKSINIMI: tum node'lar AYNI sabit/pinli genesis'ten baslamali (Bitcoin
gibi gomulu genesis). Bu, genesis.rs + token dagitimi (21M, 6-dilim) ile birlikte
tasarlanmali; mainnet-oncesi, acele edilmeden. Kod zaten bunu biliyor (lib.rs:
"gercek mainnet genesis'i pinli/vesting'li olacak").

## GENESIS BAGLAMA PLANI (karar)
genesis.rs YAZILDI ve test edildi (21M AIDAG, 6-dilim GenesisDagitim). ANCAK henuz
node'a BAGLI DEGIL (her node kendi gecici genesis'ini uretir). KARAR: gercek cuzdan
adresleri + vesting + node'a baglama (paylasilan sabit genesis) AUDIT SONRASINA
birakildi. Sebep: genesis geri alinamaz; adresler/multisig/vesting proje olgunlasip
audit yapilinca netlesir. Sira: testler -> kod dondur -> audit -> genesis sabitle -> mainnet.

## PERFORMANS KANITI (olculmus, tekrarlanabilir)

İki bağımsız test, O(n) lineer ölçeklenmeyi (O(n²) darboğazı YOK) kanıtlar.
Tüm ölçümler: imza doğrulama + insert + artımlı GHOSTDAG (mavi_boncuk) dahil.

### olcek_egrisi (node.rs)
| vertex | süre(s) | TPS  |
|--------|---------|------|
| 100    | 0.016   | 6199 |
| 1000   | 0.164   | 6084 |
| 5000   | 0.805   | 6212 |
| 10000  | 1.521   | 6573 |
TPS düşmüyor → DAG büyüme maliyeti yok → O(n) lineer.

### torba_stres (ghostdag.rs)
| vertex | süre(s) | TPS  |
|--------|---------|------|
| 20000  | 3.3     | 6097 |
| 40000  | 6.7     | 6014 |
| 80000  | ~13     | ~6200 |
n 2x → süre 2x (TPS sabit) → O(n) lineer doğrulandı.

### imza_paralel_bench (ghostdag.rs)
- 100.000 imza: tek çekirdek 14.272/s, paralel 192.627/s (13.5x hızlanma).

### Güvenlik (9 fuzz kalkanı, elle çalıştırma, 149s)
Hepsi geçer: sahte token, sahte belge, replay, bakiye/çift-harcama,
çorba (karışık yük), geçersiz vertex, invariant, determinizm, doğrulama.

Çalıştırma:
- cargo test --release --lib olcek_egrisi -- --ignored --nocapture
- TORBA_N=20000,40000,80000 cargo test --release --lib torba_stres -- --ignored --nocapture
- cargo test --release --lib fuzz -- --ignored

## CUSTODY / KONSENSUS SERTLESTIRME TESTLERI (2026-09-22)
Her biri once ESKI kodda BASARISIZ, yeni kodda BASARILI oldugu dogrulanarak eklendi:
- gunluk_cap_eski_tarihli_vertexle_asilamaz — gecmis tarihli satislar gunluk 100k tavanini asamaz (zincir saati)
- tge_eski_tarihli_vertexle_geri_cekilemez — eski tarihli tip=15 TGE'yi geriye cekemez
- reorg_tam_yeniden_hesap_gunluk_sayaci_sifirlar — reorg sonrasi durum == sifirdan yukleyen dugum
- mainnet_bos_dugum_genesis_sonrasi_panik_yapmaz — bos veriyle acilan mainnet dugumu ilk vertex'te coker degil
- es_sync_gelecek_tarihli_vertexi_reddeder — esten gelen gelecek tarihli vertex (kural 7) reddedilir
- tge_gecmise_ayarlanamaz / tge_gunu_gelince_kesinlesir — 3 gun bildirim + TGE gunu kesinlik
- on_satis_asamalar_otomatik_devam_toplam_tavan_asilamaz — 630k sonrasi satis kesilmez, 1.680.000'de durur
- rpc::mainnet_kapisi_testleri — mainnet'te faucet/test_bakiye/lsc_test_bakiye hicbir vertex yazmaz

## RWA ORACLE + KYC TESTLERI (dal: rwa-oracle-kyc) — bkz. RWA_ORACLE_KYC_TASARIM.md
- node::rwa_tests::rwa_yonetim_* — M-of-N: 2 imza gecer, 1 imza red, ayni imzaci 2 kez sayilmaz, replay red, esik altina dusurme red, imzaci/esik degisikligi M-of-N, owner tek anahtarla veremez, suresi dolan/baska ag imzasi red, kurulmamis yonetim; rwa_mainnet_genesis_ve_dagitim_degismedi
- node::rwa_tests::rwa_kubra_benzeri_rolsuz_servis_hicbir_rwa_islemi_yapamaz — rolsuz servis anahtari (yasak listesi olmadan) yalniz tip=1 yazar; 17/18/19/20 etkisiz
- node::rwa_tests::rwa_eski_tarihli_vertex_bayatligi_atlatamaz — eski tarihli vertex zincir saatini geri almaz, bayat veriyi tazelemez, rapor zamani = zincir saati
- node::rwa_tests::rwa_kurum_dogrulama_* — kurum dogrulama yalniz M-of-N; owner/kurum/tek imza veremez; kayitsiz adres dogrulanmaz; mevcut belge/kurum kayitlari birebir korunur
- node::rwa_tests::rwa_mainnet_genesis_ve_mevcut_vertexler_kurum_dogrulamadan_etkilenmez — pinli genesis + mevcut tip vertex'ler; dogrulama denemesi kayitlari degistirmez
- node::rwa_tests::rwa_gercek_kubra_adresi_yasakli_rol_alamaz — gercek KUBRA adresi sabit yasak listesinde; rol alamaz
- node::rwa_tests::rwa_olcum_zamani_ileri_veya_eskiyse_rapor_reddedilir / rwa::tests::olcum_zamani_ileride_veya_pencereden_eskiyse_reddedilir — olcum zamani penceresi
- rwa_precompile::tests — secici keccak, Ethereum precompile cakismasi (tum spec), Chainlink ABI, int256 isaret, bayat/durmus/hatali cagri revert, KYC isApproved, RWA kapaliyken Ethereum ile ayni, kontrat STATICCALL, sabit gaz (2599 OOG / 2600 basari, revert ayni gaz), deger reddi
- node::rwa_tests::rwa_precompile_* — eth_call ile zincir durumunu okur; mainnet'te kapali
- soulware-core imza_dosyasi::tests — anahtar fail-closed: yok/bozuk red, --yeni-anahtar-uret var olan dosyanin uzerine yazmaz, 0600, mesaj sizdirmaz
- tx::rwa_tx_tests — tip 17..20 kodlama: gidis-donus, eksik/fazla bayt, yanlis tip, gecersiz alan, tip numarasi cakismasi
- oracle_hesap::tests — alt medyan, sapma siniri, i128 sinirlarinda tasmasiz 256-bit karsilastirma, eleme, girdi sirasindan bagimsizlik, kesici, bayatlik
- rwa::tests — tur kapanisi, tekrar rapor, yanlis tur, bayat acik tur, devre kesici durdur/ac, rolu iptal edilenin raporu, tur gecmisi budama, KYC onay/iptal
- node::rwa_tests — bildirim suresi, kendi kendine kurum kaydi yetki vermez, owner/KUBRA yazamaz, rol iptali aninda, eski tarihli vertex bildirimi kisaltamaz, taze + ters sirali dugum ayni durum, mainnet'te kapali
- rpc::rwa_rpc_testleri — /oracle, /kyc, /kurum roller (i128 deger string)

## MAINNET REPLAY (konsensus degisikligi oncesi ZORUNLU)
Canli veri dosyasinin KOPYASI eski ve yeni kodla oynatilir; ozet birebir ayni olmali:
  cp /root/aidag-mainnet/aidag-data-mainnet.log /tmp/kopya.log
  MAINNET_REPLAY_DOSYA=/tmp/kopya.log CARGO_TARGET_DIR=/root/aidag-build \
    cargo test --release -p lsc-net --test mainnet_replay_ozet -- --ignored --nocapture | grep '^OZET'
(on satis kayitlari, TGE, arz/hesap sayilari, tum imzalayan/alici adreslerin bakiye+nonce'u,
 dosyadaki HER belge hash'inin ve HER kurum imzalayaninin turetilmis kaydi)
Kurum dogrulama geriye uyum kontrolu (ayni kopya ile):
  MAINNET_REPLAY_DOSYA=/tmp/kopya.log CARGO_TARGET_DIR=/root/aidag-build \
    cargo test --release -p lsc-net --test mainnet_kurum_dogrulama_ozet -- --ignored --nocapture

