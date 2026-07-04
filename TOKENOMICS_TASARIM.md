# AIDAG-Chain — Tokenomics / Varlik Mimarisi (Tasarim Kararlari)

> DURUM: Bu bir TASARIM belgesidir. Asagidaki kararlar verilmistir ama
> COGU HENUZ KODLANMAMISTIR. "Su an ne var / ne hedefleniyor / ne zaman"
> her bolumde acikca belirtilmistir. (Prensip: dusunce ile gercek ortusmeli.)

## Model: Platform (karma)
AIDAG-Chain bir PLATFORM'dur: kendi resmi varliklari (native) + uzerinde
baskalarinin cikarabilecegi tokenlar (Kalkan korumali).

- **Native cekirdek varliklar (resmi, zincire gomulu):**
  - **LSC**  — yakit/gas. Her islem ucreti bununla odenir. Hedef arz: 2.1 milyar.
  - **AIDAG** — teminat/deger. Stake, Kalkan teminati. Hedef arz: 21 milyon (sabit).
- **Platform-ustu tokenlar (baskalarinin cikardigi):**
  - Kullanici/kurum tokenlari → Kalkan korumali token kayit sisteminde.
  - Kalkan: sahte-token reddi + stake-gated kayit (AIDAG teminatiyla). ZATEN VAR.

## Kararlar ve gerekceleri

### K1 — Karma model (native ciftli + platform tokenlari)
Dogru: Kalkan zaten "baskalari token cikarsin, sahteler engellensin" icin kuruldu.
Bu vizyonla birebir ortusur.

### K2 — Mevcut bakiye defteri = AIDAG; LSC ikinci defter olarak eklenecek
Gerekce: mevcut BakiyeRegistry + stake + Kalkan zaten bu defterle calisiyor.
Onu AIDAG saymak en az degisiklik, en saglam. LSC temiz ikinci defter olur.
DURUM: su an TEK defter var (AIDAG gibi davraniyor). LSC defteri HENUZ YOK.

### K3 — Arz: Genesis'te tanimli (sabit), uretimle DEGIL
Gerekce: AIDAG 21M + LSC 2.1B = sabit/sinirli arz. En saglam yol genesis'te
toplam arzi baslangicta tanimlamak (Bitcoin-vari netlik, denetlenebilir).
Uretimle olusma (madencilik) enflasyon+karmasiklik getirir; sabit-arza uymaz.
DURUM: HENUZ uygulanmadi. Su an arz siniri/genesis-dagitimi kodda YOK.

### K4 — Yakit mekanizmasi: tasarim var, kodlama AVM asamasinda
Her islemde LSC harcama = zincirin en hassas parcasi. Nonce (replay koruma,
zaten "AVM'de baglanacak" denmisti) ile BIRLIKTE uygulanmali. Yanlis yapilirsa
calisan transfer/kaliciligi bozar.
DURUM: HENUZ YOK. Islemler su an UCRETSIZ. Yakit, AVM asamasinda + nonce ile.

