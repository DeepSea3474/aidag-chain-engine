# AVM Köprü Planı (revm → AIDAG-Chain)

> revm cekirdegi eklendi, EVM islem calisti (kanitlandi).
> Simdi: revm'i sisteme baglayan kopruler — AVM'nin "bize ait" katmani.
> Her adim derlenip TEST edilecek. Mevcut tip 1-7 HIC bozulmayacak.

## Kopruler (SIRALI)
1. ADRES: ed25519 adres (20 bayt) <-> revm Address (20 bayt). UYUMLU. En kolay.
2. STATE/DATABASE: revm Database trait'ini AIDAG/LSC defterleriyle uygula. EN ZOR.
3. GAS: EVM gas <-> LSC. Gelir EKOSISTEME (kisisel degil).
4. ISLEM: yeni tip=8 (AVM cagrisi). Nonce burada baglanir. Tip 1-7 korunur.
5. KALKAN: AVM guvenligi (teminat/yetki + revm sandbox/gas). Sonra AUDIT.

## Sira: 1 adres -> 2 state -> 3 gas -> 4 islem -> 5 kalkan -> AUDIT

## Prensipler
- Sifirdan VM YAZILMADI: revm cekirdek, kopruler bizim. Ozgunluk butunde.
- Mevcut sistem (tip 1-7) bozulmaz; 245+ test hep gecmeli.
- Gas geliri ekosisteme (saygin durus).
- Olgunlasinca BAGIMSIZ AUDIT (token/mainnet oncesi).


## Kalkan (adim 5) detayli tasarim
Bkz: KALKAN_GUVENLIK_KAPISI_TASARIM.md — kontrat guvenlik kapisi (rozet sistemi,
likidite kilidi zorunlu, tehlikeli desen->slash). TASARIM/HEDEF; AVM ustune kurulacak.
