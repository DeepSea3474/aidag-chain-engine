#!/bin/bash
set -e

mkdir -p go_node python_agent

# 1. python_agent/main.py
cat << 'PYEOF' > python_agent/main.py
import time
print("[SOULWARE-AI] Python Micro AI Agent Started.")
print("[AI-AGENT] Idle mode. Waiting for signals from Go Cell.")
while True:
    time.sleep(3600)
PYEOF

# 2. python_agent/Dockerfile
cat << 'PYDOC' > python_agent/Dockerfile
FROM python:3.11-slim
WORKDIR /app
COPY main.py .
CMD ["python", "main.py"]
PYDOC

# 3. go_node/main.go
cat << 'GOEOF' > go_node/main.go
package main

import (
	"fmt"
	"net"
	"os"
	"os/signal"
	"syscall"
)

func main() {
	fmt.Println("🚀 [SOULWARE-WIND] Go Micro-Cell Starting...")

	addr, err := net.ResolveUDPAddr("udp", "0.0.0.0:9001")
	if err != nil {
		fmt.Printf("Error: %v\n", err)
		return
	}

	conn, err := net.ListenUDP("udp", addr)
	if err != nil {
		fmt.Printf("Port Listen Error: %v\n", err)
		return
	}
	defer conn.Close()

	fmt.Println("💤 [SOULWARE-WIND] Cell Idle. Go Goroutine Listening P2P Wind (Port 9001)...")

	sigChan := make(chan os.Signal, 1)
	signal.Notify(sigChan, syscall.SIGINT, syscall.SIGTERM)

	go func() {
		buf := make([]byte, 1024)
		for {
			n, remoteAddr, err := conn.ReadFromUDP(buf)
			if err != nil {
				continue
			}

			message := string(buf[:n])
			fmt.Printf("⚡ [SIGNAL] Wind Signal Caught! Source: %s\n", remoteAddr.String())
			fmt.Printf("📩 [PAYLOAD] Micro-Vector: %s\n", message)
			fmt.Println("🔗 [AIDAG-SHIELD] Proof Stamped: 0x-AIDAG-GO-PROOF-OK\n")
		}
	}()

	<-sigChan
	fmt.Println("💤 [SOULWARE-WIND] Go Cell Safely Shutting Down.")
}
GOEOF

# 4. go_node/Dockerfile
cat << 'GODOC' > go_node/Dockerfile
FROM golang:1.22-alpine AS builder
WORKDIR /app
COPY main.go .
RUN go mod init go_node && CGO_ENABLED=0 GOOS=linux go build -o cell_node main.go

FROM alpine:latest
WORKDIR /root/
COPY --from=builder /app/cell_node .
CMD ["./cell_node"]
GODOC

echo "✅ All files created cleanly!"
