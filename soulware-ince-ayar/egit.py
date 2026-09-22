#!/usr/bin/env python3
"""KUBRA ince ayar (QLoRA) — Qwen2.5-7B-Instruct (Apache-2.0) + veri/egitim.jsonl.

GPU makinesinde calisir (calistir.sh cagirir). Adimlar:
  1. Taban model 4-bit (QLoRA) yuklenir, LoRA adaptoru eklenir.
  2. Kayip YALNIZ asistan cevaplari uzerinden hesaplanir (sistem/kullanici maskeli).
  3. Her epoch sonunda dogrulama kaybi olculur; en iyi adaptor saklanir.
  4. Adaptor, bf16 taban modelle BIRLESTIRILIR -> cikti/birlesik (HF formati).
Surum bagimsizligi icin yalniz transformers.Trainer + peft kullanilir (trl yok).
"""
import json, math, os, sys
import torch
from transformers import (AutoModelForCausalLM, AutoTokenizer, BitsAndBytesConfig,
                          Trainer, TrainingArguments)
from peft import LoraConfig, PeftModel, get_peft_model, prepare_model_for_kbit_training

TABAN = os.environ.get("TABAN_MODEL", "Qwen/Qwen2.5-7B-Instruct")
KOK = os.path.dirname(os.path.abspath(__file__))
CIKTI = os.path.join(KOK, "cikti")
MAKS = int(os.environ.get("MAKS_TOKEN", "1024"))
EPOCH = float(os.environ.get("EPOCH", "3"))

tok = AutoTokenizer.from_pretrained(TABAN)
if tok.pad_token is None:
    tok.pad_token = tok.eos_token


def kodla(ornek):
    """Sohbet sablonuyla tokenle; yalniz asistan turlari etiket, gerisi -100."""
    msg = ornek["messages"]
    ids, etiket = [], []
    for i in range(len(msg)):
        onceki = tok.apply_chat_template(msg[:i], tokenize=False) if i else ""
        simdiki = tok.apply_chat_template(msg[: i + 1], tokenize=False)
        parca = tok(simdiki[len(onceki):], add_special_tokens=False)["input_ids"]
        ids += parca
        etiket += parca if msg[i]["role"] == "assistant" else [-100] * len(parca)
    return {"input_ids": ids[:MAKS], "labels": etiket[:MAKS]}


def yukle(ad):
    return [kodla(json.loads(l)) for l in open(os.path.join(KOK, "veri", ad))]


class Veri(torch.utils.data.Dataset):
    def __init__(self, xs): self.xs = [x for x in xs if any(e != -100 for e in x["labels"])]
    def __len__(self): return len(self.xs)
    def __getitem__(self, i): return self.xs[i]


def harmanla(grup):
    n = max(len(x["input_ids"]) for x in grup)
    pad = lambda v, d: v + [d] * (n - len(v))
    return {"input_ids": torch.tensor([pad(x["input_ids"], tok.pad_token_id) for x in grup]),
            "labels": torch.tensor([pad(x["labels"], -100) for x in grup]),
            "attention_mask": torch.tensor([pad([1] * len(x["input_ids"]), 0) for x in grup])}


def egit():
    egitim, dogrulama = Veri(yukle("egitim.jsonl")), Veri(yukle("dogrulama.jsonl"))
    print(f"egitim={len(egitim)} dogrulama={len(dogrulama)} taban={TABAN}", flush=True)
    model = AutoModelForCausalLM.from_pretrained(
        TABAN, device_map="auto", torch_dtype=torch.bfloat16,
        quantization_config=BitsAndBytesConfig(load_in_4bit=True, bnb_4bit_quant_type="nf4",
                                               bnb_4bit_compute_dtype=torch.bfloat16,
                                               bnb_4bit_use_double_quant=True))
    model = prepare_model_for_kbit_training(model)
    model = get_peft_model(model, LoraConfig(
        r=16, lora_alpha=32, lora_dropout=0.05, task_type="CAUSAL_LM",
        target_modules=["q_proj", "k_proj", "v_proj", "o_proj", "gate_proj", "up_proj", "down_proj"]))
    model.print_trainable_parameters()
    args = TrainingArguments(
        output_dir=os.path.join(CIKTI, "adaptor"), num_train_epochs=EPOCH,
        per_device_train_batch_size=4, per_device_eval_batch_size=4, gradient_accumulation_steps=4,
        learning_rate=1e-4, lr_scheduler_type="cosine", warmup_ratio=0.05, bf16=True,
        logging_steps=10, eval_strategy="epoch", save_strategy="epoch", save_total_limit=1,
        load_best_model_at_end=True, metric_for_best_model="eval_loss", greater_is_better=False,
        report_to=[], gradient_checkpointing=True, remove_unused_columns=False, seed=3474)
    t = Trainer(model=model, args=args, train_dataset=egitim, eval_dataset=dogrulama, data_collator=harmanla)
    t.train()
    son = t.evaluate()
    print(f"EN IYI dogrulama kaybi={son['eval_loss']:.4f} (perplexity={math.exp(son['eval_loss']):.2f})", flush=True)
    t.model.save_pretrained(os.path.join(CIKTI, "adaptor", "en_iyi"))


def birlestir():
    taban = AutoModelForCausalLM.from_pretrained(TABAN, torch_dtype=torch.bfloat16, device_map="cpu")
    m = PeftModel.from_pretrained(taban, os.path.join(CIKTI, "adaptor", "en_iyi")).merge_and_unload()
    hedef = os.path.join(CIKTI, "birlesik")
    m.save_pretrained(hedef, safe_serialization=True)
    tok.save_pretrained(hedef)
    print("birlesik model:", hedef, flush=True)


if __name__ == "__main__":
    {"egit": egit, "birlestir": birlestir}[sys.argv[1]]()
