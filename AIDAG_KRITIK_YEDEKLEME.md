# AIDAG-Chain — Kritik Yedekleme Dokümanı

> **2026-07-16** · Ayrı, güvenli bir yerde sakla (offline + şifreli + birden fazla kopya).
> ⚠️ **Bu dokümanda GİZLİ ANAHTAR/SEED YOKTUR** (bilerek). Gizli olanları aşağıdaki
> **checklist**'e göre SEN ayrıca yedekle — onların değerini asla bir dosyaya/mesaja yazma.

---

## 🔴 BÖLÜM 1 — GİZLİ (bunları SEN yedekle, burada değiller)

Bunlar kaybolursa/sızarsa **fon gider, geri gelmez.** Her birini **offline + şifreli + en az 2 kopya** (ideali donanım cüzdanı / kağıt yedek kasada) sakla:

- [ ] **`aidag-kurucu.key`** — EN KRİTİK. Kurucu ed25519 anahtarı. Şunları kontrol eder:
  - Genesis'i imzalar (güven kökü)
  - On-satış operatörü (satışı imzalar)
  - On-satış escrow'u (`11c1906e`, 1.68M AIDAG) buradan dağıtılır
  - **Bu dosyanın kendisini** güvenli yedekle (içeriğini kimseye gösterme, dosyaya kopyalama).
- [ ] **MetaMask seed phrase(leri)** — 6 dağıtım cüzdanını kontrol eder:
  - ekosistem (4.62M), hazine (5.25M), likidite (3.15M), topluluk (2.52M), kurucu (2.73M), erken-destekçi (1.05M)
  - Büyükleri (hazine/ekosistem/kurucu) **ayrı seed / donanım** cüzdanında tut — tek seed = tek nokta riski.
- [ ] **USDT ödeme cüzdanı anahtarı** — ön-satış gelirinin (para) geldiği cüzdan (aşağıda; ayrı, güvenli).

> Kural: gizli anahtar/seed **hiçbir zaman** dijital olarak (mesaj, e-posta, ekran görüntüsü, bulut) durmasın. Kağıt/metal yedek + kasa.

---

## 🟢 BÖLÜM 2 — PUBLIC REFERANS (kaybetme, ama sır değil)

### Ağ
| Alan | Değer |
|---|---|
| Ağ adı | AİDAG Chain |
| Chain ID | `3474` (0xD92) |
| RPC | `https://aidag-chain.com/rpc/` |
| Explorer | `https://aidag-chain.com/scan` |
| Sembol | `AİDAG` (değer) · gas: `LSC` |

### Kurucu / Genesis (güven kökü — public)
| Alan | Değer |
|---|---|
| Kurucu pubkey | `cece417af631d437df7adfe7afca45b4745b9958ee446df3062c1c008c2e1c73` |
| Kurucu adres | `11c1906e07508e0b83ef4afa042879281e196b9f` |
| Genesis vertex id | `b82345008ae109d842beefa4004a8680fc6f545fefa2c87a6a218de0f1269c39` |
| Vesting başlangıç (TGE) | `1784073600` (2026-07-15) — *gerçek lansmanda güncellenecek* |

### Genesis dağıtım adresleri (21.000.000 AIDAG, pinli)
| # | Dilim | Oran | AIDAG | Adres |
|---|-------|------|-------|-------|
| 0 | Ekosistem | %22 | 4.620.000 | `0xcfbb3E5A398B9C43E10770d19Ff5EA6F027aEb3B` |
| 1 | Hazine | %25 | 5.250.000 | `0x7458936B578aD2346934a9729a86d3F516d59E24` |
| 2 | Likidite | %15 | 3.150.000 | `0x1d3F315aA99C8c298a0a5F39EEf6115BB1FC0924` |
| 3 | Topluluk | %12 | 2.520.000 | `0xc98182Ec9C5ED46F84879DbDd6E2D0979773e1bB` |
| 4 | Kurucu | %13 | 2.730.000 | `0x57241fb83E0Ee8624399A9Ad0f4ccf2B1dE4e716` |
| 5 | Erken destekçi | %5 | 1.050.000 | `0x8fA6bF5Dd4125433a749af10DF4A1791043a72Cd` |
| 6 | Ön-satış (escrow) | %8 | 1.680.000 | `0x11c1906e07508e0b83ef4afa042879281e196b9f` (kurucu native) |

### USDT ödeme cüzdanı (ön-satış geliri)
| Alan | Değer |
|---|---|
| USDT cüzdan adresi | `0xE5b0a02E8821103e6274cAA8B107b9E480B2A80A` (ayrı, EVM) |
| Zincir/standart | **BEP-20 (BSC)** — USDT `0x55d398326f99059fF775485246999027B3197955` |
> Bu cüzdan, alıcıların ödediği **USDT'yi** toplar (AIDAG dağıtımıyla ayrı, 6 dilimden ayrı).
> ⚠️ Sitede kabul edilen AĞ net yazılmalı; alıcı yanlış ağdan gönderirse para kaybolur.
> Güvenli tut (donanım/çoklu-imza ideali — gerçek para toplar).

---

## Notlar
- Genesis dağıtımı kodda pinli: `lsc-engine/src/mainnet.rs` (`MAINNET_DAGITIM_ADRES_HEX`).
- Detaylı tokenomik/vesting: `AIDAG_GENESIS_DAGITIM_ADRESLERI.md` + `TOKENOMIK_ON_SATIS_PROGRAMI.html`.
- Audit remediation: `AUDIT_BULGULARI_2026-07-15.md` + `AUDIT_REMEDIATION_RAPORU.html`.
