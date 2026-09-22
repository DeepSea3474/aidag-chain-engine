# KUBRA İnce Ayar (QLoRA, Qwen2.5-7B-Instruct)

Amaç: KUBRA'nın sesi — sıcak, samimi, dürüst, **'sen'** hitabı, düzgün Türkçe, yarım cümle yok,
**uydurmama**. Lisans ilkeleri: [LISANS_RAPORU.md](LISANS_RAPORU.md).

## Adımlar
1. **Veri (bu sunucuda, bedava):**
   ```
   python3 aya_tr_indir.py          # Aya Türkçe satırları (kaldığı yerden devam eder)
   python3 kubra_kisilik_uret.py    # el yazımı KUBRA diyalogları
   python3 hazirla.py               # temizlik + veri/egitim.jsonl, veri/dogrulama.jsonl
   ```
2. **Başlangıç ölçümü (bu sunucuda):** `python3 degerlendir.py once-taban` → `olcum-once-taban.json`
3. **Eğitim (GPU makinesinde, ≥24 GB VRAM):** bu klasörü (ham/ ve .venv hariç) kopyala, sonra
   `bash calistir.sh` → `cikti/kubra-7b-ince-q4_k_m.gguf` (+ `.sha256`, `egitim.log`).
   Makineyi açarken SSH anahtarını cloud-init ile ver (kullanıcı `kubra`).
4. **Deneme (bu sunucuda, canlıyı bozmadan):** GGUF'u `soulware-models/` altına kopyala, ayrı bir
   portta dene: `llama-server -m soulware-models/kubra-7b-ince-q4_k_m.gguf --port 18651 -t 16 -c 4096 --jinja`
   ve `python3 degerlendir.py sonra-ince http://127.0.0.1:18651/v1/chat/completions`.
5. **Karar:** Ölçüm her ölçütte daha iyi (siz↓, yarım↓, uydurma↓, bilgi↑) ve canlı `/v1/ask` testleri
   (AIDAG araçları, belge hash, kişilik soruları) geriye gitmediyse `kubra-llama.service` modeli
   değiştirilir. Eski model dosyası silinmez (geri dönüş).

## Başlangıç ölçümü (2026-09-22, taban 7B, araçsız)
siz 10/20 · yarım 4/20 · uydurma 4/5 · AIDAG bilgisi 0/5 → `olcum-once-taban.json`
