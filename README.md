# AIDAG-Chain (LSC)

> **Turkce aciklama asagida / Turkish description below**

A distributed DAG-based blockchain, written from scratch in Rust, that blocks
counterfeit-token registration at the protocol level and is secured by a
collateral (staking) economy.

**Core idea:** identity = canonical address. A symbol can be imitated, but the
canonical address is a token's true identity. "Same symbol, different address"
is a counterfeit and is rejected at the protocol level (blocked, not merely
flagged) — the attacker's staked collateral is slashed.

**Technical basis:** Rust (lsc-engine core, lsc-net P2P), DAG ledger with
GHOSTDAG convergence, ed25519-signed transactions, blake3 hashing, libp2p
networking. EVM-compatible smart contract engine (AVM, built on revm):
standard ERC-20 tokens run (deploy + transfer proven), and contracts can be
read externally via standard eth_call JSON-RPC. 292 tests, fmt + clippy gates.

**Performance - O(n2) consensus bottleneck solved (logged, reproducible):**

| Vertices    | TPS   |
|-------------|-------|
| 1,000,000   | 3,535 |
| 5,000,000   | 3,492 |
| 10,000,000  | 3,432 |

As data grows 10x (1M->10M), TPS stays near-flat (3,535->3,432) - linear/log
scaling, NOT O(n2). The old approach collapsed to 336 TPS at 100k; the new
incremental interval scheme sustains 3,432 TPS at 10M vertices. Honest limit:
these are pure-ingest TPS (excluding network/signature/disk); real end-to-end
node TPS will be lower.

**Ethereum compatibility (proven):** Full eth_sendRawTransaction support -
raw Ethereum tx -> RLP decode -> ecrecover -> executed in AVM. Real transfer to
a MetaMask address demonstrated. Ethereum components were integrated WITHOUT
breaking the system: a u64->u128 migration (for 18-decimal EVM/wei
compatibility) was completed with all 292 tests still green.

**Seeking a strategic equity partner** to fund audit, mainnet, and listing.
Investor deck: https://aidag-chain.com/investor-deck.html

**Honest status:** Early-stage prototype. No public testnet or real users yet;
verified with local multi-node tests. Single developer; no independent security
audit yet. See the Turkish section below for full details and the roadmap.

**License:** Apache-2.0 (see LICENSE). Copyright 2026 Aydin Akyuz.
Contributions welcome — see CONTRIBUTING.md.

---

# AIDAG-Chain (LSC)

**Sahte token kaydini protokol seviyesinde engelleyen, teminat ekonomisiyle guvenceye alinmis, dagitik bir DAG blok zinciri.**

Rust ile sifirdan yazilmis; calisan, test edilmis ve birden cok dugum arasinda canli dogrulanmis bir prototip.

## Cozulen problem

Kripto kullanicilari, gercek bir token'i (or. "USDC") taklit eden sahte token'larla dolandiriliyor: ayni sembol, farkli (sahte) adres. AIDAG-Chain bunu "kimlik = kanonik adres" ilkesiyle ele alir: sembol taklit edilebilir, ama kanonik adres token'in gercek kimligidir.

## Yaklasim: Kalkan + Teminat + Ceza

1. Kalkan (anti-scam): Token'lar kanonik adresleriyle kaydedilir. "Ayni sembol, farkli adres" bir taklittir ve protokol seviyesinde reddedilir (uyari degil, engelleme; deftere hic girmez).

2. Teminat (staking): Token kaydi icin once AIDAG teminati kilitlemek gerekir. Kaydeden = islemi imzalayan (ed25519); imza kimligi kanitlar, kimse baskasinin teminatini kullanamaz.

3. Ceza (slashing): Taklit kaydetmeye kalkisan adresin tum teminati yakilir. Sahteciligin bedeli vardir; durust kullanici etkilenmez.

Ozet: teminat yatir, durust kaydet (kabul), ag dogrulasin; taklit dene, reddedil + teminatini kaybet.

## Teknik temel

- Dil: Rust (lsc-engine cekirdek, lsc-net P2P ag)
- Yapi: DAG tabanli defter, GHOSTDAG yakinsama
- Guvenlik: ed25519 imzali islemler, blake3 hash
- Ag: libp2p (mDNS kesif, gossipsub yayim, senkron)
- Dayaniklilik: kalici depolama, yeniden baslatma toparlanmasi, bozuk-veri reddi
- AVM (akilli kontrat): revm tabanli, EVM-uyumlu. Standart ERC-20 calisir
  (deploy + transfer kanitli). Sozlesmeler eth_call ile disaridan okunabilir.
- Kalite: 283 test (engine + net), fmt + clippy kapilari

## Canli kanit

Iki gercek dugum arasinda dogrulandi:
- Bir dugum teminat yatirdi + gercek token kaydetti; ikinci dugum aldi ve dogruladi.
- Bir dugum taklit token denedi; reddedildi + teminati yakildi. Sonuc her iki dugumde tutarli (token=1, stake=0).

## Durust durum / sinirlar

Erken asama bir prototiptir:
- Henuz halka acik testnet ya da gercek kullanici yoktur.
- Yerel cok-dugum testleriyle dogrulandi; internet kesfi icin bootstrap mekanizmasi (LSC_BOOTSTRAP) eklendi, genis olcekli public testnet henuz ayakta degil.
- Tek gelistirici; bagimsiz guvenlik denetimi (audit) henuz yapilmadi.
- Yalnizca isim/sembol taklidini hedefler; rug-pull, honeypot gibi turleri degil.

## Calistirma

./check.sh ile tum kalite kapilari (fmt + clippy + testler) calisir.

Ana uretici: ./target/debug/lsc-node /ip4/0.0.0.0/tcp/4001 aidag-data-A.log

Dinleyici: ./target/debug/lsc-node /ip4/0.0.0.0/tcp/4002 listen aidag-data-B.log

Testnet'e katilma (bootstrap ile, internet uzerinden): bilinen bir dugume baglan:

    LSC_BOOTSTRAP=/ip4/<BOOTSTRAP_IP>/tcp/40001 \
      ./target/debug/lsc-node /ip4/0.0.0.0/tcp/40002 listen

LSC_BOOTSTRAP virgulle ayrilmis birden cok adres alabilir. RPC adresi
LSC_RPC_ADDR ile ayarlanir (varsayilan 0.0.0.0:8645).

---

Gelistirme asamasindadir. Iddialar, depodaki calisan kod ve testlerle ortusur.

