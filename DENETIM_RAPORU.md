# İç Denetim Raporu ve Düzeltmeler — 2026-09-24

**Kapsam:** `main` (986320a) ve `rwa-oracle-kyc` (f78a386). Beş alan paralel ve şüpheci bir gözle incelendi:
- konsensus ve durum,
- RPC ve ağ,
- imza, yetki ve AVM,
- KUBRA ve soulware servisleri,
- site, betikler ve bağımlılıklar.

**Yöntem:**
- Her bulgu bir kanıt testiyle gösterildi: izole kopya, `unshare -n` ağ ad alanında devnet ya da süreç içi mainnet kuralları.
- Canlı servislere dokunulmadı.

**Düzeltmeler:** `denetim-duzeltmeleri` dalında. Her düzeltmenin, saldırıyı yeniden deneyip artık **başarısız** olduğunu gösteren bir regresyon testi var.

**Mainnet geçmişi korunuyor:**
- Canlı mainnet verisinin bir kopyası (3901 vertex) eski ve yeni kodla yeniden oynatıldı.
- İki kodun ürettiği durum özetleri **birebir aynı**: bakiyeler, nonce'lar, ön satış kayıtları, 3787 belge kaydı ve zamanları.
- Kontrol testi: `lsc-net/tests/mainnet_replay_ozet.rs`.

## Test özeti (bu dal)

| Takım | Sonuç |
|---|---|
| Rust çalışma alanı (`cargo test --workspace`) | 469 geçti, 0 kaldı (öncesi 404) |
| Motor denetim regresyonu (`lsc-engine/src/denetim_regresyon.rs`) | 15/15 |
| RPC denetim regresyonu (`rpc.rs` `mainnet_kapisi_testleri`) | 6/6 (+2 mevcut) |
| Mainnet geçmişi yeniden oynatma eşitliği | birebir aynı (27 satır özet) |
| Belge araçları uçtan uca (izole, aday nginx) | 49/49 |
| KUBRA tuzlu-v2 uçtan uca (izole; eski ikili = canlı v1) | 50/50 |
| Koordinatör uçtan uca (gerçek düğüm, pay, 2 worker) | 33/33 |
| `katil.html` oturum anahtarı uçtan uca (jsdom + gerçek koordinatör) | 22/22 |
| Ön satış izleyicisi (`tests/test_on_satis_izleyici.py`) | 45/45 |
| XSS (`web-yedek/aidag/test/xss.test.js`) | 120/120 (eski kodda 16/44) |
| Belge çıktısı (PDF → 600 dpi → QR/DataMatrix/Code128 okuma) | 46/46 |
| `cargo audit` | Açık bağımlılık açığı kalmadı; hickory-proto yalnız mDNS yolunda (mainnet'te kapalı) |

## Bulgular ve düzeltmeler

Ciddiyet sütununda: K = kritik, Y = yüksek, O = orta, D = düşük.

### Konsensus, AVM ve yetki (`lsc-engine`)

| # | Bulgu | Ciddiyet | Düzeltme | Kanıt testi |
|---|---|---|---|---|
| 1 | Geçersiz AVM tx'i (ör. 50 KB initcode) tüm kontratları ve storage'ı siliyordu; bedava ve tekrarlanabilirdi | K | `avm_calistir_zincir`: db her durumda geri yükleniyor, tx db taşınmadan önce kuruluyor. Geçersiz tx taban gas öder, nonce ilerler. | `d1_gecersiz_avm_tx_kontratlari_silmez` |
| 2 | EVM yolu (tip 9/12, MetaMask) vesting kilidini atlıyordu | K | `avm_kilitli_calistir`: EVM'e yalnız **harcanabilir** bakiye (bakiye − vesting − rezerv) yükleniyor, kilitler sonra geri ekleniyor | `d2_evm_yolu_vesting_kilidini_asamaz` |
| 3 | tip12'de chainId kontrolü yoktu (BSC/ETH/chainId'siz tx replay); yüksek-S imza; testnet chainId = mainnet | K/Y | `ham_eth_tx_kabul_edilir`: chainId == ağın chainId'si (EIP-155 zorunlu), düşük-S (EIP-2). `evm_chain_id`: mainnet 3474, diğer ağlar 3_474_000_000 + id | `d3_...`, `r4_...` |
| 4 | EVM CHAINID opcode'u 1 dönüyordu | O | `cfg.chain_id` ve `TxEnv.chain_id` ağ kimliği | `d4_chainid_opcode_ag_kimligi` |
| 5 | tip11 imzası ağ kimliği içermiyordu (ağlar arası replay) | Y | Mesaj = `AIDAG-EvmTransfer-v2\0` ‖ chainId ‖ … | `d5_tip11_imzasi_baska_agda_gecersiz` |
| 6 | Stake bedavaydı ve başkası adına yapılabiliyordu (token gaspı + meşru ihraççının slash edilmesi) | Y | staker == imzalayan; miktar bakiyeden `STAKE_KASASI`'na aktarılır; slash edilen `AIDAG_YAKIM_ADRESI`'ne gider | `d6_...`, `taklit_deneyenin_stakei_yakilir` |
| 7 | tip8 eşleştirme imzalayana bağlı değildi (ödül gaspı) | O | test_adresi == imzalayan | `d7_eslestirme_yalniz_sahibi_yapar` |
| 8 | Ön satış tahsisi escrow'da rezerve değildi; owner boşaltabiliyordu | O | Rezerv = satılmış − claimlenmiş; owner harcayamaz, claim rezervden ödenir | `d8_escrow_rezervi_...` |
| 9 | **Kurucu TGE kararı:** genesis vesting sabit 27 Eylül'de başlıyordu; mainnet'te karar yoksa varsayılan tarih geçerliydi | O / istek | Mainnet'te tip15 kararı yoksa TGE **belirsiz** (u64::MAX), hiçbir genesis dilimi açılmaz. Vesting tabanı = max(plan başlangıcı, kurucu TGE kararı) | `d9_...`, `d9b_kurucu_tge_karari_vestingi_baslatir` |
| 10 | Owner tek işlemde 210M LSC basabiliyordu | Y | `COMPUTE_REWARD_GUNLUK_TAVAN` = 10.000 LSC/gün (zincir saati). Geçmiş toplam 87 LSC | `d10_odul_gunluk_tavani` |
| 11 | Belge/kurum kayıt zamanı geriye tarihlenebiliyordu | Y | `GUVENLIK_V2_AKTIVASYON` (2026-10-01) sonrası kayıt zamanı = zincir saati (monoton); geçmiş kayıtlar değişmez | `d11_...`, `d11b_...` |
| 12 | Çöp, bilinmeyen tip ve 1 MiB payload DAG'a kalıcı giriyordu (spam; senkronu kilitleme) | Y | Mainnet yapısal kuralı (`yapisal_islem_kurali`): bilinen tip, tipe göre çözülmesi, ≤ 60 KiB. Geçmişteki tek ihlal id ile muaf | `d12_mainnet_yapisal_islem_kurali` |
| 13 | `toplam_stake` u128 taşması `/status`'u kalıcı çökertiyordu | Y | `saturating` toplama; RPC'de metin | `d13_...`, `r3_...` |

