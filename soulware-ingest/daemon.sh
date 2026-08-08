#!/usr/bin/env bash
# soulware-learn daemon — KUBRA SÜREKLI ARKA-PLAN ÖĞRENME.
# Wikipedia'dan (açık, CC BY-SA) partiler halinde bilgi çeker, korpusa ekler.
# Offset kalıcı (.data/soulware-learn-offset) → kaldığı yerden devam eder (reboot/çökme sonrası).
# Yumuşak tavan: korpus MAX belgeye ulaşınca durur (bellek-içi depo ölçek sınırı; indeks sonra).
set -u
KB=${SOULWARE_KB_URL:-http://127.0.0.1:8646}
CONFIG=${SOULWARE_WIKI_CONFIG:-20231101.tr}
BATCH=${SOULWARE_LEARN_BATCH:-40}
SLEEP=${SOULWARE_LEARN_SLEEP:-600}
MAX_DOCS=${SOULWARE_LEARN_MAX:-5000}
DATA=/root/aidag-lsc/.data
STATE="$DATA/soulware-learn-offset"
LOG="$DATA/soulware-learn.log"
mkdir -p "$DATA"
offset=$(cat "$STATE" 2>/dev/null || echo 0)

echo "[$(date -u +%FT%TZ)] soulware-learn basladi: config=$CONFIG batch=$BATCH sleep=${SLEEP}s max=$MAX_DOCS offset=$offset" >> "$LOG"
while true; do
  # KUBRA hazir mi bekle (boot sirasi / restart toleransi).
  until curl -s --max-time 5 "$KB/health" >/dev/null 2>&1; do sleep 15; done
  # Yumusak tavan: korpus doluysa bekle (ölçek indeksi gelene kadar sismesin).
  n=$(curl -s --max-time 8 "$KB/kb/stats" 2>/dev/null | grep -oE '"belge_sayisi":[0-9]+' | grep -oE '[0-9]+' || echo 0)
  if [ "${n:-0}" -ge "$MAX_DOCS" ]; then
    echo "[$(date -u +%FT%TZ)] tavan ($n/$MAX_DOCS) — bekliyor" >> "$LOG"
    sleep "$SLEEP"; continue
  fi
  python3 /root/aidag-lsc/soulware-ingest/ingest.py --kb "$KB" --config "$CONFIG" --count "$BATCH" --offset "$offset" >> "$LOG" 2>&1
  offset=$((offset + BATCH + 15))   # atlananları telafi için biraz ileri
  echo "$offset" > "$STATE"
  sleep "$SLEEP"
done
