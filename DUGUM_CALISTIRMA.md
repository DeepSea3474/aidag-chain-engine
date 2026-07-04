# AIDAG-Chain — Dugum Calistirma ve Testnet'e Katilma

Bu rehber, bir AIDAG dugumunu nasil derleyip calistiracagini ve testnet'e
nasil katilacagini anlatir.

> Not: AIDAG-Chain su an gelistirme/testnet asamasindadir. Test AIDAG'in
> GERCEK DEGERI YOKTUR (gercek satis degeri/arz henuz belirlenmedi).

## 1. Derleme

Rust gerekli (https://rustup.rs). Sonra:

    git clone https://github.com/DeepSea3474/aidag-chain-kubrairem2007.git
    cd aidag-chain-kubrairem2007
    cargo build --release

## 2. Dugum modlari

Dugum uc modda calisir:

- ANA URETICI (mod yok): genesis URETIR + vertex uretir. Agda BIR tane olur.
- listen: genesis URETMEZ, aga baglanir, dinler/senkronize olur.
- produce: genesis URETMEZ, aga baglanir, vertex de uretir.

## 3. Tek dugum (yerel deneme)

    LSC_RPC_ADDR=0.0.0.0:8645 \
      ./target/release/lsc-node /ip4/0.0.0.0/tcp/40001

RPC artik http://localhost:8645 adresinde.

## 4. Testnet'e katilma (bootstrap ile)

Bilinen bir bootstrap dugumune baglanarak aga katil. Kendi genesis'ini
URETME (listen modu) — bootstrap dugumunun zincirini cekersin:

    LSC_BOOTSTRAP=/ip4/<BOOTSTRAP_IP>/tcp/40001 \
    LSC_RPC_ADDR=0.0.0.0:8645 \
      ./target/release/lsc-node /ip4/0.0.0.0/tcp/40002 listen

LSC_BOOTSTRAP virgulle ayrilmis birden cok adres alabilir:

    LSC_BOOTSTRAP=/ip4/1.2.3.4/tcp/40001,/ip4/5.6.7.8/tcp/40001

## 5. Test AIDAG alma (faucet)

Dugum calisirken, bir adrese test AIDAG iste:

    curl http://localhost:8645/faucet/<ADRES_HEX_40>

Ya da Python SDK ile:

    from aidag_sdk import AidagClient
    c = AidagClient("http://localhost:8645", network_id=1)
    c.faucet()                  # kendi adresine test AIDAG
    print(c.bakiye(c.adres().hex()))

## 6. Islem yapma

SDK ile transfer, belge dogrulama, kurum kaydi, token (Kalkan) — hepsi
sdk/python/README.md ve ornek_*.py dosyalarinda anlatiliyor.

## RPC ozeti

- GET  /health, /status, /tips
- GET  /bakiye/:adres, /lsc-bakiye/:adres, /belge/:hash, /kurum/:adres
- GET  /faucet/:adres          (testnet test AIDAG)
- GET  /tokens
- POST /submit                 (imzali vertex)

## Yeniden baslatma ve veri yukleme (KRITIK)

Calisan dugumu durdurup yeniden baslatirken DIKKAT:

1. **Data dosyasini MUTLAK yolla ver.** Aksi halde dugum kalici veriyi
   YUKLEYEMEZ (bos baslar, vertex_sayisi=0). Ornek:

       LSC_FAUCET_OWNER=<owner_hex> LSC_RPC_ADDR=0.0.0.0:8645 \
         ./target/release/lsc-node /ip4/0.0.0.0/tcp/40001 \
         /root/aidag-lsc/aidag-data-40001.log

   Sondaki ".log" arguman = explicit data dosyasi (main.rs onu yakalar).

2. **RELEASE kullan, debug DEGIL.** Cok vertex'li (10K+) reload, debug
   binary'de dakikalarca surer / pratikte takilir. release'de saniyeler.
   Once: cargo build --release

3. **Tek dugum / port cakismasi.** Ayni anda iki dugum ayni RPC portunu
   (8645) tutamaz -> "Address already in use". Yenisini baslatmadan once
   eskisini durdur: pgrep -af lsc-node ; kill <PID>

4. **Restart oncesi YEDEK.** Kritik veriyle oynamadan once:
       cp aidag-data-40001.log aidag-data-40001.log.YEDEK

> Yukleme basariliysa log: "Diskten <N> vertex yuklendi: toplam_vertex=<N>".
> Bu satiri gormuyorsan veri yuklenmemis demektir (yol/release kontrol et).

## Bilinen sinirlar

NOTLAR_BILINEN_SINIRLAR.md dosyasina bak. Ozetle: erken asama prototip;
genis olcekli public testnet henuz ayakta degil; bagimsiz audit yapilmadi.
