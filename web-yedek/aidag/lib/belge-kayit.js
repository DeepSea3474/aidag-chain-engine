/*
 * AIDAG-Chain belge KAYIT (kurum personeli tarafı) — tarayıcıda çalışır.
 *
 * KUBRA imzasız bir talep hazırlar (/kubra-brain/v1/belge/hazirla). Bu modül:
 *   - talebi DOĞRULAR (yalnız Record(belge_hash) imzalatabilir; başka işlem değil),
 *   - vertex'i talep alanlarından KENDİSİ kurar (sunucudaki imzalanacak_id'ye güvenmez),
 *   - İMZALAYICI ile imzalar. İmzalayıcı arayüzü: { tur, pubkey: Uint8Array(32), imzala(id32) -> Uint8Array(64) }
 *     Bugün: anahtar dosyası ([1][32 seed], tarayıcıda kalır). İleride: e-İmza köprüsü aynı arayüzle.
 * Gizli anahtar hiçbir yere gönderilmez; yalnız imzalı vertex /rpc/submit'e gider.
 *
 * Bağımlılıklar (sayfada zaten yüklü): blake3hash (lib/blake3.js), nacl (lib/nacl.min.js)
 * Tarayıcı: window.BelgeKayit · Node (testler): module.exports
 */
(function (kok, fabrika) {
  if (typeof module === "object" && module.exports) module.exports = fabrika();
  else kok.BelgeKayit = fabrika();
})(typeof self !== "undefined" ? self : this, function () {
  "use strict";

  var TALEP_TUR = "aidag-belge-kayit-talebi";
  var DOMAIN_TAG = new TextEncoder().encode("AIDAG-vertex-v1\u0000");
  var FORMAT_VERSION = 1, WIRE_VERSION = 1, TX_RECORD = 1, MAX_PARENTS = 8;

  function hex(b) { return Array.prototype.map.call(b, function (x) { return x.toString(16).padStart(2, "0"); }).join(""); }
  function unhex(s) {
    s = String(s).replace(/^0x/, "").trim();
    if (s.length % 2 || /[^0-9a-fA-F]/.test(s)) throw new Error("gecersiz hex");
    var a = new Uint8Array(s.length / 2);
    for (var i = 0; i < a.length; i++) a[i] = parseInt(s.substr(i * 2, 2), 16);
    return a;
  }
  function hex32(s, alan) {
    var b = unhex(s);
    if (b.length !== 32) throw new Error(alan + ": 32 bayt (64 hex) olmali");
    return b;
  }
  function leU32(n) { var b = new Uint8Array(4); new DataView(b.buffer).setUint32(0, n, true); return b; }
  function leU64(n) { var b = new Uint8Array(8); new DataView(b.buffer).setBigUint64(0, BigInt(n), true); return b; }
  function cat() {
    var n = 0, i, o, k = 0;
    for (i = 0; i < arguments.length; i++) n += arguments[i].length;
    o = new Uint8Array(n);
    for (i = 0; i < arguments.length; i++) { o.set(arguments[i], k); k += arguments[i].length; }
    return o;
  }
  function esit(a, b) { if (a.length !== b.length) return false; for (var i = 0; i < a.length; i++) if (a[i] !== b[i]) return false; return true; }

  // Talep kontrolleri: talep yalnız belge_hash'in Record kaydını imzalatabilir.
  function talepDogrula(talep, beklenenHash, netId) {
    if (!talep || talep.tur !== TALEP_TUR || talep.surum !== 1) throw new Error("gecersiz talep turu/surumu");
    if (talep.durum === "zaten-kayitli" || (talep.zincir && talep.zincir.kayitli)) throw new Error("belge zaten zincirde kayitli");
    if (netId !== undefined && talep.network_id !== netId) throw new Error("talep baska bir ag icin (network_id " + talep.network_id + ")");
    var h = hex32(talep.belge_hash, "belge_hash");
    if (beklenenHash !== undefined && hex(h) !== String(beklenenHash).toLowerCase()) throw new Error("talepteki hash sizin belgenizin hash'i degil");
    if (talep.payload_hex !== "01" + hex(h)) throw new Error("payload_hex belge_hash ile uyusmuyor (talep degistirilmis olabilir)");
    var parents = (talep.parents || []).map(function (p) { return hex32(p, "parent"); });
    parents.sort(function (a, b) { var x = hex(a), y = hex(b); return x < y ? -1 : x > y ? 1 : 0; });
    parents = parents.filter(function (p, i) { return i === 0 || hex(p) !== hex(parents[i - 1]); });
    if (parents.length > MAX_PARENTS) throw new Error("en fazla " + MAX_PARENTS + " parent");
    return { netId: talep.network_id, hash: h, parents: parents, payload: cat(new Uint8Array([TX_RECORD]), h) };
  }

  function vertexId(blake3hash, netId, pk, parents, ts, payload) {
    var parts = [DOMAIN_TAG, new Uint8Array([FORMAT_VERSION]), leU32(netId), pk, leU64(parents.length)];
    parents.forEach(function (p) { parts.push(p); });
    parts.push(leU64(ts), leU64(payload.length), payload);
    return blake3hash(cat.apply(null, parts));
  }

  // Talep + imzalayıcı -> imzalı vertex (wire baytları). ts: imza anı (unix sn).
  function imzaliVertex(talep, imzalayici, ts, deps, beklenenHash, netId) {
    var t = talepDogrula(talep, beklenenHash, netId);
    if (talep.imzalayan && talep.imzalayan.pubkey && talep.imzalayan.pubkey !== hex(imzalayici.pubkey)) {
      throw new Error("talep baska bir imzalayici icin hazirlanmis");
    }
    var id = vertexId(deps.blake3hash, t.netId, imzalayici.pubkey, t.parents, ts, t.payload);
    var sig = imzalayici.imzala(id);
    if (!sig || sig.length !== 64) throw new Error("imzalayici 64 baytlik imza dondurmedi");
    var parts = [new Uint8Array([WIRE_VERSION]), leU32(t.netId), leU64(t.parents.length)];
    t.parents.forEach(function (p) { parts.push(p); });
    parts.push(leU64(ts), leU64(t.payload.length), imzalayici.pubkey, sig, t.payload);
    return { wire: cat.apply(null, parts), id: hex(id) };
  }

  // Anahtar dosyası: [algo=1][32 seed] (33 bayt) — soulware/aidag araçlarıyla aynı biçim.
  function anahtarDosyasiImzalayici(bytes, nacl) {
    if (!(bytes instanceof Uint8Array) || bytes.length !== 33 || bytes[0] !== 1) throw new Error("anahtar dosyasi bicimi [1][32 seed] (33 bayt) olmali");
    var kp = nacl.sign.keyPair.fromSeed(bytes.slice(1));
    return {
      tur: "anahtar-dosyasi",
      pubkey: kp.publicKey,
      imzala: function (id) { return nacl.sign.detached(id, kp.secretKey); },
      sil: function () { kp.secretKey.fill(0); },
    };
  }

  // Dogrulamada denenecek ozetler, sirayla: once STANDART blake3; girdi 1 KB'tan buyukse
  // eski sayfanin standart disi ozeti (geriye uyum: eski sayfadan yapilmis kayitlar).
  function ozetAdaylari(bayt, blake3hash, blake3hashEski) {
    var s = hex(blake3hash(bayt)), out = [{ hash: s, yontem: "standart" }];
    if (bayt.length > 1024 && typeof blake3hashEski === "function") {
      var e = hex(blake3hashEski(bayt));
      if (e !== s) out.push({ hash: e, yontem: "eski" });
    }
    return out;
  }

  function adres(pk, blake3hash) { return "0x" + hex(blake3hash(pk).slice(0, 20)); }

  return { TALEP_TUR: TALEP_TUR, hex: hex, unhex: unhex, talepDogrula: talepDogrula, vertexId: vertexId,
    imzaliVertex: imzaliVertex, anahtarDosyasiImzalayici: anahtarDosyasiImzalayici, adres: adres, ozetAdaylari: ozetAdaylari };
});