### RPC ve ağ (`lsc-net`)

| # | Bulgu | Ciddiyet | Düzeltme | Kanıt testi |
|---|---|---|---|---|
| 14 | `/submit` her durumda `ok:true` dönüyordu; geçersiz işlemler DAG'a girip yayınlanıyordu | Y | `islem_on_kontrol` kapısı (bakiye, nonce, yetki, tekrar, tip, bilinmeyen ebeveyn). Yalnız `Integrated` başarı sayılır ve yayınlanır | `r1_...`, `r2_...` |
| 15 | `eth_getTransactionByHash`/`Receipt` her hash için uydurma "başarılı" dönüyordu | O | Gerçek dizin (`eth_islemleri`): bilinmeyen hash → null, başarısız → `status 0x0` | `r4_...` |
| 16 | `eth_sendRawTransaction` ön kontrolsüzdü | O | Aynı kapı (chainId, düşük-S, nonce, harcanabilir, gas) | `r1` + `d3` |
| 17 | `/eslestir` kimliksizdi ve yalnız RAM'de tutuluyordu | O | Uç kaldırıldı (yalnız zincirde, imzalı tip8) | kod |
| 18 | Gelecek tarihli orphan diske yazılıyordu; restart sonrası düğümler ayrışıyordu | Y | Orphan havuza/diske girmeden saat politikası + (mainnet) yapısal kural uygulanır. Disk yüklemede gelecek tarihli vertex elenir | kod + `restart_entegrasyon` |
| 19 | Orphan TTL temizliği hiç çağrılmıyordu | O | 15 sn'de bir, 600 sn TTL | kod |
| 20 | mDNS mainnet'te açıktı (+ hickory-proto DoS açıkları) | D/O | `LSC_MAINNET=1` iken mDNS `Toggle` ile kapalı | kod |
| 21 | Büyük vertex'ler pull-sync'i kilitleyebiliyordu (10 MiB CBOR sınırı); gossip sınırı örtük | Y | Parça başına 4 MiB bayt bütçesi, açık `max_transmit_size` 128 KiB, payload ≤ 60 KiB | kod |
| 22 | Push-sync yeni abone başına tüm DAG'ı yayınlıyordu (büyütme DoS'u) | O | 60 sn'de en fazla bir kez | kod |

