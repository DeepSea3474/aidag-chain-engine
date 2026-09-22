set -euo pipefail

KNOWLEDGE_FILE="/root/kubra-inbound/codebase_knowledge.txt"
TARGET_DIR="/root/aidag-lsc/data"

if [ ! -f "$KNOWLEDGE_FILE" ]; then
    echo "[HATA] Kaynak dosya bulunamadı: $KNOWLEDGE_FILE" >&2
    exit 1
fi

mkdir -p "$TARGET_DIR"
cp "$KNOWLEDGE_FILE" "$TARGET_DIR/active_grounding.txt"

echo "[✔] Doğrulandı ve taşındı: $TARGET_DIR/active_grounding.txt"

systemctl restart soulware-kubra.service
systemctl status soulware-kubra.service --no-pager --lines=5
