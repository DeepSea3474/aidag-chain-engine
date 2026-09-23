// e2e yardimcisi: sayfanin GERCEK tarayici modullerini node'da calistirir.
// Kullanim: node js_yardimci.js <islem> <json-arg>   (cikti: JSON)
"use strict";
const fs = require("fs"), path = require("path");
const WEB = process.env.WEB_KOK; // lib/blake3.js, lib/nacl.min.js, lib/belge-kayit.js burada
global.window = global; global.self = global;
eval(fs.readFileSync(path.join(WEB, "lib/blake3.js"), "utf8"));
global.nacl = require(path.join(WEB, "lib/nacl.min.js"));
const BK = require(path.join(WEB, "lib/belge-kayit.js"));
const [islem, argJson] = process.argv.slice(2);
const a = JSON.parse(argJson || "{}");
const cikti = x => process.stdout.write(JSON.stringify(x));

if (islem === "blake3") {
  // a.dosya: diskteki dosyanin ozeti (tarayicidaki blake3hash ile)
  cikti({ hash: BK.hex(blake3hash(new Uint8Array(fs.readFileSync(a.dosya)))) });
} else if (islem === "adaylar") {
  cikti(BK.ozetAdaylari(new Uint8Array(fs.readFileSync(a.dosya)), blake3hash, blake3hashEski));
} else if (islem === "eski_canli") {
  // Canli/main'deki ESKI blake3.js dosyasinin ciktisi (karsilastirma icin)
  const g = {}; new Function("globalThis", "window", fs.readFileSync(a.eski_js, "utf8"))(g, g);
  cikti({ hash: BK.hex(g.blake3hash(new Uint8Array(fs.readFileSync(a.dosya)))) });
} else if (islem === "anahtar") {
  const imz = BK.anahtarDosyasiImzalayici(new Uint8Array(fs.readFileSync(a.anahtar)), nacl);
  cikti({ pubkey: BK.hex(imz.pubkey), adres: BK.adres(imz.pubkey, blake3hash) });
} else if (islem === "imzala") {
  // Sayfadaki "Imzala ve Zincire Gonder" ile ayni yol: BK.imzaliVertex
  const imz = BK.anahtarDosyasiImzalayici(new Uint8Array(fs.readFileSync(a.anahtar)), nacl);
  try {
    const r = BK.imzaliVertex(a.talep, imz, a.ts, { blake3hash }, a.beklenen, a.net);
    cikti({ ok: true, hex: BK.hex(r.wire), id: r.id });
  } catch (e) { cikti({ ok: false, hata: String(e.message || e) }); }
} else if (islem === "id") {
  // Talepteki alanlardan JS ile vertex id (Rust'in imzalanacak_id'siyle karsilastirma)
  const t = BK.talepDogrula(a.talep);
  cikti({ id: BK.hex(BK.vertexId(blake3hash, t.netId, BK.unhex(a.pubkey), t.parents, a.talep.ts, t.payload)) });
} else if (islem === "kurum") {
  // TEST: tip=5 kurum kaydi vertex'i ([5][kategori][ad]) — yalniz izole devnet'te kurum hazirlamak icin
  const seed = new Uint8Array(fs.readFileSync(a.anahtar)).slice(1);
  const kp = nacl.sign.keyPair.fromSeed(seed);
  const payload = new Uint8Array([5, a.kategori, ...new TextEncoder().encode(a.ad)]);
  const parents = a.parents.map(BK.unhex).sort((x, y) => BK.hex(x) < BK.hex(y) ? -1 : 1);
  const id = BK.vertexId(blake3hash, a.net, kp.publicKey, parents, a.ts, payload);
  const sig = nacl.sign.detached(id, kp.secretKey);
  const le = (n, k) => { const b = new Uint8Array(k); const d = new DataView(b.buffer); k === 4 ? d.setUint32(0, n, true) : d.setBigUint64(0, BigInt(n), true); return b; };
  const parca = [new Uint8Array([1]), le(a.net, 4), le(parents.length, 8), ...parents, le(a.ts, 8), le(payload.length, 8), kp.publicKey, sig, payload];
  cikti({ hex: parca.map(BK.hex).join("") });
} else {
  console.error("bilinmeyen islem"); process.exit(2);
}
