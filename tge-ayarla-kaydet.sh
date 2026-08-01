#!/bin/bash
# ============================================================================
# tge-ayarla-kaydet.sh — OWNER: ON-SATIS TGE'yi ayarla (tek komut, restart YOK).
#
# Listeleme tarihi netlesince calistir; on-satis claim/vesting o andan acilir.
# Kullanim:
#   ./tge-ayarla-kaydet.sh <TGE>
#     TGE: "2026-08-26" gibi tarih (UTC 00:00) VEYA dogrudan unix saniye
#
# Ortam: RPC (varsayilan localhost:8645), NET (3474 mainnet), KEY (aidag-kurucu.key),
#        BIN (./target/release/tge-ayarla)
# GUVENLIK: imza offline binary'de; anahtar bu makinede kalir.
# ============================================================================
set -euo pipefail
RPC="${RPC:-http://127.0.0.1:8645}"
NET="${NET:-3474}"
KEY="${KEY:-aidag-kurucu.key}"
BIN="${BIN:-./target/release/tge-ayarla}"
IN="${1:?TGE tarihi (YYYY-MM-DD) veya unix saniye gerekli}"

if [ ! -x "$BIN" ]; then echo "HATA: $BIN yok. cargo build --release --bin tge-ayarla" >&2; exit 1; fi
# tarih -> unix (rakamsa dogrudan unix say)
if [[ "$IN" =~ ^[0-9]+$ ]]; then TGE="$IN"; else TGE=$(date -u -d "$IN 00:00:00" +%s); fi
echo "TGE ayarlaniyor: '$IN' -> unix $TGE ($(date -u -d @$TGE '+%Y-%m-%d %H:%M UTC'))"

TIPS=$(curl -s -m5 "$RPC/tips" | python3 -c "import sys,json;print(','.join(json.load(sys.stdin).get('tips',[])) or '-')")
HEX=$("$BIN" "$KEY" "$NET" "$TGE" "$(date +%s)" "$TIPS")
RESP=$(curl -s -m10 -X POST "$RPC/submit" -H 'Content-Type: application/json' -d "{\"hex\":\"$HEX\"}")
echo "$RESP"
if echo "$RESP" | grep -q '"ok":true'; then
  echo "TGE AYARLANDI ✓  (unix $TGE). On-satis claim/vesting bu andan acilir."
else
  echo "HATA: gonderim reddedildi." >&2; exit 1
fi
