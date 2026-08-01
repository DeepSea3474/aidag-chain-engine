#!/bin/bash
# ============================================================================
# on-satis-kaydet.sh — OWNER: bir odeme icin TAHSIS (tip=10) kaydet (tek komut).
#
# Kullanim:
#   ./on-satis-kaydet.sh <alici_0x> <aidag> <lsc_hediye> <odeme_ref>
#     alici_0x   : alicinin AIDAG-Chain (0x) adresi
#     aidag      : TAM AIDAG miktari (odemeye karsilik)
#     lsc_hediye : TAM LSC hediye (claim gazi icin; 0 olabilir)
#     odeme_ref  : BENZERSIZ odeme referansi (u64) — cifte-tahsis kilidi
#
# Ortam (opsiyonel):
#   RPC  node RPC (varsayilan http://127.0.0.1:8645)
#   NET  network_id: 1=devnet, 3474=mainnet (varsayilan 1)
#   KEY  kurucu anahtar dosyasi (varsayilan aidag-kurucu.key)
#   BIN  imzalayici binary (varsayilan ./target/release/on-satis-tahsis)
#
# GUVENLIK: imzalama OFFLINE binary'de olur; anahtar bu makinede kalir.
# Owner, BSC'de odemeyi (kurucu adrese) DOGRULADIKTAN sonra bunu calistirir.
# ============================================================================
set -euo pipefail
RPC="${RPC:-http://127.0.0.1:8645}"
NET="${NET:-1}"
KEY="${KEY:-aidag-kurucu.key}"
BIN="${BIN:-./target/release/on-satis-tahsis}"

ALICI="${1:?alici_0x gerekli}"
AIDAG="${2:?aidag gerekli}"
LSC="${3:?lsc_hediye gerekli}"
REF="${4:?odeme_ref gerekli}"

if [ ! -x "$BIN" ]; then echo "HATA: $BIN yok. Once: cargo build --release --bin on-satis-tahsis" >&2; exit 1; fi
if [ ! -f "$KEY" ]; then echo "HATA: $KEY yok (kurucu anahtar)" >&2; exit 1; fi

# Cifte-tahsis on-kontrol: bu ref daha once kullanilmis mi? (bulundu:true = kullanilmis)
if curl -s -m5 "$RPC/on-satis/$REF" | grep -q '"bulundu":true'; then
  echo "UYARI: odeme_ref=$REF ZATEN kullanilmis. Cifte-tahsis engellenir. Farkli ref ver." >&2
  exit 1
fi

TIPS=$(curl -s -m5 "$RPC/tips" | python3 -c "import sys,json; print(','.join(json.load(sys.stdin).get('tips',[])) or '-')")
NOW=$(date +%s)
HEX=$("$BIN" "$KEY" "$NET" "$ALICI" "$AIDAG" "$LSC" "$REF" "$NOW" "$TIPS")

echo "Zincire gonderiliyor ($RPC/submit)..."
RESP=$(curl -s -m10 -X POST "$RPC/submit" -H 'Content-Type: application/json' -d "{\"hex\":\"$HEX\"}")
echo "$RESP"
if echo "$RESP" | grep -q '"ok":true'; then
  echo "TAHSIS KAYDEDILDI ✓  Alici goruntule: $RPC/on-satis-tahsis/${ALICI#0x}"
else
  echo "HATA: gonderim reddedildi. Yukaridaki yaniti kontrol et." >&2
  exit 1
fi
