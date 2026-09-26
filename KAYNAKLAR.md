# KUBRA · Onaylı Kaynak Listesi

Bu liste, KUBRA'nın bilgi tabanına eklenecek **seçilmiş** kaynakları tanımlar.
İlke: KUBRA daha az ama **doğru, lisansı uygun ve güvenilir** kaynaktan öğrenir.
Ayrıntılı kurallar: CALISMA_VE_OGRENME_YONTEMI.md (Bölüm 5 ve 8) · KARARLAR.md (K-05, K-06, K-08)

---

## Kurallar

1. **Onay:** Listeye kaynak eklemek kurucunun onayıyla yapılır.
2. **Lisans:** Yalnızca izin verici lisanslı (MIT, Apache-2.0, BSD, CC0, CC BY) içerik ya da kamuya açık resmî metinler. CC BY-SA içerik kaynak gösterilerek kullanılır. Telifli ve ticari kullanımı kısıtlı kaynaklar **eklenmez**, yalnızca adıyla anılabilir.
3. **Doğrulama:** Aşağıdaki lisans bilgileri ilk tahmindir; her kaynak eklenmeden önce resmî sayfasından doğrulanır ve "Lisans (doğrulandı)" sütunu doldurulur.
   - İlk doğrulama turu: 26 Eylül 2026 (resmî lisans dosyaları ve kullanım şartları sayfaları). "(kısmen doğrulandı)" veya "(doğrulanmadı)" notu olanlarda resmî sayfa doğrudan okunamadı; eklemeden önce elle teyit edilmelidir. Bu turda hiçbir kaynak KUBRA'ya eklenmedi.
   - ⚠ **Ticari kullanım kısıtlı** işaretli kaynaklar Kural 2 gereği eklenmez. ◐ **ShareAlike** işaretlilerden türetilip dağıtılan metin aynı lisansla verilir.
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

| Kaynak | Lisans (ilk tahmin) | Lisans (doğrulandı) | Öncelik | Durum |
|---|---|---|---|---|
| The Rust Programming Language (resmî kitap) | MIT / Apache-2.0 | MIT OR Apache-2.0 (rust-lang/book COPYRIGHT) | Yüksek | Bekliyor |
| Rust standart kütüphane belgeleri | MIT / Apache-2.0 | MIT OR Apache-2.0 (rust-lang/rust COPYRIGHT) | Yüksek | Bekliyor |
| Rustonomicon (güvensiz Rust) | MIT / Apache-2.0 | MIT OR Apache-2.0 (depo LICENSE dosyaları) | Orta | Bekliyor |
| Rust API Guidelines | MIT / Apache-2.0 | MIT OR Apache-2.0 | Orta | Bekliyor |
| tokio belgeleri | MIT | MIT (tokio-rs/website) | Yüksek | Bekliyor |
| serde belgeleri | MIT / Apache-2.0 | ◐ ShareAlike: serde.rs sitesi CC BY-SA 4.0; API belgeleri (docs.rs) MIT OR Apache-2.0 | Orta | Bekliyor |

## 2 · Blokzincir ve dağıtık sistemler

| Kaynak | Lisans (ilk tahmin) | Lisans (doğrulandı) | Öncelik | Durum |
|---|---|---|---|---|
| GHOSTDAG / PHANTOM makaleleri (hakemli / arXiv) | Makale lisansına göre | IACR ePrint 2018/104: CC BY 4.0. ACM AFT 2021 sürümü kullanılmamalı (lisansı doğrulanmadı) | Yüksek | Bekliyor |
| rusty-kaspa kaynak kodu ve belgeleri | ISC | ISC | Yüksek | Bekliyor |
| Ethereum geliştirme önerileri (EIP'ler) | CC0 | CC0 (EIP-1: tüm EIP'ler kamu malı) | Yüksek | Bekliyor |
| revm belgeleri | MIT | MIT (depo lisansı; kitap için ayrı beyan yok, kısmen doğrulandı) | Yüksek | Bekliyor |
| libp2p belgeleri ve spesifikasyonları | MIT / Apache-2.0 | **Belirsiz:** docs README "CC BY-SA" diyor, bağlantı CC BY 4.0'a gidiyor; specs deposunda lisans dosyası yok (doğrulanmadı) | Yüksek | Bekliyor |
| ERC-3643 (izinli token standardı) | Doğrulanacak | CC0 | Orta | Bekliyor |

