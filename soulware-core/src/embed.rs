//! embed — SEMANTİK gömme (embedding) motoru: KUBRA'nın grounding'ini keyword'den
//! ANLAM'a taşır. "Türkiye'nin başkenti" ile "siyasi partiler listesi" artık
//! kelime değil ANLAM yakınlığıyla ayrılır → 491+ belgede gürültüsüz retrieval.
//!
//! candle + BERT (saf Rust, CPU, egemen). Model: paraphrase-multilingual-MiniLM-L12
//! (çok-dilli, 384-boyut). Ortalama-havuzlama + L2 normalize → cümle vektörü.
//! DÜRÜST: model yoksa Err → sistem keyword'e düşer, sahte vektör YOK.

use candle_core::{DType, Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config};
use tokenizers::Tokenizer;

pub struct Embedder {
    model: BertModel,
    tokenizer: Tokenizer,
    device: Device,
    pub boyut: usize,
}

impl Embedder {
    /// model_dir içinde config.json + tokenizer.json + model.safetensors bekler.
    pub fn load(model_dir: &str) -> anyhow::Result<Self> {
        let device = Device::Cpu;
        let cfg_bytes = std::fs::read(format!("{model_dir}/config.json"))
            .map_err(|e| anyhow::anyhow!("embed config okunamadı: {e}"))?;
        let config: Config = serde_json::from_slice(&cfg_bytes)
            .map_err(|e| anyhow::anyhow!("embed config parse: {e}"))?;
        let boyut = config.hidden_size;
        let tokenizer = Tokenizer::from_file(format!("{model_dir}/tokenizer.json"))
            .map_err(|e| anyhow::anyhow!("embed tokenizer: {e}"))?;
        let st = format!("{model_dir}/model.safetensors");
        let vb = unsafe {
            VarBuilder::from_mmaped_safetensors(&[st], DType::F32, &device)
                .map_err(|e| anyhow::anyhow!("embed safetensors: {e}"))?
        };
        let model = BertModel::load(vb, &config)
            .map_err(|e| anyhow::anyhow!("embed model yüklenemedi: {e}"))?;
        Ok(Self { model, tokenizer, device, boyut })
    }

    /// Metni tek bir normalize edilmiş vektöre gömer (ortalama-havuzlama).
    pub fn embed(&self, text: &str) -> anyhow::Result<Vec<f32>> {
        let enc = self.tokenizer.encode(text, true).map_err(|e| anyhow::anyhow!("encode: {e}"))?;
        let ids: Vec<u32> = enc.get_ids().to_vec();
        let mask: Vec<u32> = enc.get_attention_mask().to_vec();
        let seq = ids.len();
        let input_ids = Tensor::from_vec(ids, (1, seq), &self.device)?;
        let token_type_ids = input_ids.zeros_like()?;
        let attn = Tensor::from_vec(mask, (1, seq), &self.device)?;

        // BERT ileri: [1, seq, hidden]
        let out = self.model.forward(&input_ids, &token_type_ids, Some(&attn))?;
        // Ortalama-havuzlama (attention mask ağırlıklı).
        let mask_f = attn.to_dtype(DType::F32)?.unsqueeze(2)?; // [1, seq, 1]
        let masked = out.broadcast_mul(&mask_f)?;              // [1, seq, hidden]
        let summed = masked.sum(1)?;                            // [1, hidden]
        let counts = mask_f.sum(1)?;                            // [1, 1]
        let mean = summed.broadcast_div(&counts)?;              // [1, hidden]
        // L2 normalize
        let norm = mean.sqr()?.sum_keepdim(1)?.sqrt()?;        // [1, 1]
        let normed = mean.broadcast_div(&norm)?;                // [1, hidden]
        let v = normed.squeeze(0)?.to_vec1::<f32>()?;
        Ok(v)
    }
}

/// İki normalize vektörün kosinüs benzerliği (nokta çarpım = kosinüs, normalize edilmişse).
pub fn kosinus(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return -1.0;
    }
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}
