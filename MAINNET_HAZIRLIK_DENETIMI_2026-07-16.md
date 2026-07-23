# AIDAG-Chain — Mainnet Hazırlık Denetimi

**Tarih:** 2026-07-16 · **Branş:** `konsensus-determinizm`
**Kural uygulandı:** Hiçbir kaynak dosya değiştirilmedi. Her yargı bir komut çıktısına dayanır ("önce kanıt").

---

## TIER 1 — Derleme & Test

**1. `cargo build --release`** — **GEÇTİ**
`Finished release profile [optimized] in 45.37s` · warning sayısı = **0** (temiz).

**2. `cargo test --release`** — **GEÇTİ**
Toplam **326 passed / 0 failed / 22 ignored** (beklenen ~297'nin üzerinde).
Dağılım: lsc-engine 308, lsc-net 9, secp256k1/raw_tx POC 9.
`22 ignored` = yalnızca fuzz/benchmark/ölçüm + tek-seferlik `uret_mainnet_genesis` (çekirdek mantık değil — hepsi `fuzz_*`, `*_olcum`, `tps_*`, `torba_stres`, `buyuk_olcek`).

**3. `cargo clippy --release --all-targets -- -D warnings`** — **GEÇTİ**
warning/error grep = **0**, `Finished`. Sıfır uyarı.

**4. panic-risk taraması** — **GEÇTİ (kritik yollar temiz)**
Ham toplam = **542** (`unwrap()` 274 + `expect(` 226 + `panic!` 42 + `todo!` 0 + `unimplemented!` 0).
Ama **üretim ≠ test**: 542'nin ezici çoğunluğu `#[cfg(test)]` modüllerinde. Konsensus/net sıcak yolunda üretim kodu:

| Dosya | Üretim panic-risk | Not |
|---|---|---|
| node.rs | **0** | 112 örneğin tümü test modülünde (satır 1218 sonrası) |
| consensus/mod.rs, finality.rs, dag/graph.rs, dag/vertex.rs, tx.rs, registry.rs, net/store.rs | **0** | — |
| consensus/ghostdag.rs | **3** | invariant korumalı: `boyut.get_mut()` (hemen üstünde `or_insert`), `graph.get(id).expect` (savunmacı, doğrulanmış vertex), `expect("parents boş değil")` (genesis ayrı ele alınır) |
| lsc-net/lib.rs | **1** | startup-only: mainnet owner pinli değilse `expect` |
| mainnet.rs | **4** | startup-only: derleme-zamanı pinli sabit hex decode |

**Yargı:** Ağdan gelen bozuk girdiyle çalışırken node düşürebilecek **saldırıya-açık panic YOK**. Üretimdeki 8 örnek ya başlangıç-anı (pinli sabitler) ya da doğrulama-sonrası iç invariant.

---

## TIER 2 — Consensus (mavi_boncuk + ReachIndex)

**5. Blue-selection + determinizm** — **GEÇTİ**
Koşulan ve geçen testler: `ghostdag_is_deterministic_across_insertion_order`, `incremental_equals_full_across_insertion_orders`, `incremental_equals_full_with_committee_weight`, `diamond_merges_parallel_block_as_blue_with_default_k`, `diamond_with_k_zero_paints_parallel_block_red`, `higher_blue_score_tip_is_selected`, `uniform_weight_makes_blue_work_equal_blue_score`.
**Ek kanıt (opt-in fuzz, elle koşuldu — 143.63s):** `fuzz_determinizm ... ok`, `fuzz_invariant ... ok`, `fuzz_dogrula ... ok` (3 passed / 0 failed). Rastgele DAG'larda ekleme-sırasından bağımsız aynı toplam sıra.

**6. ReachIndex / interval reachability edge-case** — **GEÇTİ**
`anticone_within_ri_eski_ile_birebir`, `gapped_intervals_ayni_atalik`, `gapped_intervals_diamond`, `sp_tree_intervals_dogru_ata_kontrolu`, `diamond_sp_tree_intervals_dogru`, `inkremental_iv_atalik_dogru`, `is_ancestor_*` (4 varyant).
**Edge-case kapsamı:** tek vertex → `genesis_has_zero_score_and_no_selected_parent`; uzun zincir → `linear_chain_increments_blue_score` / `linear_chain_total_order_is_the_chain`; geniş anticone → `torba_seyrek_dag_olcumu` / `torba_stres`.

**7. Fork/reorg** — **GEÇTİ** (KANIT VAR)
Equivocation: `equivocation_fork_is_painted_red_under_small_k`, `equivocator_is_detected`.
Finality/reorg koruması: `competing_tip_not_extending_final_is_rejected`, `accepts_tip_rejects_off_spine_tip_after_finalization`, `finality_constrained_tip_selection_and_conflict_alarm`, `finality_state_is_monotone_and_idempotent` (kesinleşen blok geri alınamaz).

---

## TIER 3 — Genesis & Config

**8. Genesis deterministik / hash sabit** — **GEÇTİ**
`lsc-engine/src/mainnet.rs`: `MAINNET_GENESIS_ID_HEX = b82345…9c39`, `MAINNET_GENESIS_WIRE_HEX` pinli. ed25519 RFC8032 belirlenimci. Test `baked_genesis_tutarli`: wire decode → aynı id + imza doğrulanır + `network_id==3474` + parent'sız + zaman sabit + imzalayan==kurucu.

**9. Mainnet ≠ testnet ID** — **GEÇTİ**
`MAINNET_NETWORK_ID = 3474` (=EVM chain_id); testnet/devnet `network_id=1` (0xA1DA6 referansı). Vertex preimage'inin parçası → cross-replay yok. Mainnet **yalnız açık** `LSC_MAINNET=1` ile; varsayılan güvenli (kazayla mainnet yok) — `lsc-net/src/lib.rs:198-203`.

**10. Consensus parametreleri** — **GEÇTİ (tasarım notuyla)**
`DEFAULT_K: KType(u16) = 18` (ghostdag.rs:66), her yerde tek değer. **PoW/difficulty/blok-hedef-süresi YOK** — bu bir DAG, madencilik yok. `nonce` yalnız hesap tx-nonce'u (PoW değil). Dolayısıyla "difficulty/blok hedef süresi" tasarım gereği N/A.

---

## TIER 4 — Ağ Katmanı (lsc-net)

**11. ed25519 imza doğrulama + geçersiz reddi** — **GEÇTİ**
Koşulan/geçen: `forged_signature_rejected`, `all_zero_signature_rejected`, `tampered_payload_fails_verification`, `tampered_id_fails`, `identity_key_universal_forgery_rejected` (identity-pk evrensel sahtecilik guard'ı), `from_parts_rejects_forged_signature`, `fuzz_kalkan_gecersiz_vertex`.

**12. 2-node handshake / peer discovery** — **KISMEN**
Otomatik in-process test **YOK** (net'te yalnız `peer_id_is_generated`, `peer_id_is_deterministic_from_keypair`). Çalıştırılabilir örnek **VAR**: `test-convergence.sh` / `test-convergence-guvenli.sh` (mDNS otomatik keşif + yakınsama + çift-genesis reddi senaryoları).

---

## TIER 5 — Kalıcılık

**13. State persistence (başlat→durdur→başlat)** — **KISMEN**
Katman testleri geçer: `append_then_load_roundtrip`, `truncated_last_record_is_skipped` (çökme-güvenliği), `save_overwrites_not_appends`. Node replay-kalıcılık testleri geçer: `avm_kontrat_replay_ile_kalici`, `on_satis_replay_ile_kalici`, `artimli_esittir_tam_yeniden_hesap`. Node açılışta diskten yükler (`store::load_vertices`, net/lib.rs:406-488).
**Eksik:** tam süreç-düzeyi restart entegrasyon testi in-process değil (yalnız scriptler).

**14. Sync (yeni node DAG'ı baştan çeker)** — **KISMEN**
Mekanizma **VAR**: request-response CBOR pull-sync, offset tabanlı chunked (`SyncRequest{offset}`/`SyncResponse`, lib.rs:31-56, 792/988).
**Ama otomatik test YOK** — sync protokolünü hiçbir `#[test]` sürmüyor; yalnız runtime convergence scriptleri egzersiz ediyor.

---

## TIER 6 — Tokenomik ↔ Kod (KRİTİK)

**15. Arz/emisyon/ödül sabitleri** — **GEÇTİ (kod ↔ doküman TUTARLI)**

| Değer | Kod (genesis.rs) | Pinli doküman (MAINNET_TEKONOMIK.md) | Uyum |
|---|---|---|---|
| AIDAG arzı | `21_000_000 × 10^18` | 21.000.000, "Basim YOK, enflasyon YOK" | ✅ |
| LSC arzı | `2_100_000_000 × 10^18` | 2.100.000.000 (yakıt/gas) | ✅ |
| Blok ödülü / emisyon | **YOK** ("madencilik yok") | "Basim YOK" | ✅ |
| Dağıtım | Eko %22 / Hazine %25 / Likidite %15 / Topluluk %12 / Kurucu %13 / Erken %5 / Ön-satış %8 = %100 (kapalı, `kapali_mi`) | Hazine %25 (Payhawk) | ✅ |

Arz sabit ve genesis'te tam dağıtılıyor; `kapali_mi()` toplam == `AIDAG_ARZ` invariant'ı testlerle kilitli (`genesis_kapali_toplam_tam_arz`).

---

## ÖZET TABLO

| # | Madde | Durum | Kanıt (komut/dosya) |
|---|---|---|---|
| 1 | build --release temiz | **GEÇTİ** | `Finished in 45.37s`, warning=0 |
| 2 | test --release | **GEÇTİ** | 326 passed / 0 failed / 22 ignored |
| 3 | clippy -D warnings | **GEÇTİ** | 0 warning/error, `Finished` |
| 4 | panic-risk kritik yol | **GEÇTİ** | prod hot-path: node/tx/registry/finality/graph/vertex=0; ghostdag=3, net=1, mainnet=4 (hepsi invariant/startup) |
| 5 | blue-selection + determinizm | **GEÇTİ** | 7 default + `fuzz_determinizm`/`fuzz_invariant`/`fuzz_dogrula` ok |
| 6 | ReachIndex edge-case | **GEÇTİ** | anticone/gapped/sp_tree/is_ancestor + tek vertex/uzun zincir/geniş anticone |
| 7 | fork/reorg | **GEÇTİ** | equivocation_*_red + finality reorg-reddi testleri |
| 8 | genesis deterministik/pinli | **GEÇTİ** | mainnet.rs pinli id+wire, `baked_genesis_tutarli` |
| 9 | mainnet≠testnet ID | **GEÇTİ** | 3474 vs 1; `LSC_MAINNET=1` açık şart |
| 10 | consensus params | **GEÇTİ** | k=18; PoW/difficulty yok (DAG tasarımı) |
| 11 | ed25519 imza reddi | **GEÇTİ** | forged/all_zero/tampered/identity-forgery ok |
| 12 | 2-node handshake | **KISMEN** | otomatik test yok; convergence scriptleri var |
| 13 | persistence restart | **KISMEN** | store+replay testleri geçer; süreç-restart testi yok |
| 14 | sync (DAG çekme) | **KISMEN** | pull-sync protokolü var; otomatik test yok |
| 15 | tokenomik↔kod | **GEÇTİ** | 21M/2.1B, emisyon yok — dokümanla birebir |

---

## MAINNET BLOKERLERİ

Sert **KALDI (fail)** yok — build/test/clippy yeşil, konsensus determinizmi (fuzz dahil) kanıtlı, tokenomik dokümanla tutarlı. Blokerler **çok-node ağ/sync/restart otomatik test kapsamı** boşluklarıdır; şu an yalnız manuel scriptlerle doğrulanıyor. Önem sırasıyla:

1. **[KISMEN → #14] Sync otomatik testi yok.** Taze bir node'un bir peer'dan DAG'ı baştan çekip *aynı* state'e ulaştığını doğrulayan hiçbir `#[test]` yok. Pull-sync kodu var ama regresyon koruması manuel `test-convergence.sh`'e bağlı. Mainnet öncesi en yüksek risk — bir sync hatası sessiz zincir-ayrışması yaratır.
2. **[KISMEN → #13] Süreç-düzeyi restart entegrasyon testi yok.** Replay/store birim testleri güçlü, ama gerçek node'u başlat→durdur→başlat edip diskten aynı DAG+bakiye+finality'yi kurduğunu doğrulayan uçtan-uca test yok.
3. **[KISMEN → #12] 2-node handshake otomatik testi yok.** Peer keşfi/handshake yalnız `peer_id` birim testleri + runtime scriptlerle. mDNS LAN dışı internet-ölçeği keşif henüz yok.

### Ek notlar (bloker değil, launch-öncesi aksiyon)
- **`MAINNET_VESTING_BASLANGIC`** referans tarih (2026-07-15); mainnet.rs:40 açıkça "GERÇEK LAUNCH tarihinde güncellenip yeniden derlenir" diyor — launch günü güncelleme gerekli.
- **Ağır fuzz determinizm testleri `#[ignore]`** (opt-in). Varsayılan CI bunları koşmaz; nightly'ye bağlanması önerilir (bu denetimde elle koşuldu, geçti).
- **genesis.rs başlık yorumu** hâlâ "adresler placeholder" diyor; gerçek adresler mainnet.rs'te pinli — kozmetik tutarsızlık.

---

# GÜNCELLEME — 2026-07-23

Bu bölüm, 16 Temmuz raporundaki blokerlerin sonraki durumunu kaydeder.
Orijinal değerlendirme yukarıda değiştirilmeden duruyor.

## Bloker durumu

| # | Madde | 07-16 | 07-23 |
|---|-------|-------|-------|
| 14 | Sync otomatik testi | KISMEN | **KAPANDI** |
| 13 | Restart entegrasyon testi | KISMEN | **KAPANDI** |
| 12 | 2-node handshake | KISMEN | **KISMEN** (aşağıya bak) |

## #14 — Sync otomatik testi: KAPANDI

`lsc-engine/src/node.rs`:
- `sync_taze_node_ayni_state_e_yakinsar` — taze düğüm, dolu bir peer'dan DAG'ı
  chunked/offset sync ile (gerçek ağ döngüsünün aynı mantığı) baştan çeker.
  Doğrulanan: yalnız DAG yapısı değil, **state de birebir** yakınsıyor
  (bakiye, nonce, toplam arz).
- `sync_sirasiz_gelse_de_ayni_state_e_yakinsar` — vertex'ler ters/karışık
  sırada gelse bile orphan+cascade çözüyor, state aynı yakınsıyor.

Raporun "en yüksek risk" olarak işaretlediği sessiz zincir-ayrışması senaryosu
artık regresyon korumasında.

## #13 — Restart entegrasyon testi: KAPANDI

İki seviyede:
- `lsc-net/src/store.rs` → `restart_diskten_ayni_state_kurulur`: gerçek disk
  I/O ile append_vertex → load_vertices → replay; state birebir aynı.
- `lsc-net/tests/restart_entegrasyon.rs` → `surec_restart_diskten_ayni_state`:
  **gerçek `lsc-node` binary'si** izole ağda (LSC_NETWORK_ID=99999, port 40099,
  RPC 8699) başlar, `/submit` ile vertex alır, **süreç öldürülür**, aynı veri
  dosyasıyla yeniden başlar. Doğrulanan: genesis id + vertex sayısı + orphan
  diskten birebir kuruluyor. (`#[ignore]`'lu — gerçek süreç/port açtığı için
  elle çalıştırılır: `cargo test --test restart_entegrasyon -- --ignored`)

## #12 — 2-node handshake: KISMEN

Kapanan kısım — **ağ izolasyonu regresyonu**:
- `yabanci_network_id_her_ingest_yolunda_reddedilir`: yabancı network_id'li
  vertex üç ingest yolunda da (networked / synced / preverified) reddediliyor
  ve **orphan havuzuna girmiyor**; kendi ağından gelen aynı yapıdaki vertex
  kabul ediliyor (kapı seçici). 18 Temmuz'daki "mainnet düğümü testnet
  vertex'lerini orphan'a alıyor" hatası artık sessizce geri gelemez.
- Aynı davranış **gerçek süreçte de** doğrulandı: izole ağdaki test düğümü
  mDNS ile canlı testnet düğümünü buldu, pull-sync yaptı, 8 vertex geldi →
  `0 entegre, 0 orphan`.

Açık kalan: peer keşfi/handshake'in kendisi için otomatik test yok; kalıcı
ikinci makinede sürekli çalışan düğüm yok. (Uzak düğüm mutabakatı ve yayılım
18 Temmuz'da gerçek internet üzerinden canlı kanıtlandı, ancak tek seferlikti.)

## Yol boyunca bulunan ve düzeltilen hata

`lsc-net/src/lib.rs`: devnet genesis'i sabit `network_id=1` ile imzalanıyordu.
Varsayılan ağda görünmüyordu (zaten 1), ancak farklı bir ağ kimliğiyle açılan
düğüm **kendi genesis'ini** ağ kapısından geçiremiyor, `vertex=0` ile kalıyordu.
Genesis artık düğümün kendi ağ kimliğiyle imzalanıyor; varsayılan 1, mevcut
testnet davranışı değişmedi.

## Test durumu

330 test geçiyor / 0 başarısız (+ 1 `#[ignore]`'lu süreç-restart entegrasyon
testi, elle çalıştırılır).

## Mainnet durumu

Mainnet düğümü **bilerek durdurulmuş** durumda ve bağımsız denetim tamamlanana
kadar kapalı tutulacak. Genesis oluşturulmuştu; dışarıdan erişim olmadığı ve
hiçbir token dağıtılmadığı için karar geri döndürülebilir. Veri dosyaları
korunuyor. Testnet düğümü çalışmaya devam ediyor.

## Kalan başlıklar (kod dışı)

1. Kalıcı ikinci düğüm — ayrı makinede sürekli çalışan node
2. DIŞ bağımsız güvenlik denetimi (token / RWA / AVM kapsamı)
3. Custody sertleştirme
4. Hukuki yapı
