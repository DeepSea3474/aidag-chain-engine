# AIDAG-Chain — Dugum Calistirma (Mainnet ve yerel deneme)

Bu rehber, bir AIDAG dugumunu nasil derleyip calistiracagini ve aga
nasil katilacagini anlatir.

> Not: AIDAG-Chain MAINNET 26 Temmuz 2026'dan beri canlidir (network/Chain ID 3474,
> genesis b82345008ae109d8). Mainnet'te AIDAG gercektir (21.000.000 sabit arz) ve
> faucet YOKTUR. Bagimsiz guvenlik denetimi henuz yapilmadi.

## 1. Derleme

Rust gerekli (https://rustup.rs). Sonra:

    git clone https://github.com/DeepSea3474/aidag-chain-engine.git
    cd aidag-chain-engine
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

## 4. Mainnet'e katilma (bootstrap ile)

Mainnet dugumu pinli genesis'i kendisi yukler (uretmez); LSC_MAINNET=1 ile
baslat ve bilinen bir mainnet dugumune baglan (listen modu):

    LSC_MAINNET=1 LSC_PRODUCTION=1 \
    LSC_BOOTSTRAP=/ip4/<BOOTSTRAP_IP>/tcp/40001 \
    LSC_RPC_ADDR=127.0.0.1:8645 \
      ./target/release/lsc-node /ip4/0.0.0.0/tcp/40002 /MUTLAK/YOL/aidag-data-mainnet.log listen

Izole deneme agi (mainnet'e karismaz): LSC_NETWORK_ID=99999 ile ayri ag kimligi ver;
ag kapisi farkli network_id'li vertex'leri reddeder.

LSC_BOOTSTRAP virgulle ayrilmis birden cok adres alabilir:

    LSC_BOOTSTRAP=/ip4/1.2.3.4/tcp/40001,/ip4/5.6.7.8/tcp/40001

## 5. Faucet (yalniz yerel/izole deneme agi)

MAINNET'te faucet ve test basim uclari KAPALIDIR (21.000.000 sabit arz); istek
hicbir sey yazmadan reddedilir. Yalniz kendi yerel/izole deneme aginda:

    curl http://localhost:8645/faucet/<ADRES_HEX_40>

Ya da Python SDK ile (deneme agi):

    from aidag_sdk import AidagClient
    c = AidagClient("http://localhost:8645", network_id=1)
    c.faucet()
    print(c.bakiye(c.adres().hex()))

## 6. Islem yapma

SDK ile transfer, belge dogrulama, kurum kaydi, token (Kalkan) — hepsi
sdk/python/README.md ve ornek_*.py dosyalarinda anlatiliyor.

## RPC ozeti

- GET  /health, /status, /tips
- GET  /bakiye/:adres, /lsc-bakiye/:adres, /belge/:hash, /kurum/:adres
- GET  /faucet/:adres          (yalniz deneme agi; mainnet'te KAPALI)
- GET  /on-satis-ozet, /on-satis-tahsis/:adres   (on satis seffafligi, TGE durumu)
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

NOTLAR_BILINEN_SINIRLAR.md dosyasina bak. Ozetle: mainnet canli ama 2 dugum ayni
sunucuda (dagitik cok-dugumlu ag henuz yok); bagimsiz audit yapilmadi.
Mainnet'e guvenli yukleme prosedurü: BAKIM-REHBERI.md bolum 7.
