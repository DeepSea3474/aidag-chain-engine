# AIDAG Python SDK

AIDAG-Chain'e Python'dan baglanmak icin minimal kutuphane. ed25519 imza,
blake3 vertex id, wire kodlama — hepsi lsc-engine ile birebir uyumlu.

## Kurulum
    pip install blake3 pynacl requests

## Hizli baslangic
Once bir dugum calistir (RPC acik):
    LSC_RPC_ADDR=0.0.0.0:8645 ./target/debug/lsc-node /ip4/0.0.0.0/tcp/40001

Sonra Python'dan:
    from aidag_sdk import AidagClient
    c = AidagClient("http://localhost:8645", network_id=1)
    print(c.status())     # zincir durumu
    print(c.tokens())     # kayitli tokenlar (Kalkan)
    print(c.adres().hex()) # bu istemcinin adresi

## 1) Token gonderme (KALKAN: once stake et)
Kalkan kurali: token kaydetmek icin once stake etmelisin (sahte-token korumasi).
Tam ornek: ornek_kalkan.py

## 2) Deger transferi (odeme)
Bir adresten digerine AIDAG gonder. GONDEREN = imzalayan (baskasi adina
gonderilemez). Cift harcama engellenir (bakiye yetmezse reddedilir).

    import time
    adres = c.adres()
    c.test_bakiye_ekle(adres.hex(), 1000)        # devnet: test bakiyesi
    alici = bytes.fromhex("ee"*20)
    tips = [bytes.fromhex(t) for t in c.tips()["tips"]]
    payload = c.transfer_payload(alici, 300)     # 300 AIDAG -> alici
    wire = c.vertex_olustur(tips, payload, int(time.time()))
    c.submit(wire)
    print(c.bakiye(adres.hex()))                 # gonderen bakiyesi (700)

## 3) Belge dogrulama (gercek dunya)
Bir belgenin parmak izini (hash) zincire yaz; sonra "bu belge gercek mi,
degismedi mi, kim ne zaman kaydetti" diye dogrula. Belgenin KENDISI zincire
GIRMEZ (sadece hash). Belge bir harf bile degisirse hash degisir -> sahtecilik
yakalanir.

    import time, hashlib
    belge = open("sozlesme.pdf","rb").read()
    h = hashlib.blake2b(belge, digest_size=32).digest()
    tips = [bytes.fromhex(t) for t in c.tips()["tips"]]
    wire = c.vertex_olustur(tips, c.record_payload(h), int(time.time()))
    c.submit(wire)                               # belgeyi zincire kaydet
    print(c.belge_dogrula(h.hex()))              # kayitli mi? kim? ne zaman?

Kurumsal akis: kurum kendi anahtarini uretir (kimligi = adresi), belge hash'ini
imzalayip zincire yazar, belgeyi karsi tarafa gonderir; karsi taraf hash'i alip
/belge ile dogrular -> belgenin o kurumdan geldigi ve degismedigi kanitlanir.

## 4) Kurum/firma kimligi
Bir kurum/firma kendi kimligini zincire kaydeder (adres -> ad + kategori).
Kategori: devlet kurumu ya da ozel firma (KESIN ayrilir, karismaz). Belge
dogrulama ile birlesince: "bu belge su KURUMDAN geldi" kanitlanabilir.

    from aidag_sdk import KURUM_DEVLET, KURUM_OZEL
    import time
    tips = [bytes.fromhex(t) for t in c.tips()["tips"]]
    # Devlet kurumu olarak kaydol (ya da KURUM_OZEL ile ozel firma)
    payload = c.kurum_payload(KURUM_DEVLET, "Tapu Mudurlugu")
    wire = c.vertex_olustur(tips, payload, int(time.time()))
    c.submit(wire)
    print(c.kurum_sorgula(c.adres().hex()))   # ad, kategori, zaman

Not: Resmi devlet kurumlari icin onay/yetki katmani, ilgili kurumlarla anlasma
yapildiginda API ile baglanir. Su an altyapi kayda HAZIR.

## RPC endpoint'leri
- GET  /health        — dugum canli mi
- GET  /status        — zincir durumu (vertex/token/stake/bakiye sayilari)
- GET  /tokens        — kayitli kanonik tokenlar (Kalkan)
- GET  /tips          — DAG uclari (vertex parent'i icin)
- GET  /bakiye/:adres — bir adresin AIDAG bakiyesi
- GET  /belge/:hash   — bir belge hash'i kayitli mi (kim, ne zaman)
- GET  /kurum/:adres  — bir adres hangi kurum/firma (ad, kategori)
- POST /submit        — imzali vertex gonder (hex govde)
- POST /test_bakiye   — DEVNET: test bakiyesi basla (gercek arz degil)

## Islem tipleri (payload)
- tip=1 Record    : [1][hash:32]            — belge/veri dogrulama
- tip=2 Token     : [2][adres:20][sembol:8] — Kalkan token kaydi
- tip=3 Stake     : [3][adres:20][miktar:8] — teminat (Kalkan kapisi)
- tip=4 Transfer  : [4][alici:20][miktar:8] — deger transferi
- tip=5 Kurum     : [5][kategori:1][ad]     — kurum/firma kimligi (0=devlet,1=ozel)
