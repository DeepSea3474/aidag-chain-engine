# Kalkan — Kontrat Guvenlik Kapisi (TASARIM / VIZYON)

> DURUM: Bu belge bir TASARIM ve HEDEF dokumanidir; "yapildi" degil "boyle
> tasarliyoruz" demektir. AVM_KOPRU_PLANI.md'deki "5 kalkan" adiminin detayidir.
> Onkosul: AVM (akilli kontrat VM, revm/wasm). AVM olmadan kontrat incelenemez.
> Bu nedenle bu, yol haritasinin ILERI bir asamasidir.

## Neyin gercek, neyin tasarim oldugu (durustluk cercevesi)

- BUGUN GERCEK (kanitli, canli test edildi): Kalkan'in token KIMLIK korumasi.
  Stake kapisi + taklit reddi + slashing. Ayni sembolu farkli adresten
  kaydetmeye calisan reddedilir ve tum stake'i yakilir.
  (Bkz. GUVENLIK_KANITLARI.md — uc huner canli kanitlandi.)
- TASARIM / HEDEF (henuz kod degil, AVM'ye bagli): Bu belgedeki kontrat
  guvenlik kapisi — likidite kilidi, tehlikeli desen taramasi, rozet sistemi.
  Bunlar HEDEFtir; uygulanmadi.

Disari "kontrat denetimi yapiyoruz" diye sunulMAZ; "boyle TASARLIYORUZ, AVM
ustune kuracagiz" diye sunulur.

## Vizyon — neden

Kripto dunyasinda sahte token / rug pull / ici bos token bollugu insanlarin
canini yakiyor. Hedef: AIDAG-Chain'i, bu kirlilikten yorulmus ureticiler ve
yatirimcilar icin GUVENILIR ADRES yapmak. Kalkan bu guvenin TEKNIK temelidir.
Strateji: once teknoloji + guvenlik, sonra itibar, sonra cekim, sonra ekosistem.

## Mekanizma

Kalkan bir KOD/KURAL motorudur, insan/kurul DEGIL. Kurallar kontrati otomatik
denetler -> merkeziyetcilik ve hukuki sorumluluk dogmaz. Kalkan NIYET okumaz;
magduriyete yol acan BILINEN TEHLIKELI KOD DESENLERINI tespit eder.

## Iki giris yolu

1. AVM'de uretilen kontratlar (en kontrollu)
2. Disaridan getirilen token'lar (koprubasi)
Her ikisi de ayni kapidan gecer. Kacak yok.

## Teminat + rozet sistemi

Kanonik statu icin TEMINAT (stake) yatirilir. Kalkan tarar:

### KIRMIZI / RED + SLASH (teminat yanar)
Net magdur edici desenler -> giremez VE teminat yakilir:
- Likidite cekme / kilitsiz likidite (rug pull)
- Sinirsiz mint yetkisi
- Honeypot (alinir ama satilamaz)
- Gizli sahip yetkileri (durdurma, el koyma, kara liste)

### TURUNCU (girer + "dikkat" isareti, SLASH YOK)
Tehlikeli desen YOK ama iyi-pratikler eksik. Magdur edici DEGIL, sadece az
olgun. Kullanici "dikkatli ol" gorur. SLASH YOK (eksigi cezalandirmak haksizlik).

### YESIL (saglam onay)
ZORUNLU ON KOSUL: Likidite KILITLI olmali (yoksa yesil IMKANSIZ).
On-chain olculebilir kriterler (Kalkan otomatik gorur):
- Likidite kilitli mi + sure
- Likidite yuksekligi
- Sahiplik devredilmis mi (renounce / owner=sifir)
- Sahiplik dagilimi (dagitik mi)

### YESIL + YILDIZ (one cikan)
Yesil + zincirde KANITLI uzun/aktif gecmis (on-chain olculur, kandirilamaz):
- Uzun sure aktif (uzun islem gecmisi)
- Yuksek GERCEK aktivite (cok sayida gercek islem, aktif adres)
- Likidite cok uzun sure kilitli
NOT: "Aktif topluluk" sezgisi burada ON-CHAIN AKTIVITE olarak olculur —
zincir-disi topluluk (Twitter vb.) DEGIL, cunku o kandirilabilir.

## DURUST OLCUM SINIRI — "proje" ve "topluluk"

"Projeli olmak" ve "topluluk" DEGERLI ama Kalkan tek basina otomatik olcEMEZ:
- "Gercek proje" -> web/dok zincir-disi ve OZNEL.
- "Topluluk" -> zincir-disi; holder sayisi SAHTE olabilir.
Bu nedenle bunlar rozet sarti DEGIL; ayri SEFFAFLIK katmani olarak gosterilir
(web: var/yok, holder: X, denetim: var/yok) — bilgi olarak. Ileride guvenilir
oracle eklenirse skora katilabilir (gelecek).

## AVM_KOPRU_PLANI ile iliski

Sira: 1 adres -> 2 state -> 3 gas -> 4 islem -> 5 kalkan -> AUDIT.
Bu belge "5 kalkan" adiminin detayidir. Once AVM kopruleri (1-4), SONRA bu
guvenlik kapisi (5), sonra AUDIT.

## Ozet

Kalkan, AVM ustune kurulacak otomatik kod-kural kapisidir; magdur edici bilinen
desenleri reddeder + teminati yakar, guvenli ama eksik olani turuncu isaretler,
likidite kilidi dahil on-chain kosullari gecen olgun token'lara yesil, zincirde
kanitli aktif gecmisi olanlara yesil+yildiz verir — boylece AIDAG-Chain'i
gercek ve guvenli projeler icin guvenilir adres yapmayi HEDEFLER.
