# AIDAG-Chain · Kararlar

Bu belge, projede alınan önemli kararları ve nedenlerini kayıt altında tutar.
Her karar tarihiyle yazılır; değişen kararlar silinmez, "yerini aldı" notuyla güncellenir.
Uygulanmamış kararlar **[Sonraki iş]** olarak işaretlenir.

---

## 1. Yönetim ve yetki

### K-01 · DAO yok (Eylül 2026)
- Karar: Proje DAO ile yönetilmeyecek. Kod yorumlarındaki DAO ifadeleri kaldırıldı ("yetkili çok imzalı onay" olarak güncellendi).
- Neden: Hukuki sorumluluğun belirsizleşmemesi; kurumsal ve kamu müşterileri için net yetki yapısı.

### K-02 · Kritik yetkiler çoklu imzaya (M-of-N) bağlanır (Eylül 2026)
- Karar: Kurum rolü verme/iptal, kurum doğrulama ve akış tanımı gibi kritik işlemler tek anahtarla değil, başlangıçta 2/3 çoklu imzayla yapılır. Eşik ve imzacı listesi değişikliği de aynı çoklu imzaya tabidir.
- Güvenceler: imzacı anahtarları sunucuda tutulmaz; aynı imzacı iki kez sayılmaz; eski imza tekrar kullanılamaz; imzacı sayısı eşiğin altına düşürülemez.
- Durum: rwa-oracle-kyc dalında kodlandı ve test edildi; ana ağda kapalı.

### K-03 · Dış ortakların imzacı payı sınırlıdır (Eylül 2026)
- Karar: Bir dış ortak (ör. iş ortağı firma) çoklu imzada en fazla 1 anahtar tutabilir. Çoğunluk AIDAG tarafında kalır.
- Neden: Ağın kontrolünün fiilen bir dış tarafa geçmesini önlemek.

### K-04 · Hakkında işlem yapılan taraf, kendi hakkındaki kararı engelleyemez (Eylül 2026)
- Karar: Kural herkese eşittir. İhraççı, yatırımcı, aracı kurum veya altyapı işletmecisi fark etmeksizin, hakkında işlem yapılan taraf ilgili çoklu imza sürecinde imzacı sayılmaz.
- Neden: Çıkar çatışmasını önlemek; ör. usulsüzlüğü yapan ihraççı banka, kendi aleyhine alınan tedbiri imza vermeyerek kilitleyememelidir.
- **[Sonraki iş]** M-of-N yönetimine "hedef taraf imzacı olamaz" kuralının eklenmesi.

---

## 2. Yapay zekâ (KUBRA)

### K-05 · KUBRA'nın imza ve müdahale yetkisi yoktur (Eylül 2026)
- İlke: Yapay zekâ önerir, yetkili insan imzalar, protokol doğrular.
- KUBRA imza atmaz, para/token hareket ettirmez, rol vermez, kayıt silmez, cihazlara komut göndermez, yazıcıya doğrudan erişmez.
- Uygulama: KUBRA imza adresi RWA modüllerinde yasaklı listede; rolsüz servislerin RWA işlemi yapamadığı testle kanıtlandı. Anahtar dosyası yoksa veya bozuksa servis açılmaz (fail-closed).

### K-06 · Doğrulanamayan bilgi cevap olmaz (Eylül 2026)
- Karar: KUBRA her bilgi iddiasını bir kaynağa, ölçüme veya zincir kaydına dayandırır; dayanağı yoksa "doğrulanmış bilgim yok" der. Doğrulanmış bilgi ile öneri her zaman ayrı etiketlenir.
- Ayrıntı: CALISMA_VE_OGRENME_YONTEMI.md

### K-07 · KUBRA cevap kanıtları tuzlu hash ile kaydedilir (Eylül 2026)
- Karar: Zincire soru/cevap metni değil, rastgele tuz eklenmiş özet yazılır; tuz yalnızca kullanıcıya verilir.
- Neden: Kişisel veri koruması ve tahmin saldırısının önlenmesi.
- Durum: Canlıda.

### K-08 · KUBRA kendi kendine öğrenir, kendi kendine değişmez (Eylül 2026)
- Karar: Yeni bilgi ve model sürümleri değerlendirme setinden ve insan onayından geçmeden yayına girmez. Kapalı kurumlara güncellemeler imzalı paket olarak verilir.

---

## 3. Ürün ve iş modeli

### K-09 · AIDAG teknoloji sağlayıcıdır; token ihraç etmez (Eylül 2026)
- Karar: Kurumsal varlık tokenleştirmede tokenleri lisanslı kurumlar (banka, aracı kurum vb.) ihraç eder; AIDAG altyapıyı sağlar ve lisanslar. AIDAG token alıp satmaz, saklamaz, yatırımcı fonu tutmaz; KYC verisi lisanslı kurumda kalır.
- Hedef: Devlet denetiminde, Türkiye'deki kurum ve büyük şirketlerin varlıklarının tokenleştirilmesi; ileride uluslararası lisanslı aracılar üzerinden yabancı yatırımcı erişimi.

