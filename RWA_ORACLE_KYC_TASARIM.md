# RWA Oracle + KYC Onay Kaydi — Tasarim (1. asama: konsensus cekirdegi)

Dal: `rwa-oracle-kyc` · Durum: GELISTIRME (mainnet'te KAPALI, `RWA_MAINNET_AKTIVASYON = None`)

## Ilkeler (degismez)
- Izinli model: rapor ve KYC onayi yalniz KurumRegistry'de KAYITLI ve ROL VERILMIS kurumlardan.
- Ham veri zincire yazilmaz; yalniz hash'i (`veri_hash`, `kanit_hash`). KYC'de kisisel veri YOK.
- Kurum imzasi = vertex'in ed25519 imzasi (kaydeden = imzalayan).
- Onay/rapor yetkisi KURUMDA. Rol verme/iptal M-of-N YONETIM imzasiyla (owner tek anahtarla rol VEREMEZ).
  Owner yalniz akis tanimlar (tip=18); deger/onay YAZAMAZ, rol ALAMAZ.
- Yapay zeka (KUBRA) imza/yazma yetkisine SAHIP DEGIL: `mainnet::RWA_YASAKLI_ADRESLER`.
- Tum zaman kurallari ZINCIR SAATIYLE (vertex zamani geriye tarihlenebilir).

## Islem tipleri
| tip | ad | kim | govde |
|---|---|---|---|
| 17 | RWA_YONETIM | M-of-N imza (aktaran herkes) | nonce:8, son_gecerlilik:8, eylem:1 + govde, imza_sayisi:1, (pk:32, imza:64)*k |
| 18 | ORACLE_AKIS_TANIM | owner | akis:4, ondalik:1, M:1, sapma_bps:2, kesici_bps:2, bayat_sn:4, aciklama<=64 |
| 19 | ORACLE_RAPOR | oracle rolu aktif kurum | akis:4, tur:8, deger:i128, olcum_zamani:8, veri_hash:32 — 69 B |
| 20 | KYC_KAYIT | kyc rolu aktif kurum | adres:20, durum:1, kanit_hash:32 — 54 B |

## M-of-N yonetim (tip=17)
- Eylemler: 0=rol (kurum:20, rol:1, kapsam:4, islem:1), 1=imzaci ekle (pk:32), 2=imzaci cikar (pk:32), 3=esik (1 B).
- Baslangic 2-of-3. Imzaci ve esik degisikligi de AYNI M-of-N ile.
- Imzalanan mesaj: `"AIDAG-RWA-YONETIM-v1" || network_id(4) || nonce || son_gecerlilik || eylem`.
  Imzalar cevrimdisi uretilir; ozel anahtarlar sunucuda TUTULMAZ. Vertex'i herkes aktarabilir.
- Sayim: yalniz kumedeki, BENZERSIZ, `verify_strict` gecen imzacilar (ayni imzaci iki kez sayilmaz).
- Replay: `nonce` zincir sayacina esit olmali; yetkilendirilen islem nonce'u tuketir (eylem etkisiz olsa da).
  Ag kimligi mesajda (testnet imzasi mainnet'te gecmez); `son_gecerlilik` gecmisse red.
- Kilitleme korumasi: imzaci sayisi esigin altina dusurulemez; esik [2, imzaci sayisi]; azami 15 imzaci.
- Yonetim imzacilari kurum rolu ALAMAZ (gorevler ayrimi).
- Kurulum: mainnet `RWA_YONETIM_IMZACILARI` (bugun BOS = kurulmamis, rol islemi gecmez);
  devnet/testnet `NodeState::rwa_yonetim_kur`. Genesis'e degil baslangic durumuna yazilir.
- Durum okuma: `GET /rwa-yonetim` (imzacilar, esik, sonraki nonce, network_id).

## Yetki kurallari
- Rol yalniz KAYITLI (tip=5) kuruma verilir; oracle rolu yalniz TANIMLI akisa.
- Verilen rol `zincir_saati + RWA_ROL_BILDIRIM_SURESI` (3 gun) sonra etkin; aktif rol tekrar verilerek sure oynanamaz.
- Geri alma ANINDA etkili; kurumun KYC onaylari ve acik turdaki raporlari otomatik gecersiz.
- Owner, yonetim imzacilari ve yasakli adresler rol alamaz; imzaladiklari rapor/KYC yok sayilir.
- Akis tanimi ILK KAYIT KAZANIR (M/sapma/kesici sonradan degistirilemez).

## Oracle tur modeli (`rwa.rs`, `oracle_hesap.rs`)
- Acik tur = son kapanan + 1; kurum tura bir kez rapor verir.
- Acik turun ilk raporundan `bayat_sn` gecmisse eski raporlar dusurulur.
- Rapor >= M: alt medyan -> `|x-m|*10000 > |m|*sapma_bps` olanlar elenir -> kalan >= M ise kalanlarin alt medyani.
  Float yok, i128 + 256-bit karsilastirma; `(deger, adres)` tam siralamasi -> girdi sirasindan bagimsiz.
- Devre kesici: onceki yayina gore `kesici_bps` asilirsa akis DURUR (deger aday, yayinlanmaz).
  Durmus akis, ardisik bir tur adayi `kesici_bps` icinde dogrularsa yeniden acilir (owner mudahalesi yok).
- Okuma (`latestRoundData` karsiligi): durmus ya da `zincir_saati - guncelleme > bayat_sn` ise HATA.
- Akis basina son 1024 tur saklanir.

## KYC
- Kayit anahtari (adres, kurum): her kurum yalniz KENDI onayini verir/iptal eder.
- `isApproved(adres)` = rolu HALA AKTIF en az bir kurumun onayi var.

## Aktivasyon
- Devnet/testnet: acik. Mainnet: `zincir_saati >= RWA_MAINNET_AKTIVASYON` (bugun None = kapali).
  Aktivasyon tarihi ayri PR + mainnet replay kaniti + onay ile ayarlanir.

## RPC (salt okunur)
- `GET /oracle/:akis` — tanim, N/M, durum, acik tur, son yayin (deger STRING), zincir saati.
- `GET /kyc/:adres` — onayli + kurum bazli kayitlar.
- `GET /kurum/:adres` — mevcut yanita `roller` eklendi.

## Sonraki asamalar
- 2. asama: AVM precompile (`PrecompileProvider`), Chainlink `latestRoundData/getRoundData/decimals/description/version`,
  KYC `isApproved(address)`; `eth_call` yoluna zincir saati.
- 3. asama: kurum imzalama araci (offline), EVM (secp256k1) imzali kardes tipler.
