# KUBRA İnce Ayar — Lisans Raporu

Kontrol tarihi: **2026-09-22**. Lisanslar tahminle değil, kaynağın kendi kaydından
(Hugging Face API `cardData.license`) okundu. İlke: **başımıza iş açabilecek hiçbir içerik yok** —
yalnız proje malı içerik + izin verici (ticari kullanım serbest) lisanslar.

## Kullanılan
| Kaynak | Lisans | Ticari | Not |
|---|---|---|---|
| `kubra_kisilik.jsonl` (el yazımı, 43 diyalog) | Proje malı | ✅ | KUBRA'nın sesi; eğitimde 5x ağırlık |
| [Qwen/Qwen2.5-7B-Instruct](https://huggingface.co/Qwen/Qwen2.5-7B-Instruct) (taban model) | Apache-2.0 | ✅ | İnce ayarlı türev de Apache koşullarında bizim |
| [CohereLabs/aya_dataset](https://huggingface.co/datasets/CohereLabs/aya_dataset) — yalnız Türkçe (4.046 satır) | Apache-2.0 | ✅ | İnsan yazımı |
| [OpenAssistant/oasst2](https://huggingface.co/datasets/OpenAssistant/oasst2) — yalnız Türkçe | Apache-2.0 | ✅ | Yalnız 9 Türkçe ağaç |

## Kullanılmayan (bilinçli olarak elendi)
| Kaynak | Lisans | Neden |
|---|---|---|
| facebook/empathetic_dialogues | CC-BY-NC-4.0 | Ticari kullanım YASAK |
| databricks/databricks-dolly-15k | CC-BY-SA-3.0 | "Aynı lisansla paylaş" yükümlülüğü; Türkçe yok |
| HuggingFaceH4/ultrachat_200k | MIT | İçerik ChatGPT çıktısı — OpenAI koşulları rakip model eğitimini kısıtlar |
| Qwen/Qwen2.5-72B-Instruct | Qwen lisansı ("other") | Koşullu ticari; ince ayar 7B (Apache) üzerinde |

## Ek temizlik (Apache lisanslı sette bile)
`hazirla.py` şu örnekleri ATAR:
- **Telif riski:** şarkı sözü, şiir, "tüm hakları saklıdır", ©/copyright işaretli; kısa satırlardan oluşan şiir/şarkı biçimli metinler.
- **Alıntı metin:** uzun bir metin parçası yapıştırıp soru soran örnekler (parça çoğunlukla Wikipedia/haber kaynaklı olabilir → CC-BY-SA/telif belirsizliği).
- **Kişisel veri:** e-posta, telefon, 11 haneli kimlik benzeri numaralar.
- Link ağırlıklı, aşırı kısa/uzun ve tekrar eden örnekler.

Değişken bilgiler (fiyat, kademe, TGE, satılan miktar) eğitim verisine **bilinçli olarak yazılmaz**;
KUBRA bunları canlı araçtan okur. Yalnız değişmeyen olgular (Chain ID 3474, 21.000.000 arz, Rust, GHOSTDAG) öğretilir.