### K-10 · Düzenleyici için denetçi düğümü (Eylül 2026)
- Karar: Düzenleyici kurumlar kendi düğümleriyle zinciri anlık izleyebilir; kritik kararlarda düzenleyici imzası zorunlu tutulabilir.

### K-11 · Adli emanet havuzu (Eylül 2026)
- Karar: Yetkili yargı kararı olduğunda, yetkili kurum çoklu imzayla şüpheli varlıkları adli emanet havuzuna alabilir. İnceleme sonunda varlıklar ya sahibine iade edilir ya da karar uyarınca mağdurlara ödenir.
- Güvenceler: Zincir geçmişi asla değiştirilmez, işlem yeni ve imzalı bir kayıttır; karar belgesinin hash'i zincire yazılmadan işlem kabul edilmez; yetki yalnızca karardaki varlıklarla sınırlıdır; tek bir yetkili işlem yapamaz; hakkında işlem yapılan taraf engelleyemez ama görür ve itiraz yolunu kullanabilir; AIDAG'ın kendisi bu yetkiye sahip değildir; tedbir süreli olur.
- **[Sonraki iş]** Hukuki usulün bir avukatla netleştirilmesi; teknik tasarım ve testler.

### K-12 · Kurumsal ağlar ayrı ve kapalıdır (Eylül 2026)
- Karar: Kurumlara aynı yazılımla, kendi ağ kimliğine sahip, internete kapalı ayrı zincirler kurulur. Açık ağa köprü yoktur. Kamuya açık doğrulama isteyen kurumlar için yalnızca tek yönlü özet paylaşımı isteğe bağlıdır; gizli kurumlarda kullanılmaz.
- Kod: tek kod tabanı; kuruma özel olan yalnızca yapılandırmadır ve herkese açık depoya girmez.

### K-13 · Şirket A.Ş. olarak kurulur (Eylül 2026)
- Neden: Uluslararası iş birlikleri, yatırımcı girişi ve pay devri.
- Esas sözleşme: yazılım, blokzincir altyapısı, yapay zekâ, tokenizasyon altyapısı yazılımlarının geliştirilmesi, kurulumu ve lisanslanması. SPK lisansı gerektiren faaliyetler (kripto varlık alım satımı, saklama, borsa) yazılmaz. Avukatla netleştirilecek.

### K-14 · Sertifikasyon ve lisans yolu aşamalıdır (Eylül 2026)
- Önce: ISO 27001, ISO 27701, KVKK uyumu, Yerli Malı Belgesi, ürün güvenlik değerlendirmesi, gerekirse Tesis Güvenlik Belgesi.
- Sonra (36 ay +): Finansal faaliyet lisansı ihtiyacının değerlendirilmesi.

### K-15 · Yazılı materyallerde gerçek kurum adı kullanılmaz (Eylül 2026)
- Karar: Anlaşma olmadıkça sunum ve belgelerde banka veya kurum adı yerine genel ifadeler kullanılır.

---

## 4. Token ve ağ

### K-16 · Genesis vesting başlangıcı "belirlenmedi" (2100) (Eylül 2026)
- Karar: Genesis dilimlerinin açılışı, ön satış claim'iyle aynı "belirlenmedi" değerine alındı. Gerçek TGE tarihi ortaklık kurulunca belirlenecek.
- Neden: Ekip ve yatırımcı arasında simetri; ekip tokenleri yatırımcıdan önce açılmaz.
- Tarih geçmişi: 15 Temmuz → 21 Eylül → 26 Ağustos → 27 Eylül → belirlenmedi.
- **[Sonraki iş]** Tarihin koddan çıkarılıp M-of-N imzalı, yalnızca bir kez ayarlanabilen ve geçmişe yazılamayan bir zincir işlemine bağlanması.

### K-17 · Ön satış test kayıtları (Eylül 2026)
- Ana ağdaki 3 ön satış kaydı (toplam 39 AIDAG) kurucunun test kayıtlarıdır; resmî ön satış rakamlarında ayrı gösterilecektir.

---

## 5. Çalışma yöntemi

### K-18 · Önce kanıt, sonra vaat
- Her değişiklik: analiz → ayrı dalda geliştirme → tüm testler → canlı ağ geçmişiyle geriye uyumluluk → insan onayı → kontrollü yayın.
- Canlıda çalışan kod ile ana depo her zaman aynı olmalıdır. Acil durumda canlıya doğrudan yapılan düzeltme en geç ertesi gün depoya alınır ve denetlenir.
- Ayrıntı: CALISMA_VE_OGRENME_YONTEMI.md
