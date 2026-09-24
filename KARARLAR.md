# AIDAG-Chain — Kararlar

Konsensüsü, tokenomiği ya da yatırımcı haklarını etkileyen kararların kaydı. Her karar:
ne değişti, neden, kanıtı ve açık kalan iş.

---

## K-2026-09-24-01 — Genesis vesting başlangıcı "TGE henüz belirlenmedi" tarihine (2100) alındı

| | |
|---|---|
| Tarih | 2026-09-24 |
| Durum | **Taslak — proje sahibinin onayı bekleniyor.** Commit ve yayın onaydan sonra. |
| Kod | `lsc-engine/src/mainnet.rs`: `MAINNET_VESTING_BASLANGIC = TGE_BELIRSIZ` (4_102_444_800 = 2100-01-01 00:00 UTC) |
| Aynı pakette | K-04 düzeltmesi (vesting kilidinin AVM üzerinden aşılması) |
| Son tarih | Tüm mainnet düğümlerinde **2026-09-27 00:00 UTC'den önce** yayında olmalı |

### Tarih geçmişi
| Zaman | Olay | Değer |
|---|---|---|
| Mainnet açılışı (2026-07-26) | Genesis vesting başlangıcı koda sabit | `1_790_467_200` = 2026-09-27 00:00 UTC. Yanındaki yorum hatalı olarak "2026-08-26" diyordu. |
| 2026-07-26 13:31 UTC | tip=15 (kurucu adresi `0x11c1…6b9f`): ön satış TGE'si | 2027-01-01 00:00 UTC |
| 2026-09-22 19:11 UTC | tip=15 (kurucu adresi `0x11c1…6b9f`): ön satış TGE'si ertelendi | 2100-01-01 (`TGE_BELIRSIZ`) |
| 2026-09-24 | Bu karar: genesis vesting başlangıcı da ertelendi | 2100-01-01 (`TGE_BELIRSIZ`) |

Kaynak: mainnet veri dosyasının salt okunur kopyası üzerinde yeniden oynatma (denetim raporu §13).

### Neden: ekip ile yatırımcı arasında simetri
Ön satış claim'i 22 Eylül'de "belirsiz"e ertelendi, ama genesis dilimlerinin kilidi koda sabit ayrı bir tarihe bağlıydı. Değişiklik yapılmasaydı 27 Eylül'den itibaren ekosistem (%22, 12 ay), topluluk (%12, 6 ay) ve likidite (%15, 2 yıl) dilimleri saniye saniye açılmaya başlayacaktı; ön satış alıcıları ise hiçbir şey claim edemeyecekti. Karar, iki tarafı aynı "TGE henüz belirlenmedi" tarihine bağlar. Gerçek tarih ortaklık kurulunca belirlenecek.

### Etkisi
- Vestingli genesis dilimleri (0 ekosistem, 2 likidite, 3 topluluk, 4 kurucu, 5 erken destekçi) 2100'e kadar **tamamen kilitli** kalır.
- **Değişmeyenler:** hazine (dilim 1, %25) ve ön satış emaneti (dilim 6, %8, kurucu adresi) tasarım gereği kilitsizdir, bu karar onları etkilemez (arzın %33'ü).
- Ön satış TGE'si zincirde ayarlanmamış olsaydı kullanılacak geri dönüş değeri de 2100 olur.
- **Geçmiş durum değişmez:** 27 Eylül'den önce iki takvimde de her şey kilitli olduğundan, mainnet geçmişinin yeniden oynatılması eski ve yeni kodla birebir aynı özeti verir.
- **Genesis kimliği değişmez** (`b82345008ae109d8…`); vesting tarihi genesis vertex'inin parçası değildir.

### Kanıtlar
- Testler (`lsc-engine/src/denetim_testleri.rs`):
  - `eski_tarihten_2100e_kadar_hicbir_genesis_dilimi_acilmaz`: 27 Eylül 2026'dan 31 Aralık 2099'a 8 anda, vestingli her dilimin tamamı kilitli.
  - `eski_tarihten_sonra_kilitli_dilim_ne_transferle_ne_avm_ile_tasinamaz`: 28 Eylül 2026'da gerçek sabit ve kurucu takvimiyle ne tip=4 ne tip=9 kilitli miktarı taşıyabiliyor.
  - `vesting_baslangici_tge_belirsiz_ile_ayni`, `genesis_id_degismedi`.
  - Negatif kontrol: aynı testler değiştirilmemiş kodda (986320a) başarısız oluyor.
- Mainnet replay: orijinal ve yeni kodla özetler birebir aynı.

### Riskler ve sınırlar
- Tarih hâlâ **koda sabit**. Gerçek tarihi ayarlamak yeni ikili ve tüm düğümlerin eşzamanlı güncellenmesini (hard fork) gerektirir; bu, kararı tek bir geliştiricinin elinde bırakır ve geçici bir çözümdür.
- 27 Eylül 00:00 UTC'den sonra eski sürümde kalan bir düğüm dilimleri açık sayar; yeni sürümdeki düğümlerle durumu ayrışır. Bu yüzden yayın son tarihten önce tamamlanmalı, sonrasında eski sürüme geri dönülmemeli.
- K-04 düzeltmesi yayınlanana kadar kilitler AVM yoluyla aşılabilir; bu yüzden aynı pakettedir.

### Sonraki iş: tarihi zincir üstü, M-of-N imzalı, bir kez ayarlanabilen işleme bağlamak
Hedef: TGE tarihini ne tek bir anahtar ne de bir kod değişikliği belirlesin.
1. Yeni işlem türü (örn. `TGE_KESINLESTIR`), şu kurallarla:
   - **M-of-N imza:** rwa dalındaki yönetişim altyapısı (tip=17) yeniden kullanılabilir. Mesajda alan etiketi, network_id, nonce ve son geçerlilik olmalı; aynı imzacı iki kez sayılmamalı; eşik en az 2 olmalı.
   - **Bir kez ayarlanabilir:** tarih ayarlandıktan sonra ikinci işlem reddedilir.
   - **Geçmişe yazılamaz:** tarih ≥ zincir saati + bildirim süresi (3 günden uzun, örn. 30 gün). Vertex zaman damgası değil zincir saati kullanılmalı; vertex zamanı geriye tarihlenebilir (denetim Y-06).
   - **Tek tarih, iki taraf:** aynı işlem hem ön satış TGE'sini hem genesis vesting başlangıcını birlikte ayarlar; ikisi bir daha ayrışamaz.
2. Etkinleştikten sonra mainnet'te tek anahtarlı tip=15 devre dışı kalır.
3. Etkinleştirme, zincir sırasına bağlı bir aktivasyon noktasıyla ve tüm düğümlerin eşzamanlı yükseltmesiyle yapılır (hard fork); öncesinde mainnet replay kontrolü.
4. Testler: çift ayar reddi, eşik altı imza reddi, geçmiş/yakın tarih reddi, replay ve farklı imzacı kümeleri.
