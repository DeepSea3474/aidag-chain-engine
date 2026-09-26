# KUBRA · Onaylı Kaynak Listesi

Bu liste, KUBRA'nın bilgi tabanına eklenecek **seçilmiş** kaynakları tanımlar.
İlke: KUBRA daha az ama **doğru, lisansı uygun ve güvenilir** kaynaktan öğrenir.
Ayrıntılı kurallar: CALISMA_VE_OGRENME_YONTEMI.md (Bölüm 5 ve 8) · KARARLAR.md (K-05, K-06, K-08)

---

## Kurallar

1. **Onay:** Listeye kaynak eklemek kurucunun onayıyla yapılır.
2. **Lisans:** Yalnızca izin verici lisanslı (MIT, Apache-2.0, BSD, CC0, CC BY) içerik ya da kamuya açık resmî metinler. CC BY-SA içerik kaynak gösterilerek kullanılır. Telifli ve ticari kullanımı kısıtlı kaynaklar **eklenmez**, yalnızca adıyla anılabilir.
3. **Doğrulama:** Aşağıdaki lisans bilgileri ilk tahmindir; her kaynak eklenmeden önce resmî sayfasından doğrulanır ve "Lisans (doğrulandı)" sütunu doldurulur.
4. **Kayıt:** Her kaynağın adresi, sürümü, alınma tarihi ve özeti (hash) kaydedilir; özet zincire yazılabilir.
5. **Temizlik:** İçerikteki, yapay zekâyı yönlendirmeye çalışan gizli talimatlar ayıklanır.
6. **Sınav:** Yeni kaynakla güncellenen bilgi tabanı, değerlendirme setinden geçmeden yayına girmez.
7. **Yetki sınırı:** "Araç ve sistem yönetimi" kaynakları KUBRA'nın **teşhis, izleme, raporlama ve öneri** yeteneğini güçlendirmek içindir. KUBRA sistemlere, cihazlara veya zincire müdahale etmez; müdahale kararı yetkili insandadır (K-05).

Durum kodları: **Bekliyor** · **Lisans kontrolünde** · **Eklendi** · **Reddedildi**

---

## Öncelik 0 · Kendi kaynaklarımız

| Kaynak | Adres | Lisans (ilk tahmin) | Durum |
|---|---|---|---|
| AIDAG motoru (kod, testler, tasarım belgeleri) | github.com/DeepSea3474/aidag-chain-engine | BUSL-1.1 (kendi kodumuz) | Bekliyor |
| KARARLAR.md | aynı depo | Kendi belgemiz | Bekliyor |
| CALISMA_VE_OGRENME_YONTEMI.md | aynı depo | Kendi belgemiz | Bekliyor |
| MIMARI.md (hazırlanacak) | aynı depo | Kendi belgemiz | Bekliyor |
| Site ve belge doğrulama sayfaları | aidag-chain sitesi deposu | Kendi kodumuz | Bekliyor |

---

## 1 · Rust ve sistem programlama

| Kaynak | Lisans (ilk tahmin) | Öncelik | Durum |
|---|---|---|---|
| The Rust Programming Language (resmî kitap) | MIT / Apache-2.0 | Yüksek | Bekliyor |
| Rust standart kütüphane belgeleri | MIT / Apache-2.0 | Yüksek | Bekliyor |
| Rustonomicon (güvensiz Rust) | MIT / Apache-2.0 | Orta | Bekliyor |
| Rust API Guidelines | MIT / Apache-2.0 | Orta | Bekliyor |
| tokio belgeleri | MIT | Yüksek | Bekliyor |
| serde belgeleri | MIT / Apache-2.0 | Orta | Bekliyor |

## 2 · Blokzincir ve dağıtık sistemler

