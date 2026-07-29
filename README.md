# ⚡ AIDAG-WIND (Layer-1 Verifiable AI Mesh Engine)

<p align="center">
  <a href="https://asciinema.org/a/kAcnhGrzvH01sdIg">
    <img src="https://asciinema.org/a/kAcnhGrzvH01sdIg.svg" alt="asciinema recording" width="750"/>
  </a>
</p>

---

## 📌 Overview
**AIDAG-WIND** is an event-driven, high-throughput decentralized AI mesh engine built on a **Directed Acyclic Graph (DAG)** architecture. It combines a high-speed **Go Sentinel cell** with an isolated **Python AI inference agent** to provide real-time anomaly detection and cryptographically verified state transitions.

---

## 🏗️ Architecture Components
- **Go Micro-Cell (`soulware_go_cell`)**: High-performance network listener (UDP/9001) handling fast proof generation, minting state blocks, and instant anomaly rejection.
- **Python Inference Agent (`soulware_python_agent`)**: Real-time agent evaluating incoming vectors, performing model inference, and outputting state validity signals.
- **Nginx Gateway (`aidag_nginx`)**: Edge proxy routing internal communications and RPC proof endpoints.

---

## ⚡ Live Terminal Verification & Status
The system actively catches incoming signals, validates transaction payloads, and mints verified states directly to the block topology:
- **`Status: VERIFIED_VALID`**: Normal state transition and valid vector proof.
- **`Status: REJECTED_ANOMALY`**: Malicious vector or attack pattern automatically intercepted and dropped.

---

## 🚀 Quick Start
```bash
# Clone the repository
git clone https://github.com/DeepSea3474/aidag-chain-engine.git
cd aidag-chain-engine

# Start services via Docker Compose
docker-compose up -d

# Send a test signal
echo "AIDAG_TX_VALID_7700" | nc -u -w1 127.0.0.1 9001
```
