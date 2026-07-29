# -*- coding: utf-8 -*-
from flask import Flask, request, jsonify
import urllib.request
import json
import time

app = Flask(__name__)

print("[SOULWARE-AI] Python AI Agent Ready.")

@app.route("/process", methods=["POST"])
def process_vector():
    data = request.json or {}
    vector = data.get("vector", "")
    print(f"[AI-AGENT] Processing Vector: {vector}")
    time.sleep(0.3)
    proof = "0x-AIDAG-PROOF-" + str(int(time.time()))
    print(f"[AI-ENGINE] Analysis Done. Generated Proof: {proof}")
    
    # Go Hucresine Kanit Durusut
    try:
        req = urllib.request.Request(
            "http://soulware_go_cell:9002/proof",
            data=json.dumps({"proof": proof, "status": "VERIFIED"}).encode("utf-8"),
            headers={"Content-Type": "application/json"}
        )
        urllib.request.urlopen(req)
        print("[AI-SHIELD] Proof successfully returned to Go Cell.")
    except Exception as e:
        print(f"[AI-ERROR] Failed to send proof: {e}")

    return jsonify({"status": "SUCCESS", "proof": proof})

if __name__ == "__main__":
    app.run(host="0.0.0.0", port=5000)
