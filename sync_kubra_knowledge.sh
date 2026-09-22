#!/bin/bash
SRC_DIR="/root/aidag-lsc"
KNOWLEDGE_BASE="/root/kubra-inbound/codebase_knowledge.txt"
mkdir -p /root/kubra-inbound
echo "============================================================" > "$KNOWLEDGE_BASE"
echo "   AIDAG CHAIN & SOULWARE-CORE MİMARİ BİLGİ TABANI" >> "$KNOWLEDGE_BASE"
echo "   Güncelleme: $(date)" >> "$KNOWLEDGE_BASE"
echo "============================================================" >> "$KNOWLEDGE_BASE"
echo "" >> "$KNOWLEDGE_BASE"
find "$SRC_DIR" -name "*.rs" | while read -r file; do
    echo "------------------------------------------------------------" >> "$KNOWLEDGE_BASE"
    echo "DOSYA: $file" >> "$KNOWLEDGE_BASE"
    echo "------------------------------------------------------------" >> "$KNOWLEDGE_BASE"
    cat "$file" >> "$KNOWLEDGE_BASE"
    echo -e "\n\n" >> "$KNOWLEDGE_BASE"
done
echo "[✔] Tamamlandı: $KNOWLEDGE_BASE"
