#!/bin/bash
# ============================================================================
# calistir.sh — GPU MAKINESINDE tek komutla KUBRA ince ayari.
#   egitim (QLoRA) -> birlestirme -> GGUF -> q4_k_m nicemleme -> cikti/kubra-7b-ince-q4_k_m.gguf
# Gereken: NVIDIA GPU (>=24 GB, orn. L4/A10/L40S/H100), CUDA surucusu, python3, git, cmake.
# Kullanim (GPU makinesinde, bu klasorun icinde):  bash calistir.sh
# Sonra bu sunucuya yalniz GGUF dosyasi kopyalanir (bkz. README.md).
# ============================================================================
set -euo pipefail
cd "$(dirname "$0")"
test -s veri/egitim.jsonl || { echo "HATA: veri/egitim.jsonl yok (hazirla.py calistir)"; exit 1; }

python3 -m venv .venv && . .venv/bin/activate
pip install -q --upgrade pip
pip install -q torch transformers peft accelerate bitsandbytes safetensors sentencepiece gguf
pip freeze > cikti_surumler.txt 2>/dev/null || true   # tekrarlanabilirlik kaydi

mkdir -p cikti
python egit.py egit 2>&1 | tee cikti/egitim.log
python egit.py birlestir 2>&1 | tee -a cikti/egitim.log

# GGUF donusumu + nicemleme (llama.cpp)
if [ ! -d llama.cpp ]; then git clone --depth 1 https://github.com/ggml-org/llama.cpp; fi
pip install -q -r llama.cpp/requirements/requirements-convert_hf_to_gguf.txt || true
python llama.cpp/convert_hf_to_gguf.py cikti/birlesik --outtype f16 --outfile cikti/kubra-7b-ince-f16.gguf
cmake -S llama.cpp -B llama.cpp/build -DGGML_CUDA=OFF >/dev/null && cmake --build llama.cpp/build --target llama-quantize -j >/dev/null
llama.cpp/build/bin/llama-quantize cikti/kubra-7b-ince-f16.gguf cikti/kubra-7b-ince-q4_k_m.gguf Q4_K_M
sha256sum cikti/kubra-7b-ince-q4_k_m.gguf | tee cikti/kubra-7b-ince-q4_k_m.gguf.sha256
echo "BITTI: cikti/kubra-7b-ince-q4_k_m.gguf  (egitim ozeti: cikti/egitim.log)"
