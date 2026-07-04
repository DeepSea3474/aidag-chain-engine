#!/bin/bash
# AIDAG-Chain "yayinla" scripti
# Kullanim: bash yayinla.sh "commit mesaji"
# Yapar: test -> test sayisini README'de guncelle -> commit -> push -> saglik
# GUVENLIK: testler yesil DEGILSE DURUR (bozuk kod push edilmez).

set -e
DIZIN="/root/aidag-lsc"
cd "$DIZIN"

MESAJ="$1"
if [ -z "$MESAJ" ]; then
    echo "HATA: commit mesaji gerekli."
    echo "Kullanim: bash yayinla.sh \"ne degistigini anlatan mesaj\""
    exit 1
fi

echo "=== 1/5: TESTLER calisiyor ==="
TEST_CIKTI=$(cargo test --lib 2>&1 | grep "test result:" | head -1)
echo "$TEST_CIKTI"
if ! echo "$TEST_CIKTI" | grep -q "0 failed"; then
    echo "!!! TESTLER YESIL DEGIL - push IPTAL. Once testleri duzelt."
    exit 1
fi

# Gercek test sayisini cikar (passed sayisi)
TEST_SAYISI=$(echo "$TEST_CIKTI" | grep -oE "[0-9]+ passed" | grep -oE "[0-9]+")
echo "=== 2/5: README test sayisi guncelleniyor ($TEST_SAYISI) ==="
# "NNN test" ve "NNN tests" kaliplarini guncelle (mekanik)
sed -i -E "s/[0-9]+ test (\(engine)/$TEST_SAYISI test \1/g" README.md 2>/dev/null || true
sed -i -E "s/[0-9]+ tests, fmt/$TEST_SAYISI tests, fmt/g" README.md 2>/dev/null || true
echo "README test sayisi -> $TEST_SAYISI"

echo "=== 3/5: degisiklikler ekleniyor ==="
git add -A
git status --short

echo "=== 4/5: commit + push ==="
if git diff --cached --quiet; then
    echo "Degisiklik yok, commit atlaniyor."
else
    git commit -m "$MESAJ"
    git push origin main
    echo "PUSH TAMAM."
fi

echo "=== 5/5: SAGLIK KONTROLU ==="
bash saglik-kontrol.sh 2>/dev/null | grep -A1 -E "TESTLER|NODE|GIT" | head -12

echo ""
echo "==================================="
echo "  YAYIN TAMAM. Test: $TEST_SAYISI (yesil)"
echo "==================================="
