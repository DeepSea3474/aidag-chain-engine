# RWA Oracle + KYC Onay Kaydi — Tasarim (1. asama: konsensus cekirdegi)

Dal: `rwa-oracle-kyc` · Durum: GELISTIRME (mainnet'te KAPALI, `RWA_MAINNET_AKTIVASYON = None`)

## Ilkeler (degismez)
- Izinli model: rapor ve KYC onayi yalniz KurumRegistry'de KAYITLI ve ROL VERILMIS kurumlardan.
- Ham veri zincire yazilmaz; yalniz hash'i (`veri_hash`, `kanit_hash`). KYC'de kisisel veri YOK.
- Kurum imzasi = vertex'in ed25519 imzasi (kaydeden = imzalayan).
- Onay/rapor yetkisi KURUMDA. Rol verme/iptal M-of-N YONETIM imzasiyla (owner tek anahtarla rol VEREMEZ).
  Owner yalniz akis tanimlar (tip=18); deger/onay YAZAMAZ, rol ALAMAZ.
- Yapay zeka (KUBRA) imza/yazma yetkisine SAHIP DEGIL: `mainnet::RWA_YASAKLI_ADRESLER` =
  [`KUBRA_IMZA_ADRESI` 0x1f4b6bc66533f80f76c0823d5553b1456653d747] (servis gunlugundeki acik adres).
- Tum zaman kurallari ZINCIR SAATIYLE (vertex zamani geriye tarihlenebilir).

## Islem tipleri
| tip | ad | kim | govde |
|---|---|---|---|
| 17 | RWA_YONETIM | M-of-N imza (aktaran herkes) | nonce:8, son_gecerlilik:8, eylem:1 + govde, imza_sayisi:1, (pk:32, imza:64)*k |
| 18 | ORACLE_AKIS_TANIM | owner | akis:4, ondalik:1, M:1, sapma_bps:2, kesici_bps:2, bayat_sn:4, aciklama<=64 |
| 19 | ORACLE_RAPOR | oracle rolu aktif kurum | akis:4, tur:8, deger:i128, olcum_zamani:8, veri_hash:32 — 69 B |
| 20 | KYC_KAYIT | kyc rolu aktif kurum | adres:20, durum:1, kanit_hash:32 — 54 B |

## M-of-N yonetim (tip=17)
- Eylemler: 0=rol (kurum:20, rol:1, kapsam:4, islem:1), 1=imzaci ekle (pk:32), 2=imzaci cikar (pk:32), 3=esik (1 B),
  4=kurum dogrula (kurum:20, dogrulanmis:1).
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
- OLCUM ZAMANI PENCERESI: `olcum_zamani > zincir_saati` (ileri tarihli) ya da
  `olcum_zamani + bayat_sn < zincir_saati` (pencereden eski) olan rapor REDDEDILIR.
  NOT: `olcum_zamani` kurumun kendi beyanidir. Bu kontrol yalan soyleyen kurumu DEGIL,
  gecikmeli aktarimi (bayat olcumun taze gibi islenmesini) yakalar. Asil koruma
  M-of-N rol yonetimi + medyan/asiri sapma elemesidir.
- Okuma (`latestRoundData` karsiligi): durmus ya da `zincir_saati - guncelleme > bayat_sn` ise HATA.
- Akis basina son 1024 tur saklanir.

## Kurum dogrulama (belge dogrulama icin, geriye uyumlu)
- `dogrulanmis` bayragi YALNIZ M-of-N yonetimle (tip=17 eylem 4) verilir/geri alinir; yalniz kayitli kuruma.
- Yalniz GOSTERIM bilgisidir: mevcut belge (tip=1) ve kurum (tip=5) kayitlari silinmez, reddedilmez,
  degismez; dogrulanmamis kurumun yeni belgesi de kabul edilir. Hic islem gormemis kurum = dogrulanmamis.
- RPC (yalniz EK alanlar, mevcut alanlar aynen):
  `/kurum/:adres` -> `dogrulanmis`, `dogrulama_durumu` ("dogrulanmis kurum" / "dogrulanmamis kurum" /
  "kurum kaydi yok"), `dogrulama_zamani`; `/belge/:hash` -> `kaydeden_kurum`, `kurum_durumu`.
- Gosterilen durum SU ANKI durumdur (belge anindaki degil).
- Mainnet: RWA kapali + yonetim kurulmamis -> etkisiz. Replay (3850 vertex, 3736 belge, 0 kurum) main ile birebir.

## KYC
- Kayit anahtari (adres, kurum): her kurum yalniz KENDI onayini verir/iptal eder.
- `isApproved(adres)` = rolu HALA AKTIF en az bir kurumun onayi var.

## KUBRA anahtar rotasyonu
- `RWA_YASAKLI_ADRESLER` KUBRA'nin ADRESINI pinler (`KUBRA_IMZA_ADRESI`). KUBRA imza anahtari
  degisirse (rotasyon, kayip, sizinti) liste MUTLAKA guncellenmelidir; aksi halde yeni KUBRA
  adresi yasak listesinde olmaz (M-of-N rol vermedikce yazamaz, ama ikinci savunma katmani kaybolur).
- Yeni adres EKLENIR; eski adres listeden CIKARILMAZ (konsensus sabiti: gecmisin replay'i + ele
  gecmis eski anahtar riski). Liste degisikligi konsensus degisikligidir (PR + replay + esgudumlu deploy).
- Yeni anahtar, yeni adres listeye girip deploy edilmeden imza atmaya BASLAMAMALI
  (soulware-core anahtar dosyasi yoksa ya da bicimi gecersizse acilista yeni anahtar uretip
  dosyanin uzerine yazar -> servis her acilista loglanan "imzalayan" adresi listeyle karsilastirilmali).
- Adim adim prosedur: `BAKIM-REHBERI.md` bolum 8 "KUBRA IMZA ANAHTARI ROTASYONU".

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
