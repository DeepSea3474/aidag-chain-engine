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

### K-21 · KUBRA ajan calisma dongusu (Eylul 2026)
- Karar: KUBRA her gorevi su donguyle yurutur: anla, analiz et, planla, onay al, guvenli ortamda uygula, dogrula, raporla, canliya al.
- Izin katmanlari: serbest (okuma, analiz, test ortaminda calistirma, rapor), onayli (canli sisteme dokunan her islem; tanimli, sinirli, geri alinabilir), yasak (imza, para/token, rol, konsensus, silme, cihaz yonetimi). Her adim kayit altinda.
- Guven: "onceden onayli" is listesi yalnizca olculmus basari gecmisine dayanarak coklu imzali yonetim karariyla genisletilir; KUBRA kendi yetkisini genisletemez.
- Neden: KUBRA'nin bildigini guvenle uygulayabilmesi; insan onayi ve kanit ilkesinin korunmasi.
- Ayrinti: CALISMA_VE_OGRENME_YONTEMI.md Bolum 9.

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
- Durum: Ana ağda yayında (25 Eylül 2026). Kod: `MAINNET_VESTING_BASLANGIC = TGE_BELIRSIZ`. Bkz. K-19.

### K-17 · Ön satış test kayıtları (Eylül 2026)
- Ana ağdaki 3 ön satış kaydı (toplam 39 AIDAG) kurucunun test kayıtlarıdır; resmî ön satış rakamlarında ayrı gösterilecektir.

### K-20 · Ön satışta yalnızca USDT (BEP-20) kabul edilir (Eylül 2026)
- Karar: Ön satış ödemeleri yalnızca BNB Smart Chain üzerindeki USDT (BEP-20) ile alınır. BNB ile ya da başka bir token veya ağ ile gönderilen ödemeler işlenmez.
- Neden: Otomatik ödeme izleyicisi BNB ödemelerini tespit edemiyor (denetim bulgusu CANLI-3); BNB fiyatı değişken ve tek bir fiyat kaynağına bağlı (CANLI-5).
- Minimum alım: 10 USDT. Hem sayfada hem ödeme izleyicisinde geçerlidir. 10 USDT altındaki ödemelere tahsis yazılmaz; ödeme "minimum altı" olarak kaydedilir ve iade için yöneticiye bildirilir.
- Uygulama: Ön satış sayfalarına (`/on-satis`, `/on-satis.html`) iki dilli uyarı eklenir: "Yalnızca BEP-20 ağındaki USDT kabul edilir; başka token veya ağ ile gönderilen ödemeler işlenmez." Sayfalardaki kullanılmayan BNB/ETH kur değerleri kaldırılır. "BNB Smart Chain" ağ adı ve işlem ücreti (gas) için cüzdanda az miktarda BNB gerektiği bilgisi korunur.
- Durum: **Canlı.**
  - Site: yayında (26 Eylül 2026). Uyarı ve minimum 10 USDT, site deposu `13f5920` (dal `on-satis-yalniz-usdt`).
  - Ödeme izleyicisi: canlı (26 Eylül 2026). Minimum altı ödemeler ayrı `minimum_alti` kaydına yazılır, günlüğe "MINIMUM ALTI ODEME (iade gerekli)" satırı düşer; iade bekleyenler `on-satis-izleyici.py --minimum-alti` ile listelenir. Motor deposu dal `izleyici-minimum-alti` (`7af969b`), `/opt/aidag/izleyici` üzerinde çalışıyor.
- **[Sonraki iş]** Ödeme izleyicisinde BNB yolunu kod düzeyinde kapatmak (bugün yalnızca Etherscan anahtarı ya da `BNB_TARA=1` ile açılıyor; ikisi de kapalı). Yanlış token veya ağ ile gelen ödemeler için iade sürecinin yazılması.

---

## 5. Çalışma yöntemi

### K-18 · Önce kanıt, sonra vaat
- Her değişiklik: analiz → ayrı dalda geliştirme → tüm testler → canlı ağ geçmişiyle geriye uyumluluk → insan onayı → kontrollü yayın.
- Canlıda çalışan kod ile ana depo her zaman aynı olmalıdır. Acil durumda canlıya doğrudan yapılan düzeltme en geç ertesi gün depoya alınır ve denetlenir.
- Ayrıntı: CALISMA_VE_OGRENME_YONTEMI.md

---

## 6. Yayın ve denetim kayıtları

