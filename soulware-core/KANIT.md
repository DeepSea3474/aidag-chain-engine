# KUBRA zincir kaniti

KUBRA her cevap icin zincire **yalniz 32 baytlik bir hash** yazar (tip=1 Record,
33 bayt). Soru, cevap, istemci baglami ve tuz zincire girmez.

## Semalar

| Sema | Formul | Kim |
|---|---|---|
| eski-tuzsuz | `blake3(girdi_v1)` | Tuz oncesi kayitlar (yalniz dogrulanir) |
| tuzlu-v1 | `blake3_keyed(tuz, girdi_v1)` | v2 oncesi tuzlu kayitlar (yalniz dogrulanir) |
| **tuzlu-v2** | `blake3_keyed(tuz, girdi_v2)` | **Tum yeni kayitlar** |

```
girdi_v1 = net_id (u32 LE) | ts (u64 LE) | alan1 0x1e alan2 0x1e ...
girdi_v2 = "KUBRA-KANIT-v2\0"
         | L(net_id u32 LE) | L(ts u64 LE) | L(tur)
         | alan_sayisi (u64 LE) | L(alan1) | L(alan2) | ...
L(x)     = uzunluk(x) (u64 LE) | x
```

**Neden v2:** v1/eski semada alanlar yalniz `0x1e` ile ayrilir, uzunluk oneki yoktur.
Bir alan `0x1e` iceriyorsa ayni bayt dizisi farkli bir (prompt, answer, model)
bolunmesine karsilik gelir; ornegin `prompt="P\x1eQ", answer="A"` kaydi
`prompt="P", answer="Q\x1eA"` diye de "dogrulanabiliyordu". v2'de her alan
uzunluk onekli, alan sayisi ve `tur` hash'e girer ve girdi v2'ye ozel etiketle
baslar: bolunme tektir, v2 hash'i v1/eski hash'iyle ve sohbet kaydi medya
kaydiyla karismaz.

| Uc | tur | Alanlar (tuzlu-v2) | Alanlar (v1 / eski) |
|---|---|---|---|
| `/v1/ask` (beyin veya arac) | `sohbet` | prompt, answer, model, context | prompt, answer, model |
| `/v1/ask-stream` (beyin veya arac) | `sohbet` | prompt, answer, model, context | v1: prompt, answer, model · eski beyin: prompt, answer |
| `/v1/image`, `/v1/video` | `medya` | prompt, wallet (yoksa bos), icerik baytlari | prompt, [wallet], icerik |

`context` = istemcinin `/v1/ask` govdesinde verdigi `context` alani **aynen**
(verilmediyse bos). Verildiyse yanitta `"istemci_baglami": true` doner ve
dogrulamada ayni `context` verilmelidir.

Yanitlarda `"sema": "tuzlu-v2"` doner (gorsel/video: `x-kubra-sema` basligi).
`tuz` = 32 bayt, isletim sistemi CSPRNG'si (`OsRng`), her kayit icin yeni.
Yalniz yanitta doner (`salt`, `ts`; gorsel/video icin `x-kubra-salt`, `x-kubra-ts`).
KUBRA tuzu saklamaz ve loglamaz: **tuz kaybolursa kayit dogrulanamaz.**

## Dogrulama

```
POST /v1/verify
{"ts": 1758650000, "prompt": "...", "answer": "...", "model": "...", "salt": "<64 hex>",
 "context": "<ops.>", "sema": "<ops.: tuzlu-v2 | tuzlu-v1 | eski-tuzsuz>", "proof_hash": "<ops.>"}
```

- `sema` verilmezse: `salt` varsa once **tuzlu-v2**, sonra **tuzlu-v1** denenir;
  `salt` yoksa **eski-tuzsuz**. Tuzlu semalarda `model` zorunludur.
- **Belirsizlik kurali:** v1/eski semada `prompt`, `answer` veya `model` alanlarindan
  biri `0x1e` iceriyorsa yanit `"belirsiz": true`, `"sebep": ...` ve
  `"dogrulandi": false` olur (kayit zincirde olsa bile). v2 bundan etkilenmez.
- v1/eski kayitta `context` hash'e girmez; `context` verildiyse yanitta `uyari` doner.
- Yanit: `proof_hash`, `sema`, `denenen_semalar`, `zincirde`, `kaydeden`,
  `kubra_imzali`, `zaman`, `belirsiz`, `dogrulandi`
  (zincirde + KUBRA imzali + verilen proof_hash ile eslesir + belirsiz degil).

Bagimsiz dogrulama (KUBRA'ya guvenmeden), Python:

```python
import blake3, struct
L = lambda b: struct.pack('<Q', len(b)) + b
alanlar = [prompt.encode(), answer.encode(), model.encode(), context.encode()]   # context yoksa b""
girdi = (b"KUBRA-KANIT-v2\0" + L(struct.pack('<I', 3474)) + L(struct.pack('<Q', ts)) + L(b"sohbet")
         + struct.pack('<Q', len(alanlar)) + b"".join(L(a) for a in alanlar))
h = blake3.blake3(girdi, key=bytes.fromhex(salt)).hexdigest()
# v1 kayit:   blake3.blake3(struct.pack('<I',3474)+struct.pack('<Q',ts)+b'\x1e'.join([p,a,m]), key=tuz)
# sonra: GET /rpc/belge/<h>  -> "kayitli": true
```

Sabit test vektorleri: `soulware-core/src/kanit.rs` (`eski_sema_sabit_vektor`,
`tuzlu_sema_sabit_vektor`, `v2_sabit_vektor`).
