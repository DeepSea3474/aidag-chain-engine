//! local_brain — SoulwareAI'nın EGEMEN yerel beyni (candle, saf Rust).
//! Açık bir modeli (Qwen2.5, GGUF Q4) CPU'da çalıştırır. Ücret YOK, API YOK,
//! kimseye bağımlı DEĞİL. Yarın aynı model GPU ağında koşacak.
//!
//! DÜRÜST: CPU'da küçük model → Claude'dan yavaş ve daha mütevazı, ama EGEMEN.

use candle_core::quantized::gguf_file;
use candle_core::{Device, Tensor};
use candle_transformers::generation::LogitsProcessor;
use candle_transformers::models::quantized_qwen2::ModelWeights;
use tokenizers::Tokenizer;

pub struct LocalBrain {
    model: ModelWeights,
    tokenizer: Tokenizer,
    device: Device,
    eos: Vec<u32>,
    pub model_name: String,
}

impl LocalBrain {
    /// GGUF ağırlık + tokenizer.json'dan yükle. Dosyalar yoksa Err döner (servis
    /// yine ayakta kalır — beyin "yapılandırılmadı" olur, sahte cevap YOK).
    pub fn load(gguf_path: &str, tokenizer_path: &str, model_name: &str) -> anyhow::Result<Self> {
        let device = Device::Cpu;
        let mut file = std::fs::File::open(gguf_path)
            .map_err(|e| anyhow::anyhow!("gguf açılamadı ({gguf_path}): {e}"))?;
        let content = gguf_file::Content::read(&mut file)
            .map_err(|e| anyhow::anyhow!("gguf okunamadı: {e}"))?;
        let model = ModelWeights::from_gguf(content, &mut file, &device)
            .map_err(|e| anyhow::anyhow!("model yüklenemedi: {e}"))?;
        let tokenizer = Tokenizer::from_file(tokenizer_path)
            .map_err(|e| anyhow::anyhow!("tokenizer yüklenemedi ({tokenizer_path}): {e}"))?;
        // Qwen2.5 durdurma token'ları: <|im_end|>=151645, <|endoftext|>=151643
        let eos = vec![151645u32, 151643u32];
        Ok(Self { model, tokenizer, device, eos, model_name: model_name.to_string() })
    }

    /// ChatML şablonuyla üret. system+user → asistan cevabı (metin).
    /// Greedy'e yakın (temp düşük) → tutarlı/deterministik; halüsilasyon savunmasına uygun.
    /// temp=0.0 → GREEDY (deterministik): aynı girdi → aynı çıktı. Ağ doğrulaması
    /// (yedekli worker çıktılarının birebir eşleşmesi) için şart. temp>0 → örnekleme.
    pub fn generate(&mut self, system: &str, user: &str, max_new: usize, temp: f64) -> anyhow::Result<(String, usize)> {
        let prompt = format!(
            "<|im_start|>system\n{system}<|im_end|>\n<|im_start|>user\n{user}<|im_end|>\n<|im_start|>assistant\n"
        );
        let enc = self
            .tokenizer
            .encode(prompt, true)
            .map_err(|e| anyhow::anyhow!("encode: {e}"))?;
        let prompt_tokens: Vec<u32> = enc.get_ids().to_vec();

        // temp=0 → greedy (deterministik, doğrulanabilir); temp>0 → örnekleme.
        let mut sampler = if temp <= 0.0 {
            LogitsProcessor::new(42, None, None)
        } else {
            LogitsProcessor::new(42, Some(temp), Some(0.9))
        };
        let mut all: Vec<u32> = prompt_tokens.clone();
        let mut out: Vec<u32> = Vec::new();
        let mut index_pos = 0usize;

        for step in 0..max_new {
            let (ctx, pos) = if step == 0 {
                (&all[..], 0)
            } else {
                (&all[all.len() - 1..], index_pos)
            };
            let input = Tensor::new(ctx, &self.device)?.unsqueeze(0)?;
            let logits = self.model.forward(&input, pos)?;
            // forward çıktısı modele göre [1,vocab] (son konum) YA DA [1,seq,vocab] olabilir.
            // batch'i çıkar, sonra rank'e göre son konumun vocab vektörünü al.
            let logits = logits.squeeze(0)?;
            let logits = if logits.rank() == 2 {
                let last = logits.dim(0)? - 1;
                logits.get(last)?
            } else {
                logits // zaten [vocab]
            };
            let logits = logits.to_dtype(candle_core::DType::F32)?;
            index_pos += ctx.len();

            let next = sampler.sample(&logits)?;
            if self.eos.contains(&next) {
                break;
            }
            all.push(next);
            out.push(next);
        }

        let text = self
            .tokenizer
            .decode(&out, true)
            .map_err(|e| anyhow::anyhow!("decode: {e}"))?;
        Ok((text.trim().to_string(), out.len()))
    }
}
