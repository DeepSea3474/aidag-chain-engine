# AVM Yetki + Tasarim Felsefesi (TASLAK)

> Durum: TASARIM/VIZYON. Kodlanmadi. Gercek kurumsal anlasma olunca uygulanacak.
> Bu belge bir PLAN; "yapilmis is" degil. Kanitlanmis olanlar en altta.

## KIMLIK CUMLESI
**"Karmasiklik bizde, kolaylik kullanicida."**
Arka planda zor olsun; kullanici icin cok kolay olsun. Cunku sistemi onlar kullanacak.
> NOT: Basitlik = arka planda DAHA COK muhendislik (karmasikligi yok etmezsin,
> kullanicidan gizlersin). Hedef dogru ama emek ister; zamanla, adim adim insa edilir.

## BENZETME (SoulwareAI)
- Normal kullanici -> MAVI, normal mod. Kurucu sifreyle -> TURUNCU, kurucu modu.
- AVM'de ayni mantik: kimlige/yetkiye gore farkli mod.

## IKI KATMAN

### 1. KURUMSAL KATMAN (izinli) — devlet kurumlari + sirketler
- Yetkili, KAYITLI adresler (KurumRegistry Devlet/Ozel ayrimi temel).
- Sorumluluk tasiyan ciddi taraflar -> "ozel" muamele.
- Gas: SABIT (ongorulebilir, kurumsal sozlesmeye uygun).
- Is/kazanc/baglantilar: KURUCU + EKIP ile DOGRUDAN muhatap.
  > Bu ticari iliski ZINCIR DISINDA olur (sirket / fatura / hukuki sozlesme).
  > Zincir sadece teknik isi yapar (belge damgasi, kurum kaydi). Kazanc zincire
  > KODLANMAZ; is modeline aittir. (Kurumsal entegrasyon Aydin/ekip tarafinda,
  > yasal anlasmalarla.)

### 2. ACIK KATMAN (izinsiz) — diger herkes
- ETH / BNB / Solana gibi: herkese acik, izin yok, gas ode-calistir.
- Gas: PIYASA / DINAMIK (kripto standardi).
- Koruma: gas ucreti (spam pahali) + teknik sinirlar (KALKAN: gas limiti, sandbox).

## YAPAY ZEKA KATMANI (kolaylastirici + uyarici)
Rol: zoru kolay gostermek + yanlisa "dur" demek. KARAR VERMEZ, GARANTI VERMEZ.
- Kolaylastirici: kullanici normal dilde konusur ("diplomami dogrulat"); AI
  arkadaki teknik isi (hash, sozlesme cagrisi, gas) cevirir, kullanici gormez.
- Uyarici ("dur" de): riskli/yanlis islemde uyarir.
  ornek: "bu adrese ilk kez ve yuksek miktar gonderiyorsunuz, emin misiniz?"
  ornek: "bu klasik bir dolandiricilik sablonuna benziyor, dikkat."
- KESIN SINIRLAR (SoulwareAI ilkeleriyle ayni):
  * AI GARANTI VERMEZ ("paran guvende" DEMEZ — yalan olur, kripto risklidir).
  * AI kullanici YERINE islem yapmaz/imzalamaz. SON ONAY her zaman kullanicida.
  * AI yetki vermez, islem onaylamaz, konsensusu/oylamayi kontrol etmez.
  * AI ONERIR/UYARIR/ACIKLAR; kullanici karar verir.

## ACIK SORU — yetki kaynagi (gercek anlasmada cozulecek)
> "X adresi yetkili devlet kurumu/sirkettir" bilgisini ZINCIRE KIM yazar?
> - Kurucu tek basina yazarsa -> merkeziyet riski ("neden sana guvenelim?").
> - Cozum: gercek kurumsal/hukuki cerceve olunca netlesir. Bu yuzden yetki
>   katmaninin KODLANMASI bilincli olarak ERTELENDI.

## SIRA (kodlama)
1. ONCE: gercek kurumsal anlasma (devlet/sirket ortagi).
2. O zaman: yetki kaynagi netlesir -> KurumRegistry yetki katmani kodlanir.
3. Sabit-gas (kurumsal) vs dinamik-gas (acik) ayrimi kodlanir.
4. AI kolaylastirici/uyarici katman (son onay hep kullanicida).
5. Bagimsiz audit.

## BUGUN KANITLANMIS OLAN (temel — bunlarin uzerine insa edilecek)
- AVM gercek Solidity sozlesmesi calistiriyor (BelgeDamgasi: deploy / kaydet /
  dogrula / cift-kayit reddi). Motor seviyesinde kanitli.
- DatabaseCommit: EVM state degisiklikleri deftere kalici yaziliyor.
- KALKAN gas korumasi: sonsuz dongu gas limitiyle guvenle durduruluyor.
- 261 test yesil, sifir regresyon.
> Bu model (iki katman + AI + basitlik) bu temelin UZERINE, gercek anlasmada insa edilecek.