### KUBRA ve koordinatör (`soulware-*`)

| # | Bulgu | Ciddiyet | Düzeltme | Kanıt testi |
|---|---|---|---|---|
| 23 | Kanıtta alan ayracı enjeksiyonu: sahte cevap `dogrulandi:true` alıyordu | Y | `tuzlu-v2`: uzunluk önekli kodlama. v1/eski kayıtlarda 0x1e varsa "belirsiz" | e2e_v2 |
| 24 | İstemci `context`'i hash'e girmiyordu | O | v2 kanıtına dahil; `istemci_baglami` alanı | e2e_v2 |
| 25 | Anahtar bozuk/eksikse sessizce yenisi üretiliyordu (geçmiş kanıtlar doğrulanamazdı) | O | Fail-closed (`imza_dosyasi.rs`) core, koordinatör ve worker'da. Yeni anahtar yalnız `--yeni-anahtar-uret` ile | e2e_v2, birim |
| 26 | `/kb/ingest`, `/kb/stats`, `/models`, `/embed-test` kimliksizdi (bilgi deposu zehirleme) | Y | Bearer token (`SOULWARE_YONETIM_TOKEN`), boyut sınırları, Unicode casefold başlık koruması. `/embed-test` kaldırıldı. `/retrieve` bilinçli olarak açık (tarayıcı işçisi) ama 32 KB + eşzamanlılık sınırlı | e2e_v2 |
| 27 | DoS: gövde 2 MB, eşzamanlılık sınırı yoktu, istemci `brain:"claude"` seçebiliyordu | Y | 32 KB gövde, beyin/medya semaforları (429), `claude` seçimi yok sayılır, `/` bilgi sızdırmaz | e2e_v2 |
| 28 | Sohbet her kayıtlı hash için "Belge DOĞRULANDI" diyordu | O | Kaydeden açıklanır: KUBRA etkileşimi / kurum (beyan) / kurum değil | birim |
| 29 | İçerik denetimi fail-open'dı | D | Fail-closed | e2e_v2 |
| 30 | Koordinatör kimliksizdi: sybil ile ödül, ücretsiz kota tüketimi, ödeme bağlanmamış | Y | Cüzdan imzası (ed25519 / EIP-191) + tarayıcı için 24 saatlik oturum anahtarı. Aynı cüzdan/IP tek oy, istemci başı günlük kota, ödeme belirli bir tip7 vertex'ine bağlı ve tek kullanımlık | e2e_coord, katil_oturum |

### Site ve betikler

| # | Bulgu | Ciddiyet | Düzeltme | Kanıt testi |
|---|---|---|---|---|
| 31 | KUBRA sayfasında `linkify` XSS'i (öznitelikten çıkış, `javascript:` href) | Y | DOM ile link üretimi; yalnız `https:` ve site içi link; `esc` tırnakları kaçırır | xss 120/120 |
| 32 | Kaçışsız hata metinleri, kurum adı sahteciliği (bidi/kontrol karakterleri) | D | textContent/esc; ad temizleme; "beyan" etiketi ayrı | xss, belge-çıktı |
| 33 | Ön satış izleyicisi tek public RPC'ye güveniyordu (sahte log → bedelsiz tahsis) | K | 2 bağımsız sağlayıcıdan birebir aynı receipt; status, USDT adresi, to, değer kontrolleri | izleyici 13 test |
| 34 | Blok işaretçisi ödeme işlenmeden ilerliyordu (ödeme kaybı); onay derinliği yoktu | Y/O | Kalıcı kuyruk, işaretçi atomik ilerliyor, 15 blok onay | izleyici 7 test |
| 35 | Durum dosyaları atomik değil, kilitsiz ve git'teydi; float kademe hesabı hatalıydı; asgari alım yoktu | O/D | Atomik yazım + flock + bozukta dur; Fraction; `MIN_USD=10`; `.gitignore` | izleyici 11 test |
| 36 | `on-satis-kaydet.sh` argüman hatası; `yayinla.sh` test kapısı yalnız ilk sonuca bakıyordu; `zincire-yaz.js` sabit seed kullanıyordu | D/O | Düzeltildi (dal + PR akışı; seed yok) | izleyici 10 test |
| 37 | `blake3.js` 1 KB üstü girdide standart dışı özet üretiyordu | Y | Standart BLAKE3 + geriye uyumlu doğrulama (986320a) | belge e2e |

