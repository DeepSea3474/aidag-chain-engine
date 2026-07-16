# AIDAG-Chain — Genesis Dağıtım Adresleri (PİNLİ)

> **Onay:** 2026-07-16 · Toplam **21.000.000 AIDAG** (sabit, kapalı sistem) · 7 dilim.
> Bu adresler mainnet genesis'ine **pinlendi** — genesis'te sabit, sonradan değiştirilemez.

## Ağ bilgileri
| Alan | Değer |
|---|---|
| Ağ adı | AİDAG Chain |
| Chain ID | `3474` |
| RPC | `https://aidag-chain.com/rpc/` |
| Explorer | `https://aidag-chain.com/scan` |
| Sembol | `AİDAG` (değer) · gas: `LSC` |

## Dağıtım tablosu

| # | Dilim | Oran | AIDAG | Adres | Vesting |
|---|-------|------|-------|-------|---------|
| 0 | Ekosistem | %22 | 4.620.000 | `0xcfbb3E5A398B9C43E10770d19Ff5EA6F027aEb3B` | 12 ay doğrusal |
| 1 | Hazine | %25 | 5.250.000 | `0x7458936B578aD2346934a9729a86d3F516d59E24` | Açık (Payhawk) |
| 2 | Likidite | %15 | 3.150.000 | `0x1d3F315aA99C8c298a0a5F39EEf6115BB1FC0924` | 2 yıl doğrusal |
| 3 | Topluluk | %12 | 2.520.000 | `0xc98182Ec9C5ED46F84879DbDd6E2D0979773e1bB` | 6 ay doğrusal |
| 4 | Kurucu | %13 | 2.730.000 | `0x57241fb83E0Ee8624399A9Ad0f4ccf2B1dE4e716` | 6 ay cliff + 2 yıl |
| 5 | Erken destekçi | %5 | 1.050.000 | `0x8fA6bF5Dd4125433a749af10DF4A1791043a72Cd` | 6 ay cliff + 2 yıl |
| 6 | Ön-satış (escrow) | %8 | 1.680.000 | `0x11c1906e07508e0b83ef4afa042879281e196b9f` | dağıtımda alıcıya |
| | **TOPLAM** | **%100** | **21.000.000** | | |

## Notlar
- **On-satış (slice 6) = kurucu native anahtarı** (`11c1906e`, `aidag-kurucu.key`). Operasyon/escrow: ön-satış AIDAG'ı buradan alıcılara dağıtılır. On-satış vertex'ini bu anahtar imzalar.
- **On-satış vesting:** her alıcıya dağıtımda %20 TGE + kalan %80 12 ay (dağıtım anında uygulanır, birikmeli).
- **Slice 0-5:** kullanıcı MetaMask/EVM cüzdanları (adres = anahtardan türer, ağdan bağımsız).
- **Vesting başlangıcı (TGE):** `MAINNET_VESTING_BASLANGIC` — gerçek lansman tarihinde güncellenmeli.
- **Custody uyarısı:** hazine/ekosistem/kurucu büyük dilimler; anahtarları offline + yedekli (ideali donanım cüzdanı) tut.

## Kaynak
Kodda pinli: `lsc-engine/src/mainnet.rs` (`MAINNET_DAGITIM_ADRES_HEX`) + `new_mainnet()` otomatik yükler. Kanıt testli, deterministik (tüm mainnet node'ları aynı dağıtımı alır).
