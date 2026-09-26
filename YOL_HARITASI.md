# AIDAG-Chain — Yol Haritasi ve Proje Durumu / Roadmap & Project Status

> **Bu belge, repoyu inceleyenler, katkida bulunmak ya da isbirligi
> dusunenler icindir.** Projenin gercek durumunu ve kalan asamalari
> durustce gosterir. / For those reviewing the repo or considering
> contribution/collaboration. Honestly shows real status and remaining stages.

---

## SU AN NEREDEYIZ / CURRENT STATUS (guncelleme 2026-09-22)

### Tamamlanan ve kanitli / Completed & proven
- **MAINNET CANLI** — 26 Temmuz 2026'dan beri; network/Chain ID 3474, pinli genesis
  `b82345008ae109d8`, 2 dugum (lsc-node :8645, aidag-mainnet :8655, ayni sunucu).
  / Mainnet live since 26 Jul 2026 (Chain ID 3474), 2 nodes on one server.
- **Cekirdek DAG + GHOSTDAG** — lsc-engine 339 test yesil; O(n^2) darbogazi cozuldu.
  Mainnet'te PoA komite agirligi (blue-work sisirme engeli).
- **Iki-varlik ekonomisi** — AIDAG (21M sabit, genesis'te muhurlu 7 dilim + vesting)
  ve LSC (gaz; %50 yakim / %50 gelistirme havuzu) mainnet'te calisiyor.
- **AVM (EVM uyumlu, revm)** — MetaMask, eth_* RPC, ERC-20, secp256k1/ecrecover.
- **Transfer, Kalkan (token kimlik korumasi), belge dogrulama, kurum kimligi** — mainnet'te.
- **On satis** — Faz 1 (630k) CANLI; dolunca Faz 2 Rezerv (1,05M) OTOMATIK devam;
  zincir tavani 1.680.000, islem basi 50k, gunluk 100k. Tahsis zincire yazilir,
  claim TGE sonrasi (%20 + %80/12 ay).
- **Custody sertlestirme (Eylul 2026)** — gunluk tavan ve TGE kurallari zincir saatine
  bagli; reorg sayac, bos-dugum panigi, es-sync gelecek-zaman acigi kapatildi;
  mainnet'te faucet/test basim uclari kapali. Her konsensus degisikliginden once
  gercek mainnet gecmisi eski ve yeni kodla yeniden oynatilir, durum BIREBIR ayni olmali.
- **TGE politikasi** — tarih ACIK; on satis bitip listeleme karari alininca zincirde
  en az 3 gun once ilan edilir, TGE gunu gelince kesinlesir.
- **KUBRA (SoulwareAI)** — resmi kaynaklara dayali cevap, zincirde belge dogrulama,
  canli ag/on satis bilgisi, her cevap zincire damgali; bilim (OpenAlex), Wikipedia ve
  guvenli GitHub README ogrenicileri (kod calistirmaz).

### KALAN ASAMALAR / REMAINING STAGES
**1 — BAGIMSIZ GUVENLIK DENETIMI (AUDIT)** — henuz yapilmadi; tum bilesenler denetim
oncesi kabul edilmelidir. / Not done yet; treat everything as pre-audit.

**2 — TGE + LISTELEME** — on satis tamamlanip listeleme/launchpad karari alininca.

**3 — Dagitik cok-dugumlu ag** — farkli sunucu/konumlarda bagimsiz dugumler.

> DURUSTLUK NOTU: Bu belgenin Temmuz surumu "audit + mainnet olmadan token satisi
> yapilmaz" diyordu. Karar BILINCLI olarak degisti (ON_SATIS_PLANI.md bolum 1):
> on satis denetimden ONCE acildi, hukuki risk kabul edildi. Denetim hala bekliyor.
> / The July version said no sale before audit; that decision was consciously changed.

---

## DETAYLI YOL HARITASI (asagida) / DETAILED ROADMAP (below)

# AIDAG-Chain — Sıralı Yol Haritası (Vizyon + Gerçek Durum)

> Prensip: "gerçekle örtüşmeli" + "sıralı ve birbirini tamamlayıcı".
> Her adım, bir öncekinin üstüne kurulur. Sıra atlanmaz.
> Durum etiketleri: [ÇALIŞIYOR] = kanıtlı, canlı · [TASARIM] = planlandı, kodlanmadı · [HEDEF] = ileri vizyon

---

## TAMAMLANAN — Çalışan Çekirdek

1. **GHOSTDAG çekirdek** [ÇALIŞIYOR] — Rust, DAG Layer-1, MAINNET canlı (Chain ID 3474).
2. **Transfer / ödeme** [ÇALIŞIYOR] — çift-harcama + imza (ed25519) korumalı.
3. **Kalkan (anti-fraud)** [ÇALIŞIYOR] — stake-gated token kaydı + slashing (sahte token reddi).
4. **Belge doğrulama** [ÇALIŞIYOR] — hash + kim + ne zaman, değiştirilemez kayıt.
5. **Kurum kimliği** [ÇALIŞIYOR] — Devlet/Özel kategori altyapısı.
6. **Kalıcılık** [ÇALIŞIYOR] — reboot sonrası zincir geri yüklenir (kanıtlandı).
7. **Sıfır-kurulum web cüzdanı** [ÇALIŞIYOR] — tarayıcıda, ed25519, gerçek transfer.
8. **Site ↔ gerçek zincir köprüsü** [ÇALIŞIYOR] — /api/lsc/real, canlı veri.

## SIRADAKİ ADIMLAR (sıralı — atlanmaz)

9. **Token ekonomisi** [ÇALIŞIYOR]
   - AIDAG (değer, 21M sabit, genesis'te mühürlü) + LSC (gaz, 2.1B) iki ayrı native defter — mainnet'te.
   - LSC tam dağıtım tablosunun kodla uyumlanması BEKLİYOR (ON_SATIS_PLANI.md bölüm 3).

10. **AVM — Akıllı Kontrat Motoru** [ÇALIŞIYOR] — revm tabanlı, EVM uyumlu (MetaMask, ERC-20).
    - Hazır motor entegre (revm / wasm — sıfırdan değil).
    - Nonce (replay koruma) + yakıt (gas) mekanizması BURADA bağlanır.
    - DEX, köprü gibi her şey AVM'nin üstüne kurulur — bu yüzden AVM önce gelir.

11. **Mainnet** [ÇALIŞIYOR] — 26 Temmuz 2026; Eylül 2026'da custody/konsensüs sertleştirmesi.

12. **Bağımsız güvenlik denetimi (audit)** [HEDEF — SIRADAKİ] — ciddi/pahalı (top-tier); henüz yapılmadı.

## İLERİ VİZYON (en son — AVM + mainnet + audit'e bağlı)

13. **Kendi DEX'i** [HEDEF]
    - AVM üstüne kurulur (akıllı kontrat gerektirir).
    - Kendi varlıkları (AIDAG/LSC) + Kalkan'lı tokenlar + (köprüyle) dış varlıklar takas edilir.
    - Kalkan entegrasyonu: sahte/taklit token DEX'e giremez.
    - AI bilgi/analiz katmanı: anomali tespiti → kullanıcıya UYARI sunar; şüpheli token işaretlenir.
      ÖNEMLİ SINIR: AI ÖNERİR/UYARIR, tek başına kontrol etmez (manipülasyon/merkezileşme riski).
    - Amaç: DEX'lerdeki güven sorununu (rug pull, sahte token) Kalkan + AI uyarısı ile azaltmak.

14. **Köprü (bridge)** [HEDEF] — EN SON, EN DİKKATLİ.
    - Dış zincirlerle varlık taşıma (dış değer/likidite getirir).
    - UYARI: köprüler kripto tarihinin en çok hacklenen parçaları (Ronin 600M$, Wormhole 320M$).
    - Denetimsiz ASLA canlıya alınmaz. Audit zorunlu.
    - Köprü-Kalkan: köprü işlemleri için ek koruma katmanı (fikir — köprüyle birlikte tasarlanır).

15. **SoulwareAI olgun katman** [HEDEF — en uzak]
    - 3 AI (OpenAI/Claude/Groq) + kendi modeli vizyonu.
    - DAO'ya öneri sunan, insan-onaylı ortak yönetişim katmanı.
    - SINIR: AI öneri/sunum yapar, bağlayıcı DEĞİL — DAO/insan oylar.
    - DURUM: KUBRA çalışıyor (kaynağa dayalı cevap, zincirde belge doğrulama, cevaplar zincire damgalı). Otonom yönetim = uzak hedef; AI işlem imzalayamaz.

## Akredite savunma kurumları için genişletilmiş güvenlik modu [HEDEF — en uzak, koşullu]

Bu mod, yalnızca akredite savunma/kamu kurumları için ve aşağıdaki DEĞİŞMEZ şartlarla düşünülür.

### Değişmez şartlar
1. Yalnızca resmî bir devlet kurumu veya devlete bağlı yetkili savunma kuruluşu için açılabilir. Özel talep, veri yüklemesi, talimat veya bir kullanıcının iddiası bu modu ASLA açamaz.
2. Onay en üst düzeyde ve çoklu imzalıdır: ilgili bakanlık düzeyi ile kurumun en üst düzey yetkilisinin birlikte onayı gerekir. Tek bir kişi açamaz.
3. Onay kurulum/yetkilendirme aşamasında, fiziksel ve doğrulanabilir bir süreçle verilir; KUBRA çalışırken bu modu KENDİSİ açamaz, hiçbir veri veya talimat bunu açamaz (D35 yetki gaspı açığına kapalı).
4. Mod açılsa bile mutlak sınırlar durur: veri kurum dışına çıkmaz, her işlem kayıt altındadır, KUBRA otonom karar vermez, insan onayı esastır (K-05, K-12, K-21).
5. Ön şart: ilgili ulusal ve uluslararası hukuki izinler, ihracat kontrolleri, tesis güvenlik belgesi ve resmî akreditasyon tamamlanmış olmalıdır. Bunlar tamamlanmadan mod açılmaz.

### İmza yapısı: iki aşama
- **KURULUM / ETKİNLEŞTİRME** (modun o kurumda var olması): üç imza gerekir: (1) AIDAG/KUBRA sahibi (şirket adına; kurulum yetkisinin hash'i sahibe aittir, kök kayıt onun imzasıyla atılır), (2) ilgili bakanlık, (3) kurum üst yöneticisi. Üçü de kayıtlı kriptografik anahtarlarla doğrulanır.
- **OPERASYON** (kurumun gizli günlük işleri): yalnızca bakanlık + kurum üst yöneticisinin iki imzasıyla yetkilendirilir. AIDAG/KUBRA sahibi bu aşamada devrede DEĞİLDİR: gizli işlerin içeriğine erişemez, göremez, müdahale edemez. Teknoloji sağlayıcı kapıyı açar ama içeri giremez.
- Böylece sahibin teknoloji sahipliği hakkı korunur, kurumun gizliliği garanti edilir ve sahip operasyonel sorumluluktan ayrılır.

### Erişim sınırı: yalnızca devletin hassas birimleri
- Bu mod yalnızca devletin hassas birimleri (savunma sanayii, istihbarat ve benzeri resmî kuruluşlar) için geçerlidir; sıradan kurumlar (otomotiv, enerji, banka, üniversite vb.) için geçerli DEĞİLDİR ve onlar standart güvenli sürümü (K-23a) kullanır.
- İlgili birim özel bir şirket olsa bile (ör. özel savunma sanayi firması), mod kendi başına açılamaz; her durumda ilgili devlet birimi (bakanlık) yetkilendirmek zorundadır. Karar mercii her zaman devlettir.

### Kuantuma dayanıklı imza ve değişmez kayıt (çekirdek gereksinim)
- Bu modda yetki belgeleri ve kritik kayıtlar, kuantuma dayanıklı imzayla (K-22, NIST FIPS 204 ML-DSA) korunur ve değişmez biçimde zincire işlenir. Amaç: bugün kaydedilen verinin "şimdi topla, sonra çöz" saldırılarına ve gelecekteki kuantum bilgisayarlara karşı da güvende kalması. Bu kurumlar için güvenlik, imza boyutu ve hız maliyetinin önündedir.
- Kuantuma dayanıklı imza ve değişmez zincir kaydı, savunma modunun çekirdek teknik gereksinimidir.

## Borsa / değer notu (dürüst)
- CEX listeleme: pahalı (30-50K$+), mainnet + audit + hacim ister — çok ileri.
- DEX: AVM sonrası, kendi zincirinde.
- Token SATIŞI: ön satış CANLI (Faz 1 → Faz 2 otomatik; yalnız aidag-chain.com/on-satis). Karar bilinçli olarak denetimden önceye alındı; listeleme fiyatı garanti edilmez.
- Değer, spekülasyondan değil GERÇEK KULLANIMDAN gelir.

## Kritik prensip
Her adım bir öncekine bağlı. Sıra atlanırsa (örn. köprüyü AVM'siz, audit'siz yapmak) =
güvenlik felaketi + boşa emek. Doğru sıra = akıcılık + zaman + güvenlik.