### Bağımlılıklar ve lisans
- **Güncellenenler:** tokio 1.43.1, rand 0.8.6, tracing-subscriber 0.3.20, rustls 0.23.45. revm ve alloy konsensus için tam sürüme sabitlendi.
- **Lisans beyanı düzeltildi:** `NOTICE` ve `lsc-engine/README.md` "Apache-2.0" diyordu, oysa `LICENSE` = BUSL-1.1 (2030-07-10'da Apache-2.0'a dönüşür). soulware crate'lerine `license-file` eklendi.
- **Lisans uyumu:** GPL, AGPL ya da ticari olmayan kullanım şartlı bağımlılık yok.

## Kalan riskler (bu dalda çözülmedi — karar gerektirir)
1. **Tek owner (kurucu) anahtarı.** TGE kararı, ön satış tahsisi ve ödül basımı hâlâ tek anahtara bağlı. Günlük tavan, rezerv kilidi ve 3 günlük bildirim zararı sınırlıyor ama çalınan anahtar TGE'yi erteleyebilir. Gerçek çözüm M-of-N; `rwa-oracle-kyc` dalındaki YonetimRegistry deseni bu yolları da kapsamalı.
2. **Kurum kaydı beyana dayalı** (`main`). Doğrulanmış kurum özelliği `rwa-oracle-kyc` dalında. Araçlar ve çıktılar kurumu "beyan (doğrulanmamış)" diye yazıyor.
3. **`rwa-oracle-kyc` dalı bu düzeltmeleri içermiyor.** Dala bu çalışmada dokunulmadı. Birleştirilmeden önce `main`'e rebase edilmeli; ayrıca tip18 oracle akış tanımı tek owner anahtarıyla yapılıyor.
4. **Belge kaydı ücretsiz.** Geçerli bir tip1 kaydı herkes tarafından sınırsız yazılabilir. Ücret ya da hız modeli bir ürün kararı; RPC tarafında nginx `limit_req` önerilir.
5. **Tarihsel veriler:**
   - 32 sayfa kaydı eski blake3 ile yapılmış olabilir (doğrulama geriye uyumlu).
   - Git geçmişinde eski AI anahtarları (sitenin git geçmişi) ve alıcı tx/adresleri duruyor. Anahtarlar sağlayıcıda iptal edilmeli.
6. **Çok IP'li sybil** koordinatörde doğası gereği tam engellenemez. Ödüller elle onaylanıyor; oto-settlement kapalı.
7. **BNB fiyatı** tek kaynaktan (Binance) geliyor; BNB taraması varsayılan olarak kapalı.

## Kurulum planı (her aşama ayrı onay)
Düğüm güncellemesi **2026-10-01 00:00 UTC'den (`GUVENLIK_V2_AKTIVASYON`) önce** yapılmalı. Gecikirse sabit ileri alınıp mainnet yeniden oynatma eşitliği tekrar doğrulanmalı.

1. **Site ve nginx:**
   1. `web-yedek/aidag/*` ve `lib/`, `web/lib/blake3.js` kopyalanır.
   2. nginx yamaları uygulanır: `nginx-belge-hash.patch` ve `nginx-guvenlik.conf.patch` (CSP), ardından `nginx -t` ve reload.
   3. Önce yedek alınır. Koordinatörden önce yayınlanırsa `katil.html` imza isteyip 401 alır; bu yüzden 1. ve 3. aşama birlikte yapılmalı.
2. **Mainnet düğümleri (lsc-node):**
   1. Yeni ikili; iki düğüm birlikte ve yedekli yeniden başlatılır.
   2. Kontroller: vertex sayısı, `/status`, ön satış özeti, `ozet-once` ile yeniden oynatma karşılaştırması.
   3. Hata olursa eski ikiliye dönülür.
3. **KUBRA, koordinatör ve worker:**
   1. `SOULWARE_YONETIM_TOKEN` tüm öğrenici servislere (bilim, GitHub, learn) verilir.
   2. Canlı anahtar dosyalarının biçimi (33 bayt, sürüm baytı 1) önceden kontrol edilir.
   3. nginx koordinatöre `X-Real-IP` gönderir.
   4. Kontrollü yeniden başlatma.
4. **Ön satış izleyicisi:**
   1. `git pull`'dan **önce** `.on-satis-islenmis.json`, `.on-satis-adres-usd.json` ve `.on-satis-son-blok.json` yedeklenir.
   2. Yeni izleyici eski dosyalardan tek seferlik taşıma yapar.
   3. systemd birimindeki `STATE` değişkenleri yeni `DURUM` ile uyumlu hale getirilir.

Her aşamadan sonra: sağlık kontrolü, gerçek bir işlemle doğrulama ve gerekirse yedeğe dönüş.
