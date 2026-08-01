# EVM ↔ On-satış Uyum Analizi — Doğru Yol (sistemi bozmadan)

> **Sonuç:** Custodial (senin) modelin için **motorda değişiklik GEREKMEZ.** EVM/MetaMask uyumu
> zaten kurulu ("Seçenek B" / relayer meta-tx deseni). Vertex/konsensüs katmanına dokunulmaz.

## Mevcut mimari — "Seçenek B" (relayer meta-tx)
1. MetaMask kullanıcısı **payload'ı secp256k1 ile imzalar** (EvmTransfer mesajı ya da ham eth tx).
2. RPC node dış vertex'i **kendi ed25519 anahtarıyla sarar** (`st.signing_key`, parents=`node.tips()`).
   Kullanıcının ed25519 anahtarı GEREKMEZ.
3. İşlenirken gönderen, **payload'daki secp256k1 imzasından `ecrecover`** ile bulunur (dış imzadan değil).

**Mevcut EVM tipleri:** `EVM_TRANSFER=11` (native AIDAG transferi, ecrecover gönderen → `bakiye_registry`),
`HAM_ETH_TX=12` (RLP raw eth tx → AVM), `AVM_CAGRI=9`. RPC: `eth_sendRawTransaction`.

**Kanıt:** genesis dilim 0-5 (EVM adresleri) MetaMask ile kontrol edilebilir → EvmTransfer imzalar,
AIDAG'ını gönderir. **Hazine donuk DEĞİL.** (Önceki "vertex'e secp ekle" analizi yanlıştı, iptal.)

## Custodial ön-satış — bugünkü kodla tam çalışır (0 motor değişikliği)

| Adım | Mekanizma | Durum |
|---|---|---|
| Satış | Owner tahsis kaydeder — `tip=10 ON_SATIS`, alici = alıcının **0x adresi** | ✅ var |
| Görüntüleme | Alıcı tahsis + vesting görür — `/on-satis-tahsis/:adres` (salt-okunur) | ✅ var |
| TGE release | Owner, vesting'e göre açılan AIDAG'ı 0x adrese gönderir (escrow→alıcı transfer) | ✅ var (transfer) |
| Alıcı kontrolü | Alıcı aldığı AIDAG'ı MetaMask ile yönetir — `tip=11 EvmTransfer` (ecrecover) | ✅ var |

Alıcı ödemeyi MetaMask'le yapar (BSC USDT/BNB → kurucu cüzdanı), sadece durumunu görür; owner TGE'de dağıtır.
**tip=13 on-satış claim (buyer self-claim) bu modelde KULLANILMAZ** → ed25519-only olması sorun değil.

## Tek gerçek boşluk (yalnız "trustless self-claim" istenirse)
`tip=13 ON_SATIS_CLAIM` gönderiyi dış ed25519 imzalayandan alır (`public_key_to_adres(signer)`).
Relay edilen MetaMask claim'inde bu = relayer adresi ≠ alıcı → başarısız. Yani buyer'ın KENDİ imzasıyla
self-claim yapması istenirse küçük bir ekleme gerekir:
- `ClaimTalebi`'ye `(recovery_id, imza)` + `claim_eden_adres()` (ecrecover — `EvmTransfer.gonderen_adres` ile birebir aynı desen).
- Handler: `cagiran = ecrecover_adres`, `k.alici == cagiran` kontrolü.
- **Tek tx tipi + tek handler; vertex/wire/konsensüs katmanına DOKUNMAZ.** Mevcut audit'li "Seçenek B" desenini tekrar kullanır.

## Öneri
- **Şimdi:** Custodial model → 0 motor değişikliği. Frontend'i gerçek RPC'ye bağla (tahsis görüntüleme salt-okunur) + owner backend (ödeme→tahsis kaydı, TGE→release).
- **Sonra (opsiyonel):** trustless buyer self-claim istenirse EVM-claim varyantı (küçük, izole, güvenli).
