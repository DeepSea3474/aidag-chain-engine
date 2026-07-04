# MAINNET TEKONOMIK YAPI — Baglayici Kararlar

> Bu belge, mainnet oncesi muhurlenenen baglayici tekonomik kararlardir.
> Her karar tek tek tartisilip kesinlestirilmistir. Mainnet kodlamasi bunlara uyar.
> "Geriye donup sorma yok" — burada yazili olan baglayicidir.

## SABIT ARZ (degismez)
- AIDAG: 21.000.000 (deger/teminat). Basim YOK, enflasyon YOK.
- LSC: 2.100.000.000 (yakit/gas).

---

## HAZINE (%25 AIDAG = 5.250.000) — COZULDU [Payhawk modeli]

### Kimlik
- AIDAG-Chain Global sirketinin ANA KASASIDIR (kurucu payindan AYRI).
- Sirket sahibi imza yetkisiyle yonetilir.

### Payhawk modeli — kilitli kasa disiplini
Payhawk sirket karti mantigi zincire uyarlanir:
- Hazineden token cikisi (harcama) yapilir.
- O harcamanin BELGESI (fatura/fis/sozlesme + gerekce) sisteme kaydedilmezse
  -> hazine bir sonraki harcamaya KILITLENIR.
- Belge kaydedilene kadar yeni cikis YAPILAMAZ (limit acilmaz).
- Yani: belgesiz hazine cikisi MEKANIK OLARAK IMKANSIZ.

### Kurallar
- Her harcama, en kucuk alimdan (kalem) en buyuk yatirima (sistem kurulumu) kadar
  resmi belgeye baglidir. Istisnasiz.
- Her harcama sirket giderlerine islenir (kurumsal muhasebe).
- Zincirde gerekce/referans ile seffaf gorunur.
- Amac: proje giderleri (denetim, listeleme, hukuk, gelistirme, operasyon).

### Durum
- Ilke MUHURLU. Kilit mekanizmasi mainnet hazine modulunde kodlanacak.
- Sirket (AIDAG-Chain Global) yasal kurulumu: [NETLESECEK].

---


---

## DAO UYELIK ILKESI (mainnet sonrasi) — COZULDU

### Uyelik
- On satistan ve sonrasinda AIDAG sahibi olanlar, mainnette DAO'da oy hakki kazanir.
- Token = oy/kullanim hakki. SATILAN HISSE/ONCELIK DEGIL (yasal ayrim: menkul kiymet degil).

### Oy agirligi — TAVANLI (balina korumasi)
- Oy agirligi token miktarina baglidir AMA kisi basi TAVANLIDIR.
- Kimse tek basina yonetimi ele geciremez (odul havuzundaki 25.000 tavan felsefesiyle tutarli).
- "1 uye = en az 1 oy" tabani; buyuk sahip biraz daha agirlik alir ama tavani asamaz.

### Ilke
- Adil yonetim: balina hakimiyeti YOK, kimse magdur olmaz.
- "En cok alan en cok soz sahibi" MODELI REDDEDILDI (adaletsiz + yasal risk).

### Durum
- Ilke MUHURLU. DAO mekanizmasi (oylama, uyelik, tavan) mainnet oncesi kodlanir.
- DAO baslangici: mainnet + listeleme oncesi, gercek topluluk olusunca.
