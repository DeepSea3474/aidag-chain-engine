# AIDAG-Chain — Guvenlik Kanitlari ve Dogrulama Rehberi

> AMAC: Bu belge, AIDAG-Chain cekirdek guvenlik ozelliklerinin kanitlarini ve
> bunlarin bagimsiz olarak NASIL tekrar uretilecegini aciklar. Denetim (audit)
> surecinde, her iddianin yaninda onu dogrulayan calistirilabilir bir test vardir.
>
> Ilke: "once kanit, sonra vaat." Hicbir guvenlik iddiasi, tekrar uretilebilir
> bir kanit olmadan yapilmaz.

Ortam: Testnet (canli: aidag-chain.com). Bagimsiz dis denetim HENUZ yapilmadi;
bu belge ic dogrulama kanitlarini sunar ve dis denetime hazirlik niteligindedir.

---

## 1. Replay (yeniden oynatma) korumasi

IDDIA: Ayni imzali islem (ayni nonce) ikinci kez gonderilse bile, deftere
yalnizca BIR kez yansir. Ikinci gonderim bakiyeyi/nonce'u degistirmez.

MEKANIZMA: Her gonderen adresinin bir nonce sayaci vardir (NonceRegistry).
Bir islem yalnizca nonce == beklenen ise islenir; islendikten sonra nonce ilerler.
Ayni nonce ile gelen ikinci islem dogru_mu() kontrolunden gecemez.

Birim test kaniti:
    cargo test -p lsc-engine nonce_replay_reddedilir

Canli test kaniti (testnet):
    curl -s -X POST http://localhost:8645/test_bakiye -d '{"adres":"<hex40>","miktar":1000}'
    python3 nonce_canli_test.py
Beklenen: 1. transfer (nonce=0) islenir; 2. AYNI nonce=0 ile replay -> bakiye
DEGISMEZ; nonce 1'de kalir.

---

## 2. Cift harcama (double-spend) korumasi

IDDIA: Bir adres, sahip oldugundan fazlasini harcayamaz; ayni bakiyeyi iki kez
gonderemez.

MEKANIZMA: Transfer oncesi bakiye kontrolu + nonce siralamasi. Yetersiz bakiye
veya tekrar denemesi sessizce gecersiz olur (deftere yansimaz).

Birim test kaniti:
    cargo test -p lsc-engine transfer_cift_harcama_engellenir

---

## 3. Sira-disi (out-of-order) nonce reddi

IDDIA: Nonce sirasi atlanamaz. Beklenen nonce 2 iken nonce=5 ile gelen islem
reddedilir (sirali islem zorunlulugu).

Canli test kaniti: nonce_canli_test.py adim 4 - nonce=5 ile transfer denenir,
bakiye/nonce DEGISMEZ.

---

## 4. Basarisiz islem nonce yakmaz

IDDIA: Gecersiz (orn. yetersiz bakiyeli) bir islem nonce'u ilerletmez -
yalnizca BASARILI islem nonce'u tuketir.

Birim test kaniti:
    cargo test -p lsc-engine ingest_transfer_yetersiz_nonce_ilerletmez

---

## 5. DAG (tarih) ile defter (durum) ayrimi

IDDIA: Gecersiz bir islemin vertex'i DAG'a kaydedilebilir (degismez tarih:
"bu denendi"), ANCAK deftere (bakiye/nonce) yansimaz. DAG ne oldugunu kaydeder;
defter yalnizca gecerli durumu yansitir.

KANIT: nonce_canli_test.py ciktisinda vertex_count her denemede artar
(11->12->13->14) ama replay/hatali islemlerde bakiye sabit kalir. Bu, "her vertex
kaydedilir ama yalnizca gecerli olanlar defteri degistirir" tasarimini gosterir.

---

## 6. Arz korunumu (supply preservation)

IDDIA: Transferler toplam arzi degistirmez (yalnizca yeniden dagitir).

KANIT: nonce_canli_test.py sonunda gonderen + alici = 1000 (baslangic bakiyesi).
Hicbir transfer para yaratmaz/yok etmez.

---

## 7. Cok-dugum yakinsama ve genesis tekligi

IDDIA: (a) Birden cok dugum ayni zincire yakinsar. (b) Sistem ikinci/sahte bir
genesis'i reddeder (zincir catallanmaya karsi korunur). (c) Dugumler mDNS ile
birbirini otomatik kesfeder.

Test kaniti (canli node'a dokunmadan, ayri portlarda):
    ./test-convergence-guvenli.sh
Beklenen: 7/7 GECTI. Senaryolar:
- Senaryo 1: 3 dugum tek genesis'e yakinsar.
- Senaryo 2: 2 uretici paralel; tek genesis; dugumler yakinsar.
- Senaryo 3: mDNS otomatik kesif + yabanci/ikinci genesis REDDEDILIR.

Not: Bu test ayni makinedeki canli node'u (40001) da mDNS ile kesfeder; canli
node'un farkli genesis'i DOGRU sekilde reddedilir (beklenen davranis).

---

## 8. Tum birim testler

    cargo test -p lsc-engine
Beklenen: tum testler yesil (bu yazim itibariyla 267+ test).

---

## Bilinen sinirlar (durustluk)

- Ortam TESTNET'tir; uretim/mainnet degildir.
- Bagimsiz dis guvenlik denetimi (audit) HENUZ yapilmamistir.
- test_bakiye / lsc_test_bakiye uclari yalnizca testnet icindir; gercek arz/
  dagitim modeli (token) hukuk + audit asamasindan SONRA gelir.
- Detayli sinirlar icin: NOTLAR_BILINEN_SINIRLAR.md
