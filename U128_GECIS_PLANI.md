# u128 (Tutar) Gecis Plani — Taze Kafayla Buradan Basla

## DURUM (son commit: 0342861, 278 test yesil)
Bugun EVM uyumu kuruldu (secp256k1, 0x adres, ecrecover, EVM transfer node'da,
eth_ RPC canli: chainId/getBalance/getTransactionCount, raw tx cozme POC 5).
u128 gecisine BASLANDI ama OLCULDU ki tek oturumluk degil -> temizce geri alindi.
SISTEM SU AN TEMIZ VE CALISIYOR (278 yesil). Buradan taze kafayla devam.

## KARAR: Neden u128 (U256 degil)?
- Hedef: BSC/Ethereum uyumu = 18 ondalik (gosterim kurali; EVM tam sayi tutar).
- 21M AIDAG x 10^18 = 2.1x10^25 -> u64'e SIGMAZ (u64 max ~1.8x10^19).
- u128 max ~3.4x10^38 -> bolca siger. U256 gereksiz fazla (Rust'ta kutuphane/struct).
- Tutar soyutlamasi sayesinde gerekirse ileride `type Tutar = U256` TEK SATIR.

## YONTEM: Tutar tip soyutlamasi (Aydin fikri)
1. registry.rs'e `pub type Tutar = u128;` (TEK yerden yonetim).
2. SADECE saf para alanlari u64 -> Tutar.
3. SAYAC/ZAMAN/REF u64 KALIR: nonce, timestamp, odeme_ref.
4. Derleyici sinir noktalarini gosterir -> bilinçli çöz.

## PARA ALANLARI (Tutar olacak) — registry.rs
stakelar/stake_ekle/stake_miktari/toplam_stake/slash ;
bakiyeler/bakiye/test_bakiye_ekle/transfer(miktar)/toplam_arz ;
TransferSonuc(gonderen_yeni_bakiye/mevcut/istenen) ; OnSatisKaydi(aidag/lsc_hediye).
DIKKAT: kaydet'te odeme_ref + zaman u64 KALIR (sayac/zaman).

## TEL FORMATI (serilestirme) — KARAR: A (tam tutarli, 8->16 bayt, mainnet yok risksiz)
tx.rs encode/decode PARA alanlari: StakeKaydi.miktar, TransferKaydi.miktar,
LscTransferKaydi.miktar, EvmTransfer.miktar, OnSatis. Her birinde:
struct tip + ENCODED_LEN sabiti (8->16) + encode + decode (u64::from_be_bytes -> u128) + new.

## YAYILIM SIRASI (her grupta: YEDEK + derle + 278 test)
1. type Tutar = u128 (test edildi: tanim derleme temiz)
2. STAKE grubu (registry + node + slash + StakeKaydi tel)
3. BAKIYE grubu (registry + TransferSonuc + TransferKaydi/Lsc tel + node tip=4/7/11 + EvmTransfer)
4. ON SATIS grubu
5. DIS KATMAN (node pub fn'ler, rpc.rs, sdk)
6. 278 testin para kismi + YENI buyuk-sayi testi

## BASARI OLCUTLERI
- [ ] cargo build temiz
- [ ] 278/278 test yesil (CEKIRDEK KORUNDU)
- [ ] YENI: 21_000_000*10^18 tasmadan islenir
- [ ] canli node saglam (eth_getBalance)
- [ ] Tutar tek satirdan yonetiliyor

## TUZAKLAR
- Python: count==1 assert; heredoc << 'PYEOF' (tirnakli); ! ve \n bozar.
- Her grup ayri; hepsini-birden DEGIL (cekirdek bozma riski en yuksek).
- OZGUNLUK: u128 = uyumluluk, ozgunluk DEGIL. Ozgunluk = GHOSTDAG+Kalkan+registry (cekirdek).
  Fantom modeli: DAG cekirdek + EVM kapi; "klon" denmez cunku cekirdek ozgun.
- chain id 3474 (0xd92) mainnet'te chainlist.org'da rezerve edilmeli.

## ========== TAMAMLANDI (2026-07-01) ==========
u128 (Tutar) gecisi BITTI. 5 basari olcutunun HEPSI saglandi:
- [x] cargo build temiz (motor + net + release)
- [x] 279/279 test yesil (278 + 1 yeni kanit; CEKIRDEK KORUNDU)
- [x] 21M*10^18 tasmadan islenir (bakiye_18_ondalik_21m_arz_tasmaz testi)
- [x] canli node saglam (eth_chainId=0xd92, eth_getBalance calisiyor)
- [x] Tutar tek satirdan yonetiliyor (U256 kapisi acik)

Commit'ler:
- b46615c STAKE grubu -> Tutar
- 1a5a550 BAKIYE + transfer + AVM grubu -> Tutar (revm koprusu U256->u128)
- 98b5538 21M*10^18 kanit testi
- c4e68d1 Python SDK tel formati -> u128

Uygulanan ilke: "para genisler (Tutar/u128), teknik sayac kalir (u64)".
u64 KALAN alanlar: nonce, timestamp/zaman, odeme_ref, gas (teknik birim).
Tel formatinda para 16 bayt, sayac 8 bayt. Sinirlarda bilincli 'as' donusum.

Not: Bu gecis ozgunluk degil UYUMLULUK saglar (BSC/ETH 18 ondalik).
Ozgunluk = GHOSTDAG + Kalkan + registry (cekirdek, dokunulmadi).
Gerekirse gelecekte `type Tutar = U256` tek satir (kapisi acik).
