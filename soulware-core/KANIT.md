# KUBRA zincir kaniti

KUBRA her cevap icin zincire **yalniz 32 baytlik bir hash** yazar (tip=1 Record,
33 bayt). Soru, cevap ve tuz zincire girmez.

## Semalar

| Sema | Formul | Kim |
|---|---|---|
| eski-tuzsuz | `blake3(girdi)` | Bu surumden onceki tum kayitlar |
| tuzlu-v1 | `blake3_keyed(tuz, girdi)` | Bu surumden sonraki kayitlar |

`girdi = net_id (u32 LE) | ts (u64 LE) | alan1 0x1e alan2 0x1e ...`

| Uc | Alanlar (eski) | Alanlar (tuzlu-v1) |
|---|---|---|
| `/v1/ask` (beyin veya arac) | prompt, answer, model | prompt, answer, model |
| `/v1/ask-stream` (beyin) | prompt, answer | prompt, answer, model |
| `/v1/ask-stream` (arac) | prompt, answer, model | prompt, answer, model |
| `/v1/image`, `/v1/video` | prompt, [wallet], icerik baytlari | ayni |

`tuz` = 32 bayt, isletim sistemi CSPRNG'si (`OsRng`), her kayit icin yeni.
Yalniz yanitta doner (`salt` ve `ts` alanlari; gorsel/video icin
`x-kubra-salt` ve `x-kubra-ts` basliklari). KUBRA tuzu saklamaz ve loglamaz:
**tuz kaybolursa kayit dogrulanamaz.**

Neden: metni tahmin edilebilen etkilesimler (kisa soru + sabit arac cevabi)
tuzsuz hash'ten dene-yanil ile bulunabiliyordu. Tuzla, ayni soru her seferinde
farkli bir hash uretir ve tuzu bilmeyen biri icerigi tahmin edemez.

## Dogrulama

```
POST /v1/verify
{"ts": 1758650000, "prompt": "...", "answer": "...", "model": "...", "salt": "<64 hex>", "proof_hash": "<ops.>"}
```

- `salt` verilirse tuzlu-v1, verilmezse eski-tuzsuz sema kullanilir; eski kayitlar
  aynen dogrulanir. Tuzlu kayitta `model` zorunludur.
- Yanit: `proof_hash`, `sema`, `zincirde`, `kaydeden`, `kubra_imzali`, `zaman`,
  `dogrulandi` (zincirde + KUBRA imzali + verilen proof_hash ile eslesir).
- Eski yanitlarda `ts` yoktu; kaydin zamani `/belge/<proof_hash>` yanitindaki
  `zaman` alanidir (vertex zamani = hash'e giren ts).

Bagimsiz dogrulama (KUBRA'ya guvenmeden), Python:

```python
import blake3, struct
girdi = struct.pack('<I', 3474) + struct.pack('<Q', ts) + b'\x1e'.join([prompt.encode(), answer.encode(), model.encode()])
h = blake3.blake3(girdi, key=bytes.fromhex(salt)).hexdigest()   # eski kayit: blake3.blake3(girdi)
# sonra: GET /rpc/belge/<h>  -> "kayitli": true
```

Sabit test vektorleri: `soulware-core/src/kanit.rs` (`eski_sema_sabit_vektor`,
`tuzlu_sema_sabit_vektor`).