## 3 · Kriptografi

| Kaynak | Lisans (ilk tahmin) | Lisans (doğrulandı) | Öncelik | Durum |
|---|---|---|---|---|
| BLAKE3 spesifikasyonu ve referans uygulaması | CC0 / Apache-2.0 | CC0 1.0 OR Apache-2.0 (OR Apache-2.0 WITH LLVM-exception, kod) | Yüksek | Bekliyor |
| ed25519-dalek belgeleri | BSD-3 | BSD-3-Clause | Yüksek | Bekliyor |
| secp256k1 kütüphane belgeleri | Doğrulanacak | bitcoin-core/secp256k1: MIT; Rust k256: Apache-2.0 OR MIT | Orta | Bekliyor |
| NIST kuantum sonrası kriptografi standartları (FIPS 203/204/205) | ABD kamu yayını | ABD kamu eseri + NIST telifsiz dünya çapında izin; atıf, onay ima edilemez | Orta | Bekliyor |
| NIST kriptografik yönergeleri (ör. SP 800 serisi) | ABD kamu yayını | ABD kamu eseri + NIST izni (üçüncü taraf içerik ayrıca kontrol) | Orta | Bekliyor |

## 4 · Siber güvenlik

| Kaynak | Lisans (ilk tahmin) | Lisans (doğrulandı) | Öncelik | Durum |
|---|---|---|---|---|
| NIST Siber Güvenlik Çerçevesi (CSF 2.0) | ABD kamu yayını | ABD kamu eseri + NIST izni (CSWP kapsamı çıkarım, kısmen doğrulandı) | Yüksek | Bekliyor |
| NIST SP 800-53 (güvenlik kontrolleri) | ABD kamu yayını | ABD kamu eseri + NIST izni | Orta | Bekliyor |
| NIST SP 800-61 (olay müdahalesi) | ABD kamu yayını | ABD kamu eseri + NIST izni | Yüksek | Bekliyor |
| MITRE ATT&CK (saldırı teknikleri bilgi tabanı) | Kullanım koşulları doğrulanacak | MITRE ATT&CK Kullanım Şartları: telifsiz, ticari dahil; MITRE telif bildirimi zorunlu | Yüksek | Bekliyor |
| MITRE ATLAS (yapay zekâya yönelik saldırılar) | Kullanım koşulları doğrulanacak | **Belirsiz:** atlas-data LICENSE Apache-2.0, README "ALL RIGHTS RESERVED" (doğrulanmadı) | Yüksek | Bekliyor |
| OWASP Top 10 ve OWASP LLM Top 10 | CC BY-SA | ◐ ShareAlike: CC BY-SA 4.0 (her ikisi) | Yüksek | Bekliyor |
| OWASP Uygulama Güvenliği Doğrulama Standardı (ASVS) | CC BY-SA | ◐ ShareAlike: CC BY-SA 4.0 | Orta | Bekliyor |
| CVE / NVD açık zafiyet kayıtları | Kamu verisi, koşullar doğrulanacak | CVE: MITRE Kullanım Şartları (telifsiz, türev serbest, bildirim); NVD API: "not endorsed" bildirimi (kısmen doğrulandı) | Orta | Bekliyor |
| Güvenlik firmalarının herkese açık denetim raporları | Rapor bazında doğrulanacak | ⚠ **Ticari kullanım kısıtlı:** varsayılan telifli; rapor bazında (doğrulanmadı) | Orta | Bekliyor |
| CIS Benchmarks | Ticari kullanım kısıtlı olabilir | ⚠ **Ticari kullanım kısıtlı:** CC BY-NC-SA 4.0; ticari kullanım CIS onayı/üyeliği gerektirir | — | Lisans kontrolünde |
| IEC 62443 (endüstriyel siber güvenlik) | Telifli, ücretli | ⚠ **Ticari kullanım kısıtlı:** ücretli, telifli; ISA yapay zekâ araçlarına girişi açıkça yasaklıyor | — | Eklenmez, adıyla anılır |

