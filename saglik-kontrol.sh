#!/bin/bash
# AIDAG-Chain Saglik Kontrol Scripti
# Kullanim: bash saglik-kontrol.sh
# Sistemi tarar, sorunlari + DUZELTME KOMUTLARINI gosterir.
# Hicbir seyi kendi degistirmez - sadece teshis + oneri. Sen calistirirsin.

DIZIN="/root/aidag-lsc"
cd "$DIZIN" || exit 1

echo "================================================"
echo "   AIDAG-CHAIN SAGLIK RAPORU  ($(date '+%Y-%m-%d %H:%M'))"
echo "================================================"
echo ""

# --- 1. NODE DURUMU ---
echo "--- [1] NODE ---"
if systemctl is-active --quiet lsc-node.service; then
    VC=$(curl -s http://127.0.0.1:8645/status 2>/dev/null | grep -o '"vertex_count":[0-9]*' | cut -d: -f2)
    echo "  OK  Node aktif. Vertex sayisi: ${VC:-'?'}"
else
    echo "  HATA  Node CALISMIYOR!"
    echo "        DUZELTME: systemctl restart lsc-node.service && sleep 3 && systemctl is-active lsc-node.service"
    echo "        NEDEN bak: journalctl -u lsc-node.service -n 50 --no-pager"
fi
echo ""

# --- 2. TESTLER ---
echo "--- [2] TESTLER ---"
TEST_SONUC=$(cargo test --lib 2>&1 | grep "test result:" | head -1)
if echo "$TEST_SONUC" | grep -q "0 failed"; then
    echo "  OK  $TEST_SONUC"
else
    echo "  HATA  Testler kirik veya calismadi!"
    echo "        Sonuc: ${TEST_SONUC:-'test calismadi'}"
    echo "        DUZELTME: hangi test kirik gor -> cargo test --lib 2>&1 | grep -A3 FAILED"
    echo "        Son degisiklik bozduysa geri al -> git stash  (ya da git status ile bak)"
fi
echo ""

# --- 3. GUVENLIK (cargo audit) ---
echo "--- [3] GUVENLIK (bagimlilik aciklari) ---"
if command -v cargo-audit >/dev/null 2>&1; then
    AUDIT=$(cargo audit 2>&1 | grep -E "vulnerabilities found|warnings found" | head -2)
    if echo "$AUDIT" | grep -q "0 vulnerabilities"; then
        echo "  OK  Bilinen acik yok."
    else
        echo "  UYARI  $AUDIT"
        echo "        DETAY: cargo audit"
        echo "        DUZELTME (once yedek al):"
        echo "          cp Cargo.lock Cargo.lock.YEDEK-\$(date +%Y%m%d)"
        echo "          cargo update && cargo build --release && cargo test --lib"
        echo "        (guncelleme sonrasi test YESIL degilse yedekten don)"
    fi
else
    echo "  ATLANDI  cargo-audit kurulu degil."
    echo "        KUR: cargo install cargo-audit"
fi
echo ""

# --- 4. GIT (kaydedilmemis/push edilmemis) ---
echo "--- [4] GIT (yedek durumu) ---"
DEGISEN=$(git status --porcelain 2>/dev/null | wc -l)
if [ "$DEGISEN" -gt 0 ]; then
    echo "  UYARI  $DEGISEN dosyada kaydedilmemis degisiklik var."
    echo "        GOR: git status"
    echo "        KAYDET: git add -A && git commit -m 'aciklama' && git push origin main"
else
    echo "  OK  Calisma alani temiz (kaydedilmemis degisiklik yok)."
fi
PUSH_BEKLEYEN=$(git log origin/main..HEAD --oneline 2>/dev/null | wc -l)
if [ "$PUSH_BEKLEYEN" -gt 0 ]; then
    echo "  UYARI  $PUSH_BEKLEYEN commit GitHub'a push edilmemis (yedeksiz)."
    echo "        DUZELTME: git push origin main"
fi
echo ""

# --- 5. DISK ---
echo "--- [5] DISK ---"
DISK_KULLANIM=$(df / | tail -1 | awk '{print $5}' | tr -d '%')
if [ "$DISK_KULLANIM" -gt 85 ]; then
    echo "  UYARI  Disk %$DISK_KULLANIM dolu!"
    echo "        DUZELTME (build cache temizle): cargo clean"
    echo "        NE YER KAPLIYOR: du -sh target/ ; df -h /"
else
    echo "  OK  Disk %$DISK_KULLANIM dolu (yeterli yer var)."
fi
echo ""

# --- 6. OWNER ANAHTARI ---
echo "--- [6] OWNER ANAHTARI (kritik) ---"
if [ -f /root/faucet_anahtar.txt ]; then
    echo "  OK  Anahtar dosyasi mevcut (/root/faucet_anahtar.txt)."
    echo "        HATIRLATMA: Bu anahtarin OFFLINE/guvenli bir YEDEGI var mi? Yoksa AL."
else
    echo "  HATA  Owner anahtar dosyasi BULUNAMADI!"
    echo "        Yedegin varsa geri koy. Yoksa hazine kontrolu risktedir."
fi
echo ""

echo "================================================"
echo "  Rapor bitti. UYARI/HATA olanlarin altindaki"
echo "  DUZELTME komutlarini kopyalayip calistir."
echo "  (Bu script hicbir seyi kendi degistirmez.)"
echo "================================================"
