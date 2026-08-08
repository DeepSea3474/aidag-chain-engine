# KUBRA — GPU Dağıtımı (asıl model sıçraması)

CPU'da yazılım kaldıraçları (semantik retrieval + araç + seed) KUBRA'yı **8/9**'a
taşıdı. Kalan boşluk (derin muhakeme, kirli-metin çıkarımı) **model boyutu** meselesi
= **GPU**. Kod GPU-hazır: `cuda_if_available` ile GPU varsa otomatik kullanır.

## Neden GPU
- 1.5B/CPU ~1 token/sn. GPU'da büyük model **20-50 token/sn** → cevap saniyeler içinde.
- Büyük açık model (Qwen2.5-32B/72B, DeepSeek-R1-Distill-32B) → muhakeme + çıkarım büyük sıçrama.
- DeepSeek testi kanıtladı: başka bir 1.5B çözmüyor; **boyut** gerekiyor.

## Donanım seçenekleri (kaynak kararı — SENİN)
| Yol | Maliyet | Not |
|---|---|---|
| **Kiralık bulut GPU** | ~$0.4–2/saat (RTX 4090 / A100) | Hızlı başlangıç; RunPod/Vast.ai/Lambda. Test/üretim. |
| **Kendi GPU'n** | tek seferlik donanım | RTX 3090/4090 (24GB) → 32B Q4 rahat. |
| **Katkı GPU'ları (DePIN)** | $0 (ağ büyüyünce) | Asıl vizyon; worker'lar GPU verir. Uzun vade. |

Model→VRAM (Q4): 7B ~5GB · 14B ~9GB · 32B ~20GB · 72B ~40GB.

## Kurulum (GPU sunucuda)
```bash
# 1. NVIDIA sürücü + CUDA toolkit kurulu olmalı (nvidia-smi çalışmalı)
# 2. GPU derlemesi
cd /root/aidag-lsc
cargo build --release --features cuda -p soulware-core

# 3. Büyük model indir (örn. Qwen2.5-32B-Instruct Q4, ~20GB — 24GB GPU)
#    veya DeepSeek-R1-Distill-Qwen-32B (muhakeme) — registry.json'da listeli
# HF GGUF: Qwen/Qwen2.5-32B-Instruct-GGUF (veya bartowski)

# 4. Çalıştır (systemd unit'te SOULWARE_LOCAL_MODEL'i büyük GGUF'a çevir)
SOULWARE_LOCAL_MODEL=/path/qwen2.5-32b-instruct-q4_k_m.gguf \
SOULWARE_LOCAL_TOKENIZER=/path/tokenizer.json \
SOULWARE_MAX_TOKENS=512 \
  ./target/release/soulware-core
```

## Beklenen
- Cevaplar **saniyeler** (dakikalar değil).
- Muhakeme/çıkarım büyük atlama → benchmark 8/9 → daha yükseğe.
- Worker'lar (GPU) öz-kıyaslamada **yüksek tier** alır → zor işleri onlar alır (kurulu sistem).

## DePIN bağlantısı
GPU worker'lar (katkıcılar) büyük modeli koşar → yüksek tier → koordinatör zor işleri
onlara yönlendirir → katkıcılar LSC kazanır → ağ zekâsı GPU'larla büyür. Kod hazır.
