#!/bin/bash
# Güvenlik istisnalarının gözden geçirme tarihi. Süre dolduysa CI KIRMIZI (zorunlu yeniden değerlendirme).
SON="2026-12-29"
BUGUN="$(date +%Y-%m-%d)"
if [[ "$BUGUN" > "$SON" ]]; then
  echo "❌ Güvenlik istisnalarının gözden geçirme tarihi ($SON) GEÇTİ. deny.toml + .cargo/audit.toml istisnalarını yeniden değerlendirin."
  exit 1
fi
echo "✓ Güvenlik istisnaları geçerli (gözden geçirme son tarihi: $SON, bugün: $BUGUN)."
