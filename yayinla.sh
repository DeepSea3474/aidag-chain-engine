#!/bin/bash
# AIDAG-Chain "yayinla" scripti
# Kullanim: bash yayinla.sh "commit mesaji"
# Yapar: test -> test sayisini README'de guncelle -> commit (DALDA) -> dali push -> saglik
# GUVENLIK:
#  - Testler TAMAMEN yesil DEGILSE DURUR: cargo cikis kodu 0, HER "test result:"
#    satiri "ok. ... 0 failed", hicbir FAILED / error satiri yok.
#  - Yalniz IZLENEN dosyalardaki degisiklikler eklenir (git add -u); izlenmeyen
#    dosyalar (anahtar, .env, durum dosyalari) ASLA otomatik eklenmez.
#  - main'e DOGRUDAN push YOK: degisiklik ayri bir dala commit+push edilir,
#    main'e birlestirme PR ile yapilir.

set -euo pipefail
DIZIN="${DIZIN:-$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)}"
cd "$DIZIN"

MESAJ="${1:-}"
if [ -z "$MESAJ" ]; then
    echo "HATA: commit mesaji gerekli."
    echo "Kullanim: bash yayinla.sh \"ne degistigini anlatan mesaj\""
    exit 1
fi

echo "=== 1/5: TESTLER calisiyor ==="
set +e
TUM_CIKTI=$(cargo test --lib 2>&1)
CARGO_KOD=$?
set -e
SONUC_SATIRLARI=$(printf '%s\n' "$TUM_CIKTI" | grep "test result:" || true)
printf '%s\n' "$SONUC_SATIRLARI"
if [ "$CARGO_KOD" -ne 0 ]; then
    echo "!!! cargo test cikis kodu $CARGO_KOD - push IPTAL."
    exit 1
fi
if [ -z "$SONUC_SATIRLARI" ]; then
    echo "!!! hic 'test result:' satiri yok - push IPTAL."
    exit 1
fi
if printf '%s\n' "$SONUC_SATIRLARI" | grep -vqE "test result: ok\. .* 0 failed"; then
    echo "!!! en az bir test grubu yesil DEGIL - push IPTAL."
    exit 1
fi
if printf '%s\n' "$TUM_CIKTI" | grep -qE "FAILED|^error(\[|:)"; then
    echo "!!! ciktida FAILED/error var - push IPTAL."
    exit 1
fi

# Gercek test sayisi = TUM gruplardaki passed toplami
TEST_SAYISI=$(printf '%s\n' "$SONUC_SATIRLARI" | grep -oE "[0-9]+ passed" | grep -oE "[0-9]+" | awk '{s+=$1} END {print s+0}')
echo "=== 2/5: README test sayisi guncelleniyor ($TEST_SAYISI) ==="
sed -i -E "s/[0-9]+ test (\(engine)/$TEST_SAYISI test \1/g" README.md 2>/dev/null || true
sed -i -E "s/[0-9]+ tests, fmt/$TEST_SAYISI tests, fmt/g" README.md 2>/dev/null || true
echo "README test sayisi -> $TEST_SAYISI"

echo "=== 3/5: degisiklikler ekleniyor (yalniz izlenen dosyalar) ==="
git add -u
IZLENMEYEN=$(git ls-files --others --exclude-standard)
if [ -n "$IZLENMEYEN" ]; then
    echo "UYARI: izlenmeyen dosyalar EKLENMEDI (gerekirse elle 'git add <dosya>'):"
    printf '  %s\n' $IZLENMEYEN
fi
git status --short

echo "=== 4/5: commit + dal push (main'e dogrudan push YOK) ==="
if git diff --cached --quiet; then
    echo "Degisiklik yok, commit atlaniyor."
else
    DAL=$(git rev-parse --abbrev-ref HEAD)
    if [ "$DAL" = "main" ] || [ "$DAL" = "master" ] || [ "$DAL" = "HEAD" ]; then
        DAL="yayin/$(date +%Y%m%d-%H%M%S)"
        git checkout -b "$DAL"
        echo "UYARI: main uzerindeydin -> degisiklik yeni dala alindi: $DAL"
    fi
    git commit -m "$MESAJ"
    git push -u origin "$DAL"
    echo "PUSH TAMAM (dal: $DAL). main'e birlestirmek icin PR ac:"
    echo "  gh pr create --base main --head $DAL"
fi

echo "=== 5/5: SAGLIK KONTROLU ==="
bash saglik-kontrol.sh 2>/dev/null | grep -A1 -E "TESTLER|NODE|GIT" | head -12 || true

echo ""
echo "==================================="
echo "  YAYIN TAMAM (PR bekliyor). Test: $TEST_SAYISI (yesil)"
echo "==================================="