## Su an GERCEKTE calisan (kanitli)
- Tek varlikli bakiye defteri + transfer (cift-harcama korumali)
- Kalkan (sahte-token reddi, stake-gated kayit, slashing)
- Belge dogrulama, kurum kimligi
- Kalicilik (reboot'ta zincir geri yuklenir — 14 Haz dogrulandi)

## Henuz HEDEF (kodlanmadi)
- AIDAG/LSC iki ayri native defter
- Sabit arz (21M / 2.1B) + genesis dagitimi
- Yakit (gas) mekanizmasi
- Token SATISI / on satis — EN SON, sadece: olgun mainnet + bagimsiz denetim
  + gercek hukuki danismanlik + yasal yapi sonrasi. (Securities riski; aceleci
  on satis YAPILMAZ.)

## Sira (sequential)
1. State modeli olgunlasmasi
2. AVM asamasi: token ekonomisi (iki defter, arz) + nonce + yakit BIRLIKTE
3. Mainnet
4. Bagimsiz denetim
5. Avukat + yasal yapi
6. EN SON: token/on satis (yasal cercevede)


---

## ODUL HAVUZU — Testnet/Topluluk Dagitimi

> DURUM: TASLAK karar. Oranlar mainnet oncesi kesinlesir, baglayici degildir.
> Hesap kodlamasi HENUZ YOK (tasarim). Stake arayuzu kullaniciya HENUZ acik degil.

### Havuz
Kaynak: Genesis "Topluluk & Dagitim" = AIDAG arzinin %12'si = **2.520.000 AIDAG** (sabit).
Bu havuz testnet katilimcilarina / erken topluluga dagitilacak. Asla asilmaz.

### Kategoriler (en etkiliden en dusuk etkiliye)
| # | Kategori | Oran | Miktar (AIDAG) | Gerekce |
|---|----------|------|----------------|---------|
| 1 | Stake | %50 | 1.260.000 | En etkili: token kilitler, ag guvenligi/istikrari. Satis baskisini azaltir. |
| 2 | Testnet aktivitesi | %30 | 756.000 | Gercek kullanim: transfer, eslestirme. Spam degil, anlamli islem. |
| 3 | Referans | %20 | 504.000 | Buyume: yeni GERCEK aktif kullanici getirme (anti-Sybil sart). |
| | **Toplam** | **%100** | **2.520.000** | Kapali: asla asilmaz. |

### Kapalilik garantisi (tokenomik ACIK VERMEZ)
1. Toplam ASLA 2.520.000'i asmaz (matematiksel sinir).
2. Her kategori kendi havuzuyla sinirli (stake havuzu testnet havuzuna karismaz).
3. Kisi basi tavan: bir kisi havuzu tek basina supuremez (orn. max 25.000 AIDAG/kisi).
4. Otomatik orantisal dagitim:
   pay = (kisinin o kategorideki etkisi / kategorideki TOPLAM etki) x kategori havuzu
   -> havuz hic asilmaz; kac kisi katilirsa pay o kadar bolunur.
5. "Yoktan para" YOK: hicbir odul havuz disindan gelmez.
6. Anti-Sybil: 1 gercek adres = 1 test cuzdani (ZATEN KODLU). Sahte referans/aktivite sayilmaz.

### Onemli sinirlar (yasal/dogruluk)
- Faucet = test araci, ODUL DEGIL (bedava test AIDAG; gercek deger yok).
- Cekilis/sans oyunu YOK (yasal risk). Odul = hak edis (etki), sans degil.
- Token SATISI bu havuzda YOK; satis EN SON adim (mainnet + denetim + avukat sonrasi).
- Kullaniciya gosterilen "tahmini birikim" = anlik tahmin, GARANTI DEGIL (orantisal; katilim arttikca degisir).

### Su an ne var / ne hedef
- VAR: stake defteri (tip=3), testnet aktivitesi zincirde (transfer/eslestirme kalici), anti-Sybil eslestirme.
- HEDEF (kodlanmadi): stake kullanici arayuzu, referans sistemi, otomatik odul hesabi, tahmini birikim gostergesi.

### Dagitilmayan artan odul (havuzda kalan)
Orantisal dagitim + kisi-basi tavan sonrasi havuzda token artarsa:
- %50 -> stake edenlere ekstra odul, 12 ay vesting:
  - Ilk ay: %30 serbest
  - Ay 2-12: kalan %70, her ay ~%6.36 (orantili)
  - Formul: ilk_ay = miktar*0.30; ay 2-12 = (miktar*0.70)/11
- %50 -> sirket/proje kasasi, 3 yil kilitli (zincirde gorunur, seffaf)
- Satis/IEO havuzuyla KARISTIRILMAZ (ayri havuzlar, sabit arz).
- Ilke: hak edilmeyen artan, ya hak edene (stake) ya kilitli kasaya gider;
  hemen satilabilir token uretilmez.
- DURUM: tasarim karari. Kodlama mainnet odul dagitimiyla birlikte yapilacak.



---

## GAS MODELI (Kopru 3 — AVM)

> DURUM: TASARIM karari verildi. Kodlama Kopru 3'te (AVM gas baglama).
> Referans: Ethereum EIP-1559 (base fee yakimi) + BNB (gas burn) kanitli "kullanim->yakim->deger" modeli.

### Karar: Hibrit (yak + gelistirme)
Her islemde GAS ucreti **LSC** ile odenir (AIDAG degil; AIDAG saf deger token kalir).
Alinan LSC ikiye bolunur:
- **%50 YAKILIR (burn)** -> LSC arzi azalir; kullanim arttikca LSC degerlenir (ETH/BNB mantigi).
- **%50 GELISTIRME havuzuna** -> proje finansmani (altyapi, gelistirici, bakim). Surdurulebilir.

### Gerekce
- Yakim (%50): "kullanim = arz azalmasi = deger" bagi. En degerli zincirlerin (ETH, BNB) kanitli modeli.
- Gelistirme (%50): solo/erken asama surdurulebilirligi.
- AIDAG korunur: gas LSC'den alinir, 21M AIDAG hic yakilmaz/harcanmaz (saf deger token).
- Seffaf: yakim + gelistirme havuzu zincirde kayitli/denetlenebilir.

### Gas fiyati
- SU AN: SABIT fiyat (basit, ongorulebilir).
- MAINNET: dinamik (ag yogunluguna gore, EIP-1559 base fee tarzi) -> sonra.

### Su an ne var / ne hedef
- VAR: LSC defteri, AVM Kopru 1 (adres) + Kopru 2 (state/revm) calisiyor (6 test yesil).
- HEDEF (kodlanacak): Kopru 3 = revm gas ciktisi -> LSC kes -> %50 yak + %50 gelistirme havuzu.
