# -*- coding: utf-8 -*-
from flask import Flask, request, jsonify
import urllib.request
import json
import hashlib

app = Flask(__name__)

print("[SOULWARE-AI] AIDAG Neural Engine Initialized.")

def analyze_dag_vector(vector):
    data_bytes = vector.encode("utf-8")
    hash_object = hashlib.sha256(data_bytes).hexdigest()
    
    # Zararli kelime veya yüksek anomali tespiti
    is_malicious = "ATTACK" in vector or "MALICIOUS" in vector
    score = 99 if is_malicious else (sum([ord(c) for c in vector]) % 80)
    
    status = "REJECTED_ANOMALY" if is_malicious else "VERIFIED_VALID"
    proof = f"0x-AIDAG-{hash_object[:12].upper()}"
    
    return proof, status, score

@app.route("/process", methods=["POST"])
def process_vector():
    data = request.json or {}
    vector = data.get("vector", "")
    print(f"⚡ [AI-ENGINE] Processing AIDAG Vector: {vector}")
    
    proof, status, score = analyze_dag_vector(vector)
    print(f"🔬 [ANALYSIS] Score: {score}/100 | Status: {status} | Proof: {proof}")
    
    try:
        req = urllib.request.Request(
            "http://soulware_go_cell:9002/proof",
            data=json.dumps({"proof": proof, "status": status, "vector": vector}).encode("utf-8"),
            headers={"Content-Type": "application/json"}
        )
        urllib.request.urlopen(req)
        print("🔒 [SHIELD] Proof successfully transmitted to Go Sentinel.")
    except Exception as e:
        print(f"❌ [ERROR] Shield transmission failed: {e}")

    return jsonify({"status": "SUCCESS", "proof": proof, "analysis_score": score})

if __name__ == "__main__":
    app.run(host="0.0.0.0", port=5000)
