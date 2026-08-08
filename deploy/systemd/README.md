# SoulwareAI — Kalıcı Servisler (systemd)

KUBRA ve altyapısı 7/24 çalışır: oturum kopsa da (SSH/telefon), sunucu yeniden başlasa
da (boot), servis çökse de (Restart=always) otomatik geri gelir.

## Servisler
- `aidag-node`         — AIDAG-Chain mainnet node (RPC :8645, P2P :40001)
- `soulware-kubra`     — KUBRA çekirdeği (:8646, grounding açık)
- `soulware-coordinator` — hesaplama+ödül koordinatörü (:8647, mainnet-güvenli kuyruk)
- `soulware-learn`     — sürekli arka-plan öğrenme (Wikipedia -> korpus, offset kalıcı)

## Kurulum
    sudo cp deploy/systemd/*.service /etc/systemd/system/
    sudo systemctl daemon-reload
    sudo systemctl enable --now aidag-node soulware-kubra soulware-coordinator soulware-learn

## İzleme
    systemctl status soulware-kubra
    journalctl -u soulware-kubra -f
    tail -f /root/aidag-lsc/.data/soulware-learn.log

## Not
- Koordinatörde settle key/auto YOK → mainnet-güvenli (owner offline `soulware-settle` ile basar).
- Öğrenme yumuşak tavanı: SOULWARE_LEARN_MAX (varsayılan 5000 belge; bellek-içi depo sınırı, indeks sonra).
