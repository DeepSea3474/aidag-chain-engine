#!/bin/bash
# ===== GUVENLI MOD — Node Kurtarma Botu =====
# Sadece node toparlar. Token/para/owner islemi YOK. Yetkisi: systemctl restart.
RPC="http://localhost:8645/status"
SERVIS="lsc-node.service"
LOG="/root/aidag-lsc/guvenli-mod.log"
BEKLE_SN=5        # cevap yoksa, gercek cokme mi diye bu kadar bekle
KONTROL_SN=15     # her bu kadar saniyede bir kontrol
ZAMAN() { date '+%Y-%m-%d %H:%M:%S'; }

# Node saglikli mi? RPC cevap veriyorsa 0 (saglikli), yoksa 1 (olu).
saglikli_mi() {
    curl -s -m 3 "$RPC" > /dev/null 2>&1
}

# Loga yaz + ekrana bas.
kayit() {
    echo "[$(ZAMAN)] $1" | tee -a "$LOG"
}

kayit "GUVENLI MOD basladi — node izleniyor (sadece kurtarma, para islemi YOK)."

while true; do
    if ! saglikli_mi; then
        # Ilk cevapsizlik: gecici takilma olabilir, BEKLE_SN kadar bekle
        kayit "UYARI: node cevap vermiyor. ${BEKLE_SN}sn bekleniyor (gecici mi gercek mi)..."
        sleep "$BEKLE_SN"
        if ! saglikli_mi; then
            # Hala olu: GERCEK cokme. Owner onayi BEKLEMEDEN otomatik kurtar.
            kayit "COKME ONAYLANDI: node hala olu. Otomatik kurtarma baslatiliyor (systemctl restart)..."
            systemctl restart "$SERVIS"
            sleep 8
            if saglikli_mi; then
                kayit "KURTARILDI: node tekrar saglikli."
            else
                kayit "KRITIK: restart sonrasi hala cevap yok! Elle mudahale gerekebilir."
            fi
        else
            kayit "GECICI: node kendine geldi, mudahale gerekmedi."
        fi
    fi
    sleep "$KONTROL_SN"
done