| Kaynak | Lisans (ilk tahmin) | Öncelik | Durum |
|---|---|---|---|
| GHOSTDAG / PHANTOM makaleleri (hakemli / arXiv) | Makale lisansına göre | Yüksek | Bekliyor |
| rusty-kaspa kaynak kodu ve belgeleri | ISC | Yüksek | Bekliyor |
| Ethereum geliştirme önerileri (EIP'ler) | CC0 | Yüksek | Bekliyor |
| revm belgeleri | MIT | Yüksek | Bekliyor |
| libp2p belgeleri ve spesifikasyonları | MIT / Apache-2.0 | Yüksek | Bekliyor |
| ERC-3643 (izinli token standardı) | Doğrulanacak | Orta | Bekliyor |

## 3 · Kriptografi

| Kaynak | Lisans (ilk tahmin) | Öncelik | Durum |
|---|---|---|---|
| BLAKE3 spesifikasyonu ve referans uygulaması | CC0 / Apache-2.0 | Yüksek | Bekliyor |
| ed25519-dalek belgeleri | BSD-3 | Yüksek | Bekliyor |
| secp256k1 kütüphane belgeleri | Doğrulanacak | Orta | Bekliyor |
| NIST kuantum sonrası kriptografi standartları (FIPS 203/204/205) | ABD kamu yayını | Orta | Bekliyor |
| NIST kriptografik yönergeleri (ör. SP 800 serisi) | ABD kamu yayını | Orta | Bekliyor |

## 4 · Siber güvenlik

| Kaynak | Lisans (ilk tahmin) | Öncelik | Durum |
|---|---|---|---|
| NIST Siber Güvenlik Çerçevesi (CSF 2.0) | ABD kamu yayını | Yüksek | Bekliyor |
| NIST SP 800-53 (güvenlik kontrolleri) | ABD kamu yayını | Orta | Bekliyor |
| NIST SP 800-61 (olay müdahalesi) | ABD kamu yayını | Yüksek | Bekliyor |
| MITRE ATT&CK (saldırı teknikleri bilgi tabanı) | Kullanım koşulları doğrulanacak | Yüksek | Bekliyor |
| MITRE ATLAS (yapay zekâya yönelik saldırılar) | Kullanım koşulları doğrulanacak | Yüksek | Bekliyor |
| OWASP Top 10 ve OWASP LLM Top 10 | CC BY-SA | Yüksek | Bekliyor |
| OWASP Uygulama Güvenliği Doğrulama Standardı (ASVS) | CC BY-SA | Orta | Bekliyor |
| CVE / NVD açık zafiyet kayıtları | Kamu verisi, koşullar doğrulanacak | Orta | Bekliyor |
| Güvenlik firmalarının herkese açık denetim raporları | Rapor bazında doğrulanacak | Orta | Bekliyor |
| CIS Benchmarks | Ticari kullanım kısıtlı olabilir | — | Lisans kontrolünde |
| IEC 62443 (endüstriyel siber güvenlik) | Telifli, ücretli | — | Eklenmez, adıyla anılır |

## 5 · Yapay zekâ, araç kullanımı ve güvenli ajan tasarımı

| Kaynak | Lisans (ilk tahmin) | Öncelik | Durum |
|---|---|---|---|
| Hugging Face transformers belgeleri | Apache-2.0 | Yüksek | Bekliyor |
| llama.cpp belgeleri | MIT | Yüksek | Bekliyor |
| PyTorch belgeleri | BSD | Orta | Bekliyor |
| Model Context Protocol (araç bağlantı standardı) spesifikasyonu | Doğrulanacak | Yüksek | Bekliyor |
| OpenAPI spesifikasyonu | Apache-2.0 | Orta | Bekliyor |
| JSON Schema spesifikasyonu | Doğrulanacak | Orta | Bekliyor |
| RAG, araç kullanımı, halüsinasyon ölçümü ve değerlendirme üzerine hakemli yayınlar (OpenAlex üzerinden) | Makale lisansına göre | Yüksek | Bekliyor |
| NIST Yapay Zekâ Risk Yönetim Çerçevesi (AI RMF) | ABD kamu yayını | Yüksek | Bekliyor |
| Anlamsal çıkarım veri setleri (SNLI, MultiNLI, NLI-TR vb.) | Set bazında doğrulanacak; ticari olmayan şartlılar eklenmez | Yüksek | Lisans kontrolünde |

## 6 · Sistem yönetimi ve gözlemlenebilirlik

| Kaynak | Lisans (ilk tahmin) | Öncelik | Durum |
|---|---|---|---|
| systemd belgeleri | Doğrulanacak | Yüksek | Bekliyor |
| nginx belgeleri | Doğrulanacak | Orta | Bekliyor |
| Kubernetes belgeleri | CC BY 4.0 | Orta | Bekliyor |
| Prometheus belgeleri | Apache-2.0 | Orta | Bekliyor |
| OpenTelemetry belgeleri | Apache-2.0 | Orta | Bekliyor |
| Google SRE kitapları (çevrim içi sürümler) | Kullanım koşulları doğrulanacak | Orta | Bekliyor |

## 7 · Endüstriyel sistemler (iş kolları için)

| Kaynak | Lisans (ilk tahmin) | Öncelik | Durum |
|---|---|---|---|
| OPC UA açık spesifikasyon bölümleri | Kullanım koşulları doğrulanacak | Orta | Bekliyor |
| NIST SP 800-82 (endüstriyel kontrol sistemleri güvenliği) | ABD kamu yayını | Yüksek | Bekliyor |
| GS1 DataMatrix ve barkod standartları (açık kılavuzlar) | Kullanım koşulları doğrulanacak | Orta | Bekliyor |

## 8 · Mevzuat ve düzenleme

| Kaynak | Lisans (ilk tahmin) | Öncelik | Durum |
|---|---|---|---|
| AB Sınırda Karbon Düzenlemesi tüzüğü ve uygulama yönetmelikleri (EUR-Lex) | AB resmî metni, yeniden kullanıma açık | Yüksek | Bekliyor |
| AB Yapay Zekâ Yasası (EUR-Lex) | AB resmî metni | Yüksek | Bekliyor |
| GDPR (EUR-Lex) | AB resmî metni | Orta | Bekliyor |
| KVKK ve ikincil düzenlemeler (mevzuat.gov.tr) | Resmî metin | Yüksek | Bekliyor |
| SPK kripto varlık düzenlemeleri (resmî metinler) | Resmî metin | Orta | Bekliyor |
| IPCC emisyon faktörü veritabanı ve kılavuzları | Kullanım koşulları doğrulanacak | Yüksek | Bekliyor |
| GHG Protocol standartları | Kullanım koşulları doğrulanacak | Orta | Bekliyor |
| ISO standartları (14064, 27001 vb.) | Telifli, ücretli | — | Eklenmez, adıyla anılır |

---

## Ekleme sırası

1. Öncelik 0: kendi kaynaklarımız
2. Yüksek öncelikli, lisansı doğrulanmış kaynaklar (Rust, blokzincir, kriptografi, siber güvenlik çerçeveleri, AI güvenliği)
3. İş kolu kaynakları: karbon düzenlemesi ve emisyon faktörleri
4. Orta öncelikliler

Her ekleme turundan sonra değerlendirme seti çalıştırılır ve sonuç kaydedilir.

---

*Yaşayan belge. Kaynak ekleme, çıkarma ve lisans doğrulama sonuçları commit ile kayıt altına alınır.*
