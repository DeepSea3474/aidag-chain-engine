# AIDAG-Chain — Audit Öncesi İç Denetim Bulguları (2026-07-15)

> 4 odaklı denetçi + kod doğrulaması. Önem sırasına göre. Durum: ✅ düzeltildi ·
> 🔧 düzeltilecek · 📋 tasarım/doküman · ✓ doğrulandı-sağlam.

---

## ✅ REMEDIATION KAYDI (2026-07-16) — dış audit için

**14/16 bulgu TAM kapatıldı; her kod düzeltmesi, eski hatayı GÖSTEREN kanıt testiyle geldi.**
Test tabanı 323/323 workspace yeşil; CI tam yeşil (fmt + clippy + test).
**Kalan 2 madde:** **A1** KISMİ (owner adresi pinlendi; hazine BAKİYESİ pinlenmesi
mainnet dağıtımı = tokenomik kararı bekler), **A3** süreç/doküman (kod bug'ı değil).
DÜRÜSTLÜK NOTU: A1/A2/A5 önce commit mesajlarından "kapalı" sanılmıştı; kod
doğrulamasında owner'ın node-yerel (env) olduğu görüldü ve A2/A5 GERÇEKTEN `8cb9f55`
ile kapatıldı. A1'in bakiye ayağı hâlâ açık (aşağıda).

| Bulgu | Önem | Durum | Commit | Kanıt testi |
|-------|------|-------|--------|-------------|
| **A1** owner hazine env (konsensüs böl.) | KRİTİK | 🔧 KISMİ | `8cb9f55` | owner ADRESİ pinlendi (A2); hazine BAKİYESİ hâlâ env → dağıtım pinlenmeli (tokenomik) |
| **A2** owner-gating env | YÜKSEK | ✅ | `8cb9f55` | `a2_mainnet_owner_pinli_deterministik` (env override mainnet'te kapalı) |
| **A3** çift-dağıtım odeme_ref | ORTA | 📋 | — | süreç/doküman (aşağıda); aynı-ref zaten det. engelli |
| **A4** on-satış şeffaflık | ORTA | ✅ | `1f898d6` | `on_satis_lsc_hediye_yetersizse_kayit_gercegi_yansitir_a4` |
| **A5** geçersiz env owner | DÜŞÜK | ✅ | `8cb9f55` | mainnet owner pinli + env yok; ayrıca mainnet faucet MINT kapalı (`a2_mainnet_faucet_mint_yapmaz`) |
| **B1** fon donması (AIDAG state-diff) | KRİTİK | ✅ | `d08f715` | `avm_kontrat_ici_transfer_gercek_deftere_yansir_b1` |
| **B2** gas + DoS (gerçek gas_used) | KRİTİK | ✅ | `a127db8`+`c1d90de` | deploy/call gas + `ham_eth_kontrat_ici_transfer...b1_b2` |
| **B3** deploy nonce kalıcılığı | YÜKSEK | ✅ | `8fe032f` | `avm_ayni_hesap_iki_kontrat_deploy_edebilir_b3` |
| **B4** eksik bakiye yükleme | YÜKSEK | ✅ | `d08f715` | B1 tam-seed ile kapandı |
| **B5** hayalet tx (DAG-dışı) | YÜKSEK | ✅ | `c1d90de` | ölü `eth_ham_tx_isle` kaldırıldı |
| **B6** birleşik nonce vs EVM per-account | ORTA | ✅ | `ce4b49f` | `b6_create_nonce_birlesik_nonce_ile_tutarli` |
| **B7** U256→u128 + code_by_hash + ölü dal | DÜŞÜK | ✅ | `efb38e8` | debug_assert + O(1) index + ölü synced dalı silindi |
| **C1** RWA custodian yetkisi | YÜKSEK | ✅ | `fbeeb9a` | `kubr_teminat_yetkisi_custodian_c1` |
| **C2** BelgeDamgasi front-running | ORTA/YÜK | ✅ | `de3096f` | `belge_damgasi_front_running_onlenir_c2` |
| **C3** KUBR teminat matematik | ORTA | ✅ | `911e322` | `kubr_teminat_orani_truncation_yok_c3` |
| **C4** KUBR hardening | DÜŞÜK | ✅ | `48b59c1` | `kubr_hardening_zero_address_ownership_allowance_c4` |

Ek: CI fmt kapısı yeşile alındı (`ae60d64`, repo-geneli `cargo fmt`).

### A3 — odeme_ref türetme (süreç/doküman)
**Kod durumu (sağlam):** `on_satis_registry` aynı `odeme_ref`'i iki kez işlemez
(deterministik çift-dağıtım engeli, `node.rs` tip=10). Owner yanlışlıkla aynı ödemeyi
iki kez gönderse bile ikinci dağıtım YAPILMAZ.
**Süreç kuralı (bağlayıcı):** `odeme_ref`, ilgili ödemenin **değişmez kimliğinden**
türetilmelidir — örn. `odeme_ref = u64(keccak(banka_referansı ‖ tutar ‖ alıcı)[..8])`.
Böylece aynı fiziksel ödeme her zaman AYNI ref'i üretir; owner farklı ref uydurup
çift dağıtım yapamaz. Bu kural on-satış operasyon prosedürüne + owner aracına işlenir
(zincir-dışı ödeme onayı adımı). Kod tarafı ek değişiklik gerektirmez.

### A1 — KALAN: hazine bakiyesi pinleme (MAINNET-BLOCKER, tokenomik bekler)
**Kapanan (A2 ile):** owner ADRESİ artık `new_mainnet()`'te pinli (`8cb9f55`); tüm
mainnet node'ları aynı owner'ı kullanır. Mainnet faucet (mint) kapalı → 21M korunur.
**Açık:** owner'ın **hazine BAKİYESİ** hâlâ node-yerel env (`LSC_GENESIS_HAZINE`,
`lib.rs`) ile besleniyor. İki mainnet node farklı env ile farklı owner bakiyesi alırsa,
on-satış transferi birinde başarılı diğerinde yetersiz → ayrışma. **Tam çözüm:** mainnet
genesis DAĞITIMI (`genesis.rs`'teki 6 dilim — ekosistem/hazine/likidite/topluluk/kurucu/
erken) **kodda PİNLİ** olmalı (env değil) ve `new_mainnet()` bunu genesis'te yüklemeli.
**Bloke eden:** 6 dilimin GERÇEK adresleri + tutarları = kullanıcının nihai tokenomik
kararı (şu an placeholder, `genesis.rs` satır 5). Bu karar verilince A1 tam kapanır +
mainnet güvenle canlıya alınabilir. **Bu netleşene kadar mainnet BAŞLATILMAMALI.**

---

## A. ÖN-SATIŞ (on_satis)

**A1 — KRİTİK (konsensüs bölünmesi):** Owner hazine bakiyesi konsensüs dışı; node-yerel
env (`LSC_GENESIS_HAZINE`) ile besleniyor. On-satış AIDAG transferi owner bakiyesine
bağlı → env farklı node'larda farklı sonuç → alıcı bakiyesi + on_satis_registry kalıcı
ayrışır. `lsc-net/lib.rs:289-294` + `node.rs:961`. Fix: owner hazinesi genesis'te pinli.

**A2 — YÜKSEK (owner-gating konsensüs dışı):** `faucet_owner` genesis'te pinli değil;
`new_mainnet()` bile `None`. Env yoksa her node kendi adresini owner yapar →
garantili bölünme. `node.rs:122` + `lib.rs:264-283`. Fix: mainnet owner genesis'te pinli,
env override kapalı.

**A3 — ORTA (çift-dağıtım):** Benzersizlik yalnız owner'ın seçtiği `odeme_ref`'e bağlı.
Aynı ödeme farklı ref ile 2 kez → çift AIDAG. Aynı ref deterministik engelli (sağlam).
Fix: odeme_ref değişmez ödeme kimliğinden türetilsin (süreç/doküman).

**A4 — ORTA (şeffaflık):** `node.rs:973-987` LSC hediye transferi sonucu yok sayılıp
(`let _ =`) kayıt tam `lsc_hediye` saklıyor; LSC yetersizse gitmez ama "gönderildi"
görünür. Fix: transfer sonucunu kontrol et, gerçek tutarı sakla.

**A5 — DÜŞÜK:** Geçersiz `LSC_FAUCET_OWNER` → owner None + hazine sıfır adrese sessizce.
`lib.rs:212-221` vs `269-283`. Fix: hard-fail.

✓ Sağlam: imza kapısı (ed25519 verify_strict, sahte kayıt sokulamaz), RPC salt-okuma,
aynı-ref dedup deterministik, replay/restart deterministik.

---

## B. AVM (revm/EVM)

**B1 — KRİTİK (fon donması/kaybı):** Kontrat-içi native (AIDAG) değer hareketleri gerçek
deftere (`bakiye_registry`) yansımıyor; yalnız üst-seviye `gonderen→hedef` yansıtılıyor
(`node.rs:924-927`). avm_db her replay'de sıfırlanır. Payable/withdraw kontratları fonu
dondurur. Fix: EVM state-diff'i tam yansıt VEYA bakiye_registry'yi avm_db'den türet.

**B2 — KRİTİK (ekonomik + DoS):** Gerçek `gas_used` (avm.rs:292) yok sayılıyor; sabit
21000 kesiliyor (`node.rs:854,1056`). Kullanıcı gas_limit'i yok sayılıyor. Başarısız tx
HİÇ gas kesmiyor (`node.rs:930, r.basarili`). 3M-gas çağrı = 21k ücret + O(n) tam-replay
→ DoS. Fix: gas_used'dan ücret, gas_limit uygula, başarısız tx'te de gas kes.

**B3 — YÜKSEK (deploy bozuk):** CREATE adres çakışması — hesap nonce'u kalıcılaşmıyor
(`basic()` nonce=0, `commit()` nonce yazmıyor, avm_db reset). Bir hesap pratikte tek
kontrat deploy edebilir. `avm.rs:135-140,174-199`. Fix: account nonce'u avm_db'de kalıcı.

**B4 — YÜKSEK (yanlış yürütme):** Yürütme öncesi eksik bakiye yükleme; AVM_CAGRI yalnız
gonderen'i yükler (`node.rs:908`), HAM_ETH ikisini (`1064-1067`) → tutarsız. Kontratın
gördüğü `address(this).balance` yanlış. Fix: B1 ile birlikte tek-kaynak.

**B5 — YÜKSEK (hayalet tx):** `eth_ham_tx_isle` (`node.rs:275-283`) DAG dışı, kalıcı
olmayan, nonce/gas denetimsiz doğrudan avm_db mutasyonu; sonraki ingest'te silinir.
Fix: kaldır veya tip=12 vertex üretecek şekilde yeniden yaz.

**B6 — ORTA:** Birleşik nonce sayacı EVM per-account nonce semantiğiyle çakışıyor
(tip 4/7/9/11/12 paylaşımlı). MetaMask karışık kullanımda ayrışır.

**B7 — DÜŞÜK/şüpheli:** `avm.rs:184` `balance.try_into().unwrap_or(u128::MAX)` — U256>u128
sessiz MAX → para yaratma (normalde ulaşılamaz). code_by_hash lineer tarama (perf). Ölü
`else if synced` AVM replay dalı (eskimiş yorum).

✓ Sağlam: blok env deterministik (timestamp=vertex), state hash'i yok (HashMap sırası
konsensusu etkilemez), SystemTime state'te yok. Infallible DB eksik hesapta 0 (doğru).