## 5 · Yapay zekâ, araç kullanımı ve güvenli ajan tasarımı

| Kaynak | Lisans (ilk tahmin) | Lisans (doğrulandı) | Öncelik | Durum |
|---|---|---|---|---|
| Hugging Face transformers belgeleri | Apache-2.0 | Apache-2.0 | Yüksek | Bekliyor |
| llama.cpp belgeleri | MIT | MIT | Yüksek | Bekliyor |
| PyTorch belgeleri | BSD | BSD-3-Clause (depo lisansı; site için ayrı beyan yok) | Orta | Bekliyor |
| Model Context Protocol (araç bağlantı standardı) spesifikasyonu | Doğrulanacak | Apache-2.0 (yeni katkılar), eski katkılar MIT, spec dışı belgeler CC BY 4.0 | Yüksek | Bekliyor |
| OpenAPI spesifikasyonu | Apache-2.0 | Apache-2.0 | Orta | Bekliyor |
| JSON Schema spesifikasyonu | Doğrulanacak | BSD-3-Clause OR AFL-3.0 | Orta | Bekliyor |
| RAG, araç kullanımı, halüsinasyon ölçümü ve değerlendirme üzerine hakemli yayınlar (OpenAlex üzerinden) | Makale lisansına göre | OpenAlex metaverisi CC0; makale metinleri makale bazında (NC/ND/telifliler kısıtlı) | Yüksek | Bekliyor |
| NIST Yapay Zekâ Risk Yönetim Çerçevesi (AI RMF) | ABD kamu yayını | ABD kamu eseri + NIST izni | Yüksek | Bekliyor |
| Anlamsal çıkarım veri setleri (SNLI, MultiNLI, NLI-TR vb.) | Set bazında doğrulanacak; ticari olmayan şartlılar eklenmez | ◐ ShareAlike: SNLI ve SNLI-TR CC BY-SA 4.0; MultiNLI karma (OANC, CC BY-SA 3.0, CC BY 3.0, kamu malı) (kısmen doğrulandı) | Yüksek | Lisans kontrolünde |

## 6 · Sistem yönetimi ve gözlemlenebilirlik

| Kaynak | Lisans (ilk tahmin) | Lisans (doğrulandı) | Öncelik | Durum |
|---|---|---|---|---|
| systemd belgeleri | Doğrulanacak | LGPL-2.1-or-later (man sayfaları; örnekler MIT-0), zayıf copyleft | Yüksek | Bekliyor |
| nginx belgeleri | Doğrulanacak | BSD-2-Clause (nginx.org) | Orta | Bekliyor |
| Kubernetes belgeleri | CC BY 4.0 | CC BY 4.0 | Orta | Bekliyor |
| Prometheus belgeleri | Apache-2.0 | Apache-2.0 | Orta | Bekliyor |
| OpenTelemetry belgeleri | Apache-2.0 | Belgeler CC BY 4.0 (ilk tahmin Apache-2.0 idi; kod Apache-2.0) | Orta | Bekliyor |
| Google SRE kitapları (çevrim içi sürümler) | Kullanım koşulları doğrulanacak | ⚠ **Ticari kullanım kısıtlı:** CC BY-NC-ND 4.0 (ticari değil, türev yasak) | Orta | Bekliyor |

## 7 · Endüstriyel sistemler (iş kolları için)

