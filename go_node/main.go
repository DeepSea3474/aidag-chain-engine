package main

import (
	"bytes"
	"encoding/json"
	"fmt"
	"net"
	"net/http"
	"os"
	"os/signal"
	"syscall"
)

type ProofRequest struct {
	Proof  string `json:"proof"`
	Status string `json:"status"`
}

func main() {
	fmt.Println("🚀 [SOULWARE-WIND] Go Micro-Cell Starting...")

	// HTTP Proof Server (Port 9002)
	http.HandleFunc("/proof", func(w http.ResponseWriter, r *http.Request) {
		var p ProofRequest
		if err := json.NewDecoder(r.Body).Decode(&p); err == nil {
			fmt.Printf("🔒 [AIDAG-MINT] Proof Stamped into Block: %s | Status: %s\n", p.Proof, p.Status)
			w.WriteHeader(http.StatusOK)
		}
	})
	go http.ListenAndServe("0.0.0.0:9002", nil)

	// UDP Wind Listener (Port 9001)
	addr, _ := net.ResolveUDPAddr("udp", "0.0.0.0:9001")
	conn, _ := net.ListenUDP("udp", addr)
	defer conn.Close()

	fmt.Println("💤 [SOULWARE-WIND] Cell Idle. Listening UDP (9001) & HTTP Proofs (9002)...")

	sigChan := make(chan os.Signal, 1)
	signal.Notify(sigChan, syscall.SIGINT, syscall.SIGTERM)

	go func() {
		buf := make([]byte, 1024)
		for {
			n, _, err := conn.ReadFromUDP(buf)
			if err != nil {
				continue
			}
			message := string(buf[:n])
			fmt.Printf("⚡ [SIGNAL] Wind Vector Caught: %s\n", message)

			jsonPayload := fmt.Sprintf("{\"vector\": \"%s\"}", message)
			http.Post("http://soulware_python_agent:5000/process", "application/json", bytes.NewBuffer([]byte(jsonPayload)))
		}
	}()

	<-sigChan
}