---

## C. RWA / Solidity Kontratları

**C1 — YÜKSEK (RWA güven):** KUBR `collateralMg` sadece owner'ın yazdığı serbest sayı;
gerçek altınla on-chain bağ yok. `custodian` ölü değişken. Doküman "saklayıcı günceller"
diyor ama kod `onlyOwner`. Fix: yetkiyi custodian'a bağla + belgele (oracle/attestation).

**C2 — ORTA/YÜKSEK (front-running):** BelgeDamgasi `kaydet` ilk-gelen-kazanır + hash
public → saldırgan mempool'da görüp sahipliği gasp eder. Fix: (hash,sender) namespace.

**C3 — ORTA (matematik hatası):** KUBR teminat oranı truncation — `(totalSupply/1e18)*mgPerToken`
(satır 44, 83) kesirli token'ları yutar → az-teminat "tam teminatlı" görünür.
Fix: `(totalSupply*mgPerToken)/1e18` (ve tavana yuvarla).

**C4 — DÜŞÜK:** KUBR ownership devri yok; approve-race (KUBR+Token); zero-address kontrol
yok; BelgeDamgasi sıfır-hash. Token.sol test fixture (dokunma → compat testi kırılır).

---

## Öncelik (birleşik)
1. **Konsensüs bölünmesi:** A1, A2 (mainnet blocker — genesis pinleme ile çözülür).
2. **Fon güvenliği:** B1, B2, B3, B7.
3. **Doğruluk/şeffaflık:** A4, B4, B5, C3.
4. **Tasarım:** A3, B6, C1, C2, C4.
