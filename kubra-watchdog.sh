#!/bin/bash
# KUBRA beyin watchdog'u — /v1/ask donarsa (deadlock) soulware-kubra'yı otomatik yeniden başlatır.
# Normal cevap ~90s sürdüğü için timeout bunun üstünde (160s). 2 ardışık başarısızlık → restart.
set -u
URL="http://127.0.0.1:8646/v1/ask"
PROBE_TIMEOUT=160     # tek deneme üst sınırı (normal ~90s'in üstünde)
INTERVAL=600          # denemeler arası (10 dk) — chain'e az yük
FAIL_LIMIT=2          # kaç ardışık başarısızlıkta restart
POST_RESTART=120      # restart sonrası model yüklenmesi için bekle

fails=0
echo "$(date -Is) watchdog başladı (interval=${INTERVAL}s, timeout=${PROBE_TIMEOUT}s)"
while true; do
  resp=$(curl -s --max-time "$PROBE_TIMEOUT" -X POST "$URL" \
         -H "Content-Type: application/json" -d '{"prompt":"ping"}' 2>/dev/null)
  if echo "$resp" | grep -q '"answer"'; then
    [ "$fails" -gt 0 ] && echo "$(date -Is) tekrar OK (önce $fails başarısızlık)"
    fails=0
  else
    fails=$((fails+1))
    echo "$(date -Is) BAŞARISIZ ($fails/$FAIL_LIMIT) resp='${resp:0:70}'"
    if [ "$fails" -ge "$FAIL_LIMIT" ]; then
      echo "$(date -Is) >>> DEADLOCK — soulware-kubra yeniden başlatılıyor"
      systemctl restart soulware-kubra
      fails=0
      sleep "$POST_RESTART"
    fi
  fi
  sleep "$INTERVAL"
done