| Kaynak | Lisans (ilk tahmin) | Lisans (doğrulandı) | Öncelik | Durum |
|---|---|---|---|---|
| OPC UA açık spesifikasyon bölümleri | Kullanım koşulları doğrulanacak | ⚠ **Ticari kullanım kısıtlı:** OPC Foundation sözleşmesi: kopyalama ve yeniden dağıtım yasak; yalnızca dahili/uygulama | Orta | Bekliyor |
| NIST SP 800-82 (endüstriyel kontrol sistemleri güvenliği) | ABD kamu yayını | ABD kamu eseri + NIST izni | Yüksek | Bekliyor |
| GS1 DataMatrix ve barkod standartları (açık kılavuzlar) | Kullanım koşulları doğrulanacak | ⚠ **Ticari kullanım kısıtlı:** yalnızca değiştirilmemiş, kişisel/kurum içi kullanım; dağıtım için yazılı onay (kısmen doğrulandı) | Orta | Bekliyor |

## 8 · Mevzuat ve düzenleme

| Kaynak | Lisans (ilk tahmin) | Lisans (doğrulandı) | Öncelik | Durum |
|---|---|---|---|---|
| AB Sınırda Karbon Düzenlemesi tüzüğü ve uygulama yönetmelikleri (EUR-Lex) | AB resmî metni, yeniden kullanıma açık | EUR-Lex yeniden kullanım (2011/833/EU): ticari dahil serbest, kaynak belirtilir (kısmen doğrulandı) | Yüksek | Bekliyor |
| AB Yapay Zekâ Yasası (EUR-Lex) | AB resmî metni | EUR-Lex yeniden kullanım: kaynak belirtilir (kısmen doğrulandı) | Yüksek | Bekliyor |
| GDPR (EUR-Lex) | AB resmî metni | EUR-Lex yeniden kullanım: kaynak belirtilir (kısmen doğrulandı) | Orta | Bekliyor |
| KVKK ve ikincil düzenlemeler (mevzuat.gov.tr) | Resmî metin | FSEK md. 31: resmî mevzuat serbest; Kurum rehberleri kapsam dışı olabilir (kısmen doğrulandı) | Yüksek | Bekliyor |
| SPK kripto varlık düzenlemeleri (resmî metinler) | Resmî metin | FSEK md. 31: Resmî Gazete tebliğleri serbest (kısmen doğrulandı) | Orta | Bekliyor |
| IPCC emisyon faktörü veritabanı ve kılavuzları | Kullanım koşulları doğrulanacak | ⚠ **Ticari kullanım kısıtlı:** © IPCC; ticari yeniden kullanım için yazılı izin (kısmen doğrulandı) | Yüksek | Bekliyor |
| GHG Protocol standartları | Kullanım koşulları doğrulanacak | ⚠ **Ticari kullanım kısıtlı:** Kullanım Şartları: ticari, türev ve otomatik veri madenciliği yasak | Orta | Bekliyor |
| ISO standartları (14064, 27001 vb.) | Telifli, ücretli | ⚠ **Ticari kullanım kısıtlı:** ücretli, telifli; ML/AI kullanımı ayrı lisans olmadan yasak (kısmen doğrulandı) | — | Eklenmez, adıyla anılır |

---

## Ekleme sırası

1. Öncelik 0: kendi kaynaklarımız
2. Yüksek öncelikli, lisansı doğrulanmış kaynaklar (Rust, blokzincir, kriptografi, siber güvenlik çerçeveleri, AI güvenliği)
3. İş kolu kaynakları: karbon düzenlemesi ve emisyon faktörleri
4. Orta öncelikliler

Her ekleme turundan sonra değerlendirme seti çalıştırılır ve sonuç kaydedilir.

---

*Yaşayan belge. Kaynak ekleme, çıkarma ve lisans doğrulama sonuçları commit ile kayıt altına alınır.*
