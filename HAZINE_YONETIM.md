# HAZINE YONETIM PLANI (2/3 MULTISIG)

> Hazine: 5.250.000 AIDAG (%25). Projenin ortak kasasi.
> Model: 2/3 multisig — harcama icin 3 anahtardan 2'sinin imzasi gerekir.

## Imzacilar (3 anahtar)
| Imzaci | Rol | Not |
|--------|-----|-----|
| Kurucu — Cihaz 1 | Ana (gunluk) | Telefon / MetaMask |
| Kurucu — Cihaz 2 | Ana (gunluk) | Bilgisayar / soguk cuzdan |
| Aile — Anne | Yedek | Sadece acil durumda |
| Aile — Baba | Yedek | Sadece acil durumda |

> NOT: Kurucu 2 cihaza sahip. Gunluk islerde kurucu kendi 2 cihaziyla
> tek basina 2 imza saglar (kimseye muhtac degil). Anne+Baba yalnizca
> kurucu tamamen erisemezse (acil) devreye girer.

## Kurallar
- Her hazine harcamasi: 3 anahtardan EN AZ 2 imza.
- Gunluk: Kurucu (Cihaz1 + Cihaz2) = 2 imza -> bagimsiz calisir.
- Acil (kurucu erisemez): Anne + Baba = 2 imza -> hazineye erisim korunur.
- Tek anahtar calinirsa: YETMEZ (2 imza sart) -> hazine guvende.

## Neden 2/3 (1/3 degil)
- 1/3 (herhangi biri tek imza) = hazineyi 3 ayri riske acar (her cihaz zayif halka).
- 2/3 = hem yedek (kurucu giderse aile), hem guvenlik (tek calinti yetmez).

## Kurulum zamani
- Multisig KONTRATI mainnet + audit oncesi yazilip DENETLENECEK.
- Su an (testnet): bu plan SABIT. Gercek kontrat mainnet asamasinda kurulur.
- Kontrat audit'siz canliya alinmaz (hazineyi korur, acik olmamali).

## Guvenlik notlari
- Aile anahtarlari: kriptodan anlamadiklari icin, anahtarlar SIFRELI ve
  sadece acil talimatla kullanilacak sekilde saklanmali.
- Kurucu anahtarlari: soguk saklama / kendi cuzdan (sunucuda duz tutulmaz).
- Bu belge bir PLAN'dir; hukuki/teknik uygulama mainnet oncesi netlestirilir.