### K-19 · Bağımsız denetim düzeltmelerinin ana ağ yayını (25 Eylül 2026)
- Karar: Bağımsız güvenlik denetiminin (`DENETIM_ONCESI_RAPOR.md`) kritik bulguları için hazırlanan düzeltmeler, 27 Eylül'den önce ana ağın iki düğümüne alındı. Aşağıdaki "K-01…K-08" kısaltmaları denetim raporundaki bulgu kimlikleridir; bu belgedeki karar numaralarıyla ilgisi yoktur.
- Ne değişti:
  - Denetim K-01: yeni blok en fazla 8 uç seçer; paralel uç şişirmeyle zincir durdurulamaz.
  - Denetim K-02: gelecek tarihli blok bekleme havuzu ya da disk yoluyla ağa sızamaz.
  - Denetim K-03: başarısız bir akıllı sözleşme çağrısı sözleşme durumunu silmez.
  - Denetim K-04: kilitli (vesting) token akıllı sözleşme yoluyla taşınamaz.
  - Denetim K-05: işlem makbuzları gerçek sonucu gösterir; geçersiz işlem baştan reddedilir.
  - Denetim K-06: başka zincirde imzalanmış işlem tekrar oynatılamaz (chainId zorunlu; ana ağ 3474 değişmedi).
  - Denetim K-07 (ara çözüm): RPC yazmalarına istemci başına hız sınırı; ağ yalnızca doğrulanmış blokları iletir.
  - Denetim K-08: belge kayıt betiği sabit, herkesçe bilinen anahtarı kullanmaz.
  - Genesis vesting 2100 (K-16).
  - Altyapı (depo dışı): nginx gerçek istemci IP'sini Cloudflare'in `X-Forwarded-For` başlığından alır (Cloudflare bu alan adında `CF-Connecting-IP` göndermiyor); düğümlerde ortak yazma tavanı pratikte kapalı, sınır istemci başına.
- Yayın commit'i: `cb814390aeca03509eb192ae5d2a507b879da110` (`denetim/kritik-duzeltmeler`, `rwa-oracle-kyc` üzerine). Bu kayıt, o dalın ana depoya birleştirildiği commit'tedir (K-18: canlıdaki kod ile ana depo aynı).
- Çalışan ikili: `/opt/aidag/lsc-node-cb81439/lsc-node`, sha256 `4e104c567097cda63495aef84dcdb3935524773963acbeb06c07365fea396d75` (önceki: `eaed5edf261d5798…`).
- Kanıt:
  - Testler: yayın dalı 469 geçti / 0 başarısız; bu birleştirme sonrası tüm testler başarılı (sayı birleştirme commit mesajında).
  - Canlı ağ geçmişiyle geriye uyum: iki düğümün veri kopyaları (3.964 blok) eski ve yeni kodla birebir aynı sonucu verdi.
  - Yayın sonrası: iki düğüm yeni ikilide, genesis `b82345008ae109d8`, blok sayıları eşit; 7 genesis adresinin bakiyesi ve ön satış özeti yayın öncesiyle aynı; hata yok.
  - RWA modülleri ana ağda kapalı: aynı RWA senaryosu test ağında çalışıyor, ana ağda doğru imzayla bile hiçbir kayıt oluşturmuyor (test: `denetim_testleri::rwa_ana_ag`).
- Yayın (UTC): 11:12 yedek · 11:14 ön satış izleyicisi ve kurtarma botu durduruldu, nginx · 11:18 ikinci düğüm · 11:23 herkese açık düğüm (birkaç saniyelik RPC kesintisi) · 11:26 servisler yeniden başlatıldı.
- Duraklama sırasında ön satış izleyicisinin tek sağlayıcıyla taradığı BSC blokları (123.937.377–123.940.147) iki bağımsız sağlayıcıyla yeniden tarandı: ödeme yok.
- Geri dönüş:
  - 27 Eylül 2026 00:00 UTC'den önce: düğümlerdeki `cb81439.conf` systemd dosyası silinir, servis yeniden başlatılır → eski ikili. Veri biçimi aynı.
  - Hız sınırı: önce düğümlerdeki `hiz-siniri.conf` kaldırılır, sonra nginx eski haline alınır (sıra önemli).
  - 27 Eylül 00:00 UTC'den sonra eski ikiliye dönülmez (eski sürüm genesis dilimlerini açık sayar); yalnızca ileri düzeltme.
- **[Sonraki iş]** 27 Eylül 00:00 UTC kontrolü; daha eski tek kaynaklı tarama aralıklarının yeniden taranması; denetim K-07'nin protokol çözümü (ücret ya da komite); denetim raporundaki yüksek ve orta bulgular.

