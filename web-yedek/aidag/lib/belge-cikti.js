/*
 * AIDAG-Chain belge çıktısı: QR'lı belge kapağı (A4) ve parça etiketi (70x40 mm).
 *
 * Her şey TARAYICIDA üretilir; sunucu yazıcıya erişmez, dosya almaz.
 *   - "Yazdır"   : svg() çıktısı + tarayıcının kendi yazdırma penceresi
 *   - "PDF indir": pdf() çıktısı (jsPDF, vektör)
 * İki yol da AYNI şemadan (sema()) çizer: ekran/baskı ile PDF birebir aynıdır.
 *
 * Barkodlar (bwip-js, MIT):
 *   - QR ve DataMatrix: https://aidag-chain.com/belge/<hash> (tam doğrulama adresi)
 *   - Code128 (yalnız etiket): kısa referans = hash'in ilk 16 hanesi (büyük harf).
 *     Kısa referans EŞLEŞTİRME içindir (eski okuyucular, depo/ERP); kriptografik
 *     doğrulama QR/DataMatrix'teki tam hash ile yapılır.
 *
 * Tarayıcı: window.BelgeCikti · Node (testler): module.exports
 */
(function (kok, fabrika) {
  if (typeof module === "object" && module.exports) module.exports = fabrika();
  else kok.BelgeCikti = fabrika();
})(typeof self !== "undefined" ? self : this, function () {
  "use strict";

  var SITE = "https://aidag-chain.com";
  var KURUM_UYARI = "Kurum kaydı zincirde beyana dayanır; kurum kimliği bağımsız olarak doğrulanmamıştır.";

  function hashDogrula(hash) {
    if (typeof hash !== "string" || !/^[0-9a-f]{64}$/.test(hash)) throw new Error("hash 64 haneli küçük harf hex olmalı");
    return hash;
  }
  function dogrulamaUrl(hash) { return SITE + "/belge/" + hashDogrula(hash); }
  function kisaRef(hash) { return hashDogrula(hash).slice(0, 16).toUpperCase(); }
  function kisaRefGoster(hash) { return kisaRef(hash).match(/.{4}/g).join("-"); }
  function utc(sn) {
    var d = new Date(Number(sn) * 1000);
    function p(n) { return String(n).padStart(2, "0"); }
    return d.getUTCFullYear() + "-" + p(d.getUTCMonth() + 1) + "-" + p(d.getUTCDate()) + " " +
      p(d.getUTCHours()) + ":" + p(d.getUTCMinutes()) + ":" + p(d.getUTCSeconds()) + " UTC";
  }

  // ── Barkod → dikdörtgenler (mm). 2D: satır satır bitişik koyu modüller birleşir. ──
  var BINDIRME = 0.01; // bitişik dikdörtgenler arasında görüntüleyici çizgisi olmasın
  function matris(bwip, bcid, metin, x, y, boyut, sessiz) {
    var sec = { bcid: bcid, text: metin };
    if (bcid === "qrcode") sec.eclevel = "M";
    var r = bwip.raw(sec)[0];
    var n = Math.max(r.pixx, r.pixy), m = boyut / (n + 2 * sessiz), out = [];
    for (var sy = 0; sy < r.pixy; sy++) {
      var sx = 0;
      while (sx < r.pixx) {
        if (r.pixs[sy * r.pixx + sx]) {
          var bas = sx;
          while (sx < r.pixx && r.pixs[sy * r.pixx + sx]) sx++;
          out.push({ t: "rect", x: x + (sessiz + bas) * m, y: y + (sessiz + sy) * m, w: (sx - bas) * m + BINDIRME, h: m + BINDIRME });
        } else sx++;
      }
    }
    return { ogeler: out, modul: m, boyut: [r.pixx, r.pixy] };
  }
  function code128(bwip, metin, x, y, genislik, yukseklik) {
    var r = bwip.raw({ bcid: "code128", text: metin })[0];
    var toplam = 0, i;
    for (i = 0; i < r.sbs.length; i++) toplam += r.sbs[i];
    var sessiz = 10, m = Math.min(0.33, genislik / (toplam + 2 * sessiz));
    var bas = x + (genislik - toplam * m) / 2, cx = bas, out = [];
    for (i = 0; i < r.sbs.length; i++) {
      var w = r.sbs[i] * m;
      if (i % 2 === 0) out.push({ t: "rect", x: cx, y: y, w: w, h: yukseklik }); // çift indeks = çubuk
      cx += w;
    }
    return { ogeler: out, modul: m, moduller: toplam };
  }
  function yazi(x, y, pt, metin, o) {
    o = o || {};
    return { t: "text", x: x, y: y, pt: pt, yazi: String(metin), kalin: !!o.kalin, hiza: o.hiza || "left" };
  }

  // ── Şemalar ──
  // veri: { hash, zincir:{kayitli, kaydeden, zaman}, kurum:{kayitli, ad, kategori, dogrulanmis}|null, olusturma }
  function sema(tur, veri, bwip) {
    var h = hashDogrula(veri.hash);
    if (!veri.zincir || veri.zincir.kayitli !== true) throw new Error("belge zincirde kayıtlı değil: çıktı üretilmez");
    if (tur === "kapak") return kapak(veri, h, bwip);
    if (tur === "etiket") return etiket(veri, h, bwip);
    throw new Error("bilinmeyen şablon: " + tur);
  }

  // Kurum adı/kategorisi zincirde BEYANDIR (saldırgan yazabilir). Gösterimden önce:
  //  - Unicode yön (bidi) ve görünmez/kontrol karakterleri silinir (U+202A-202E, U+2066-2069, U+200E/F,
  //    U+061C, C0/C1, sıfır genişlikli...) -> "beyan" etiketinin yönü/yeri değiştirilemez;
  //  - çift tırnak benzerleri tek tırnağa indirilir -> ad, bizim koyduğumuz “…” tırnağını kapatamaz;
  //  - uzunluk sınırlanır. Ad her zaman “tırnak içinde”, durum etiketi AYRI satırda yazılır:
  //    adın içinde "doğrulanmış kurum" geçse bile bizim etiketimiz ayrı ve görünür kalır.
  var KONTROL_RE = /[\u0000-\u001f\u007f-\u009f\u00ad\u061c\u115f\u1160\u180e\u200b-\u200f\u2028-\u202e\u2060-\u206f\u3164\ufeff\ufff9-\ufffb]/g;
  var TIRNAK_RE = /["\u201c\u201d\u201e\u201f\u00ab\u00bb\u2033\u301d\u301e\u301f\uff02]/g;
  var KURUM_AD_MAX = 80, KATEGORI_MAX = 16, SATIR_MAX = 42; // SATIR_MAX: en geniş harfle (W) bile A4 çerçevesine sığar
  function kurumMetni(s, max) {
    var t = String(s == null ? "" : s);
    if (t.normalize) t = t.normalize("NFC");
    t = t.replace(KONTROL_RE, " ").replace(TIRNAK_RE, "'").replace(/\s+/g, " ").trim();
    var h = Array.from(t);
    if (h.length > max) t = h.slice(0, max - 1).join("") + "…";
    return t;
  }
  function kurumAdi(ad) { return "\u201c" + (kurumMetni(ad, KURUM_AD_MAX) || "?") + "\u201d"; }
  function kurumDurumu(k) { return (k && k.dogrulanmis === true) ? "doğrulanmış kurum" : "beyan (doğrulanmamış)"; }
  // Uzun adı kelime sınırından SATIR_MAX'lık satırlara böler (tek kelime uzunsa keser).
  function sar(metin, max) {
    var out = [], satir = [];
    metin.split(" ").forEach(function (kelime) {
      var h = Array.from(kelime);
      while (h.length > max) { if (satir.length) { out.push(satir.join(" ")); satir = []; } out.push(h.slice(0, max).join("")); h = h.slice(max); }
      kelime = h.join("");
      if (!kelime) return;
      if (Array.from(satir.concat(kelime).join(" ")).length > max && satir.length) { out.push(satir.join(" ")); satir = []; }
      satir.push(kelime);
    });
    if (satir.length) out.push(satir.join(" "));
    return out;
  }
  function kurumSatirlari(k) {
    if (!k || !k.kayitli) return ["Kaydeden adres zincirde kurum olarak kayıtlı değil."];
    var kat = kurumMetni(k.kategori, KATEGORI_MAX);
    return sar(kurumAdi(k.ad), SATIR_MAX).concat(["Durum: " + kurumDurumu(k) + (kat ? " · kategori \u201c" + kat + "\u201d" : "")]);
  }

  function kapak(v, h, bwip) {
    var W = 210, H = 297, o = [];
    o.push({ t: "cerceve", x: 12, y: 12, w: W - 24, h: H - 24, kalinlik: 0.6 });
    o.push({ t: "cerceve", x: 15, y: 15, w: W - 30, h: H - 30, kalinlik: 0.2 });
    o.push(yazi(W / 2, 34, 22, "AIDAG-Chain", { kalin: true, hiza: "center" }));
    o.push(yazi(W / 2, 44, 13, "Belge Zincir Kaydı Kapağı", { hiza: "center" }));
    o.push({ t: "rect", x: 30, y: 50, w: W - 60, h: 0.3 });
    var y = 62, sol = 30;
    function alan(etiket, deger, kalin) {
      o.push(yazi(sol, y, 9, etiket, { kalin: true })); y += 5.5;
      [].concat(deger).forEach(function (d) { o.push(yazi(sol, y, 10.5, d, { kalin: !!kalin })); y += 5.2; });
      y += 3.5;
    }
    alan("Belge özeti (BLAKE3, 32 bayt)", [h.slice(0, 32), h.slice(32)], true);
    alan("Zincire kayıt zamanı", utc(v.zincir.zaman));
    alan("Kaydeden adres", v.zincir.kaydeden || "?");
    alan("Kurum", kurumSatirlari(v.kurum));
    alan("Ağ", "AIDAG-Chain Mainnet (Chain ID 3474)");
    alan("Kısa referans", kisaRefGoster(h));
    var qBoyut = 62, qx = (W - qBoyut) / 2, qy = y + 2;
    var q = matris(bwip, "qrcode", dogrulamaUrl(h), qx, qy, qBoyut, 4);
    o = o.concat(q.ogeler);
    o.push(yazi(W / 2, qy + qBoyut + 6, 8, "Doğrulama adresi (QR ile aynı):", { kalin: true, hiza: "center" }));
    o.push(yazi(W / 2, qy + qBoyut + 10.5, 8.5, "aidag-chain.com/belge/" + h.slice(0, 32), { hiza: "center" }));
    o.push(yazi(W / 2, qy + qBoyut + 14.5, 8.5, h.slice(32), { hiza: "center" }));
    var ay = 247;
    [
      "Bu kapak belgenin kendisi değildir; belge özetinin AIDAG-Chain'deki kaydını gösterir.",
      "QR kodu kaydın zincirde olduğunu gösterir. Elinizdeki belgenin bu kayıtla aynı olduğunu",
      "doğrulamak için belgeyi aidag-chain.com/belge sayfasına yükleyin: aynı özet çıkmalıdır.",
      (v.kurum && v.kurum.kayitli && v.kurum.dogrulanmis !== true) ? KURUM_UYARI : ""
    ].forEach(function (s) { if (s) { o.push(yazi(W / 2, ay, 8, s, { hiza: "center" })); ay += 4.4; } });
    o.push(yazi(W / 2, H - 20, 7, "Çıktı oluşturma: " + utc(v.olusturma) + " · tarayıcıda üretildi", { hiza: "center" }));
    return { tur: "kapak", w: W, h: H, icAlan: [15, 15, W - 15, H - 15], ogeler: o, barkodlar: { qrcode: { icerik: dogrulamaUrl(h), modul: q.modul, boyut: q.boyut } } };
  }

  function etiket(v, h, bwip) {
    var W = 70, H = 40, o = [];
    o.push({ t: "cerceve", x: 0.5, y: 0.5, w: W - 1, h: H - 1, kalinlik: 0.15 });
    var q = matris(bwip, "qrcode", dogrulamaUrl(h), 2, 2, 21, 2);
    var d = matris(bwip, "datamatrix", dogrulamaUrl(h), 24, 3.5, 18, 1);
    o = o.concat(q.ogeler, d.ogeler);
    o.push(yazi(44, 7, 7, "AIDAG-Chain", { kalin: true }));
    o.push(yazi(44, 11, 5, "Zincir kaydı · Mainnet"));
    o.push(yazi(44, 15, 5, utc(v.zincir.zaman).slice(0, 16) + " UTC"));
    o.push(yazi(44, 19.5, 4.6, "QR/DataMatrix: doğrula"));
    o.push(yazi(44, 22.5, 4.6, "Barkod: kısa referans"));
    var c = code128(bwip, kisaRef(h), 2, 25, W - 4, 8.5);
    o = o.concat(c.ogeler);
    o.push(yazi(W / 2, 37.6, 6, "REF " + kisaRefGoster(h), { kalin: true, hiza: "center" }));
    return {
      tur: "etiket", w: W, h: H, icAlan: [0.5, 0.5, W - 0.5, H - 0.5], ogeler: o,
      barkodlar: {
        qrcode: { icerik: dogrulamaUrl(h), modul: q.modul, boyut: q.boyut },
        datamatrix: { icerik: dogrulamaUrl(h), modul: d.modul, boyut: d.boyut },
        code128: { icerik: kisaRef(h), modul: c.modul, moduller: c.moduller }
      }
    };
  }

  // ── Çiziciler ──
  var PT_MM = 0.3528; // 1 pt = 0.3528 mm
  function kacis(s) { return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;"); }
  function f(n) { return (Math.round(n * 1000) / 1000).toString(); }

  function svg(s) {
    var p = ['<svg xmlns="http://www.w3.org/2000/svg" width="' + s.w + 'mm" height="' + s.h + 'mm" viewBox="0 0 ' + s.w + " " + s.h + '">',
      '<rect x="0" y="0" width="' + s.w + '" height="' + s.h + '" fill="#fff"/>'];
    s.ogeler.forEach(function (e) {
      if (e.t === "rect") p.push('<rect x="' + f(e.x) + '" y="' + f(e.y) + '" width="' + f(e.w) + '" height="' + f(e.h) + '" fill="#000" shape-rendering="crispEdges"/>');
      else if (e.t === "cerceve") p.push('<rect x="' + f(e.x) + '" y="' + f(e.y) + '" width="' + f(e.w) + '" height="' + f(e.h) + '" fill="none" stroke="#000" stroke-width="' + f(e.kalinlik) + '"/>');
      else if (e.t === "text") p.push('<text x="' + f(e.x) + '" y="' + f(e.y) + '" font-family="\'DejaVu Sans\',Arial,sans-serif" font-size="' + f(e.pt * PT_MM) + '"' +
        (e.kalin ? ' font-weight="700"' : "") + (e.hiza === "center" ? ' text-anchor="middle"' : "") + ' fill="#000">' + kacis(e.yazi) + "</text>");
    });
    p.push("</svg>");
    return p.join("");
  }

  // jsPDF: tarayıcıda window.jspdf.jsPDF, node'da require("jspdf").jsPDF.
  // fontlar: { normal: <TTF base64>, kalin: <TTF base64> } — Türkçe karakterler (ğ ş ı İ) için.
  function pdf(s, JsPDF, fontlar) {
    var doc = new JsPDF({ unit: "mm", format: [s.w, s.h], orientation: s.w > s.h ? "landscape" : "portrait", compress: true });
    doc.addFileToVFS("DejaVuSans-tr.ttf", fontlar.normal);
    doc.addFont("DejaVuSans-tr.ttf", "DejaVu", "normal");
    doc.addFileToVFS("DejaVuSans-Bold-tr.ttf", fontlar.kalin);
    doc.addFont("DejaVuSans-Bold-tr.ttf", "DejaVu", "bold");
    doc.setProperties({ title: "AIDAG-Chain belge " + s.tur, subject: "Belge zincir kaydı", creator: "aidag-chain.com (tarayıcı)" });
    doc.setFillColor(0, 0, 0); doc.setDrawColor(0, 0, 0); doc.setTextColor(0, 0, 0);
    s.ogeler.forEach(function (e) {
      if (e.t === "rect") doc.rect(e.x, e.y, e.w, e.h, "F");
      else if (e.t === "cerceve") { doc.setLineWidth(e.kalinlik); doc.rect(e.x, e.y, e.w, e.h, "S"); }
      else if (e.t === "text") {
        doc.setFont("DejaVu", e.kalin ? "bold" : "normal"); doc.setFontSize(e.pt);
        doc.text(e.yazi, e.x, e.y, { align: e.hiza === "center" ? "center" : "left" });
      }
    });
    return doc;
  }

  function dosyaAdi(s, hash) { return "aidag-belge-" + s.tur + "-" + kisaRef(hash).toLowerCase() + ".pdf"; }

  return { SITE: SITE, KURUM_UYARI: KURUM_UYARI, dogrulamaUrl: dogrulamaUrl, kisaRef: kisaRef, kisaRefGoster: kisaRefGoster,
    utc: utc, sema: sema, svg: svg, pdf: pdf, dosyaAdi: dosyaAdi,
    kurumMetni: kurumMetni, kurumAdi: kurumAdi, kurumDurumu: kurumDurumu, kurumSatirlari: kurumSatirlari };
});
