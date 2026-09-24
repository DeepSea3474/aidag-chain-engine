// Bugunun kayit dosyasini AIDAG zincirine yazar (belge dogrulama mantigi, terminalden)
const fs = require('fs');
const nacl = require('/var/www/aidag/lib/nacl.min.js'); // module.exports -> dogrudan nacl
require('/var/www/aidag/lib/blake3.js'); // global.blake3hash yukler

// blake3hash fonksiyonu (sayfadaki gibi) — lib'in export sekline gore
const blake3hash = global.blake3hash;

const RPC = "http://127.0.0.1:8645";
const NETWORK_ID = 3474, FORMAT_VERSION = 1, WIRE_VERSION = 1, TX_RECORD = 1;
const DOMAIN_TAG = new TextEncoder().encode("AIDAG-vertex-v1\u0000");

const hex = b => [...b].map(x=>x.toString(16).padStart(2,"0")).join("");
const unhex = s => { s=s.replace(/^0x/,"").trim(); const a=new Uint8Array(s.length/2); for(let i=0;i<a.length;i++)a[i]=parseInt(s.substr(i*2,2),16); return a; };
function leU32(n){const b=new Uint8Array(4);new DataView(b.buffer).setUint32(0,n,true);return b;}
function leU64(n){const b=new Uint8Array(8);new DataView(b.buffer).setBigUint64(0,BigInt(n),true);return b;}
function cat(...a){let n=0;for(const x of a)n+=x.length;const o=new Uint8Array(n);let i=0;for(const x of a){o.set(x,i);i+=x.length;}return o;}

// DENETIM K-08: imza anahtari DOSYADAN okunur. Eskiden sabit tohum [7;32] kullaniliyordu;
// bu tohum depoda ve testlerde acik oldugu icin "resmi kaydeden" (0871f3aa...) adresi adina
// herkes kayit imzalayabiliyordu. Dosya: 32 bayt ham tohum ya da 64 hex, izinler 0600.
const ANAHTAR_YOLU = process.env.AIDAG_KAYIT_ANAHTARI;
if (!ANAHTAR_YOLU) {
  console.error("HATA: AIDAG_KAYIT_ANAHTARI tanimli degil (32 baytlik ed25519 tohum dosyasinin yolu).");
  process.exit(1);
}
if ((fs.statSync(ANAHTAR_YOLU).mode & 0o077) !== 0) {
  console.error("HATA: anahtar dosyasi grup/digerleri tarafindan erisilebilir; izinleri 0600 yapin.");
  process.exit(1);
}
let ham = fs.readFileSync(ANAHTAR_YOLU);
if (ham.length !== 32) {
  const metin = ham.toString("utf8").trim().replace(/^0x/, "");
  if (/^[0-9a-fA-F]{64}$/.test(metin)) ham = Buffer.from(metin, "hex");
}
if (ham.length !== 32) {
  console.error("HATA: anahtar dosyasi 32 bayt ham tohum ya da 64 hex karakter olmali.");
  process.exit(1);
}
const SEED = new Uint8Array(ham);
if (SEED.every(b => b === SEED[0])) {
  console.error("HATA: tek bayttan olusan tohum (orn. [7;32]) kamuya acik demo anahtaridir; reddedildi.");
  process.exit(1);
}
const kp = nacl.sign.keyPair.fromSeed(SEED);
const PK = kp.publicKey, SK64 = kp.secretKey;

function hashId(parents,ts,payload){
  const parts=[DOMAIN_TAG,new Uint8Array([FORMAT_VERSION]),leU32(NETWORK_ID),PK,leU64(parents.length)];
  for(const p of parents)parts.push(p);
  parts.push(leU64(ts),leU64(payload.length),payload);
  return blake3hash(cat(...parts));
}
function vertexOlustur(parents,payload,ts){
  parents=parents.slice().sort((a,b)=>hex(a)<hex(b)?-1:1);
  const vid=hashId(parents,ts,payload);
  const sig=nacl.sign.detached(vid,SK64);
  const parts=[new Uint8Array([WIRE_VERSION]),leU32(NETWORK_ID),leU64(parents.length)];
  for(const p of parents)parts.push(p);
  parts.push(leU64(ts),leU64(payload.length),PK,sig,payload);
  return cat(...parts);
}
function recordPayload(hash32){return cat(new Uint8Array([TX_RECORD]),hash32);}

(async()=>{
  // 1) dosyanin hash'i
  const data = fs.readFileSync("/root/aidag-lsc/zincir-kayitlari/2026-09-21-gelistirme.txt");
  const dosyaHash = blake3hash(new Uint8Array(data));
  console.log("dosya hash:", hex(dosyaHash));

  // 2) tips al
  const tipsR = await (await fetch(RPC+"/tips")).json();
  const tips = (tipsR.tips||[]).map(unhex);
  console.log("tips:", tips.length);

  // 3) vertex kur + gonder
  const ts = Math.floor(Date.now()/1000);
  const wire = vertexOlustur(tips, recordPayload(dosyaHash), ts);
  const r = await fetch(RPC+"/submit",{method:"POST",body:hex(wire)});
  const cevap = await r.json();
  console.log("SONUC:", JSON.stringify(cevap));
  // /submit reddi de "ok":true ile doner; sonucu ayrica kontrol et.
  if (!cevap.ok || String(cevap.sonuc || "").includes("Rejected")) {
    console.error("HATA: kayit zincire yazilamadi.");
    process.exit(1);
  }
})();
