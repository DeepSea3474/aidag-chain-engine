// XSS / sahtecilik testleri (node + jsdom). Sitede YAYINLANAN sayfaları test eder:
//   ask.html, ask-stream.html, gorsel.html, belge-dogrulama.html (+ soulware-web/ask.html, katil.html)
// Gerekenler (test-only, repoya girmez): npm i jsdom
// Çalıştırma: NODE_PATH=<node_modules> node web-yedek/aidag/test/xss.test.js
// Ağ YOK: fetch sahte (mock); jsdom alt kaynak (script/img) yüklemez; canlı servise istek gitmez.
"use strict";
const fs = require("fs"), path = require("path"), vm = require("vm");
const { TextEncoder, TextDecoder } = require("util");
const { JSDOM } = require("jsdom");

const WEB = path.join(__dirname, "..");
const SOUL = path.join(__dirname, "..", "..", "..", "soulware-web");
const oku = p => fs.readFileSync(p, "utf8");
const SITE = "https://aidag-chain.com";

let gecen = 0, kalan = 0;
function kontrol(ad, ok, ek) { if (ok) gecen++; else kalan++; console.log((ok ? "GECTI " : "KALDI ") + ad + (ek ? "  [" + ek + "]" : "")); }
const bekle = ms => new Promise(r => setTimeout(r, ms));
async function kadar(kosul, ms = 3000) { const son = Date.now() + ms; while (Date.now() < son) { if (kosul()) return true; await bekle(5); } return kosul(); }

// ── DOM değişmezleri: kök altında hiçbir saldırı izi olmamalı ──
const TEHLIKELI_ETIKET = new Set(["SCRIPT", "IFRAME", "OBJECT", "EMBED", "SVG", "MATH", "FORM", "BASE", "LINK", "META", "STYLE", "FRAME", "TEMPLATE"]);
function domIhlalleri(kok, izinliEtiketler) {
  const v = [];
  for (const el of kok.querySelectorAll("*")) {
    const tag = el.tagName.toUpperCase();
    if (TEHLIKELI_ETIKET.has(tag)) v.push("etiket " + tag);
    if (izinliEtiketler && !izinliEtiketler.includes(tag)) v.push("izinsiz etiket " + tag);
    for (const a of el.attributes) {
      if (/^on/i.test(a.name)) v.push(tag + " " + a.name + "=" + a.value);
      if (/^(href|src|action|formaction|xlink:href)$/i.test(a.name)) {
        const d = a.value.replace(/[\u0000- ]/g, "");
        if (/^(javascript|data|vbscript):/i.test(d)) v.push(tag + " " + a.name + "=" + a.value);
        if (tag === "A") {
          let u; try { u = new URL(a.value, SITE + "/ask"); } catch (e) { v.push("gecersiz href " + a.value); continue; }
          if (u.protocol !== "https:") v.push("https disi href " + a.value);
          if (a.value.startsWith("//")) v.push("protokolsuz dis href " + a.value);
        }
        if (tag === "IMG" && !/^blob:/.test(a.value)) v.push("img src " + a.value);
      }
    }
    if (tag === "A" && el.getAttribute("target") === "_blank" && !/noopener/.test(el.getAttribute("rel") || "")) v.push("target=_blank rel=noopener yok: " + el.getAttribute("href"));
  }
  return v;
}

// ── 1) GUVENLI bloğu: tüm kopyalar birebir aynı ──
const BLOK_RE = /\/\/ GUVENLI-BASLA[\s\S]*?\/\/ GUVENLI-BITIS/;
const kopyalar = {
  "ask.html": path.join(WEB, "ask.html"), "ask-stream.html": path.join(WEB, "ask-stream.html"),
  "gorsel.html": path.join(WEB, "gorsel.html"), "soulware-web/ask.html": path.join(SOUL, "ask.html"),
};
const bloklar = Object.fromEntries(Object.entries(kopyalar).map(([k, p]) => [k, (oku(p).match(BLOK_RE) || [""])[0]]));
kontrol("GUVENLI blogu 4 dosyada da var ve birebir ayni", Object.values(bloklar).every(b => b && b === bloklar["ask.html"]));
kontrol("ask.html ile ask-stream.html ayni dosya", oku(kopyalar["ask.html"]) === oku(kopyalar["ask-stream.html"]));

const cam = new JSDOM("<!doctype html><body></body>", { url: SITE + "/ask", runScripts: "outside-only" }).window;
let G = null;
try { cam.eval(bloklar["ask.html"] + "\n;window.__G = { esc, guvenliUrl, linkifyDom, linkEkle };"); G = cam.__G; } catch (e) { kontrol("GUVENLI blogu calistirilabilir", false, e.message); }
if (G) {

// ── 2) esc: tam kaçış (" ve ' dahil) ──
kontrol("esc: & < > \" ' kacar", G.esc(`&<>"'`) === "&amp;&lt;&gt;&quot;&#39;");
for (const p of [`" onmouseover="alert(1)`, `' onmouseover='alert(1)`, `"><img src=x onerror=alert(1)>`, `'><svg onload=alert(1)>`]) {
  const d = cam.document.createElement("div");
  d.innerHTML = `<span title="${G.esc(p)}">x</span><span title='${G.esc(p)}'>y</span>`;
  const v = domIhlalleri(d, ["SPAN"]);
  kontrol("esc ozniteliktan cikamaz: " + p, v.length === 0 && d.querySelectorAll("span").length === 2 && d.firstChild.getAttribute("title") === p, v.join("; "));
}

// ── 3) guvenliUrl: yalnız https: ve site-içi yol ──
const RED = ["javascript:alert(1)", "JaVaScRiPt:alert(1)", " javascript:alert(1)", "java\tscript:alert(1)", "java\u200bscript:alert(1)",
  "\u0000javascript:alert(1)", "ｊａｖａｓｃｒｉｐｔ:alert(1)", "javascript&colon;alert(1)", "data:text/html;base64,PHNjcmlwdD5hbGVydCgxKTwvc2NyaXB0Pg==",
  "vbscript:msgbox(1)", "http://aidag-chain.com/ask", "//evil.com", "/\\evil.com", "/.//evil.com", "\\\\evil.com", "https:\\\\evil.com",
  "https://user:pw@evil.com", "https://x\"onmouseover=\"alert(1)", "https://x'onmouseover='alert(1)", "https://aidag-chain.com\u202e/moc.live",
  "https://evil.com\u2028x", "https://ev\u200bil.com", "https://evil.com/<script>", "file:///etc/passwd", "ftp://x", "blob:https://x/1", "", null, 42];
for (const u of RED) kontrol("guvenliUrl reddeder: " + JSON.stringify(u), G.guvenliUrl(u) === null, String(G.guvenliUrl(u)));
const KABUL = [["https://aidag-chain.com/ask", "https://aidag-chain.com/ask"], ["/rpc/belge/abc", "/rpc/belge/abc"],
  ["https://example.org/a?b=c#d", "https://example.org/a?b=c#d"], ["HTTPS://AIDAG-CHAIN.COM/katil", "https://aidag-chain.com/katil"]];
for (const [u, b] of KABUL) kontrol("guvenliUrl kabul eder: " + u, G.guvenliUrl(u) === b, String(G.guvenliUrl(u)));

// ── 4) linkifyDom: saldırı yükleri ──
function isle(txt) { const d = cam.document.createElement("div"); G.linkifyDom(d, txt); return d; }
const SALDIRI = [
  `[a](https://x"onmouseover="alert(1))`,                     // bulgu: ozniteliktan cikis (markdown)
  `[a](https://x'onmouseover='alert(1))`,
  `https://x"onmouseover="alert(1)" y`,                        // duz URL + cift tirnak
  `https://x'onmouseover='alert(1)' y`,
  `[tikla](javascript:alert(1))`, `[tikla](JaVaScRiPt:alert(1))`, `[t](javascript&colon;alert(1))`, `[t](\u0000javascript:alert(1))`,
  `[t](data:text/html;base64,PHNjcmlwdD5hbGVydCgxKTwvc2NyaXB0Pg==)`, `[t](vbscript:msgbox(1))`,
  `[t](//evil.com)`, `[t](/\\evil.com)`, `[t](/.//evil.com)`, `[t](http://evil.com)`,
  `<img src=x onerror=alert(1)>`, `<script>alert(1)</script>`, `<svg onload=alert(1)>`, `"><img src=x onerror=alert(1)>`, `'><img src=x onerror=alert(1)>`,
  `[<img src=x onerror=alert(1)>](https://aidag-chain.com/ask)`, `[x](https://aidag-chain.com/ask"><script>alert(1)</script>)`,
  `https://aidag-chain.com/ask?q=<img/src=x/onerror=alert(1)>`, `aidag-chain.com/ask"onmouseover="alert(1)`,
  `[t](ｊａｖａｓｃｒｉｐｔ:alert(1))`, `[t](java\u200bscript:alert(1))`, `[t](https://aidag-chain.com\u202e/moc.live)`,
  `https://evil.com\u2028onmouseover=1`, `＜img src=x onerror=alert(1)＞`, `&lt;img src=x onerror=alert(1)&gt;`,
  `[a](https://ok.com) <iframe src=javascript:alert(1)>`, "`\"'><math><mi xlink:href=javascript:alert(1)>",
];
for (const p of SALDIRI) {
  const d = isle(p), v = domIhlalleri(d, ["A"]);
  const metinKorunur = p.includes("](") || d.textContent === p;   // markdown disinda metin birebir korunur
  kontrol("linkify saldiri: " + JSON.stringify(p).slice(0, 70), v.length === 0 && metinKorunur, v.join("; ") || (metinKorunur ? "" : "metin degisti: " + d.textContent));
}
kontrol("linkify: HTML varlik metni cozulmez (&lt; duz metin kalir)", isle("&lt;b&gt;").textContent === "&lt;b&gt;" && isle("&lt;b&gt;").children.length === 0);

// ── 5) linkifyDom: normal görünüm korunur ──
function linkler(d) { return [...d.querySelectorAll("a")].map(a => [a.textContent, a.getAttribute("href")]); }
const j = x => JSON.stringify(x);
let d = isle("Bak: https://aidag-chain.com/ask.");
kontrol("duz URL link olur, sondaki nokta metinde kalir", j(linkler(d)) === j([["https://aidag-chain.com/ask", "https://aidag-chain.com/ask"]]) && d.textContent === "Bak: https://aidag-chain.com/ask.");
d = isle("Detay: [KUBRA'ya sor](https://aidag-chain.com/ask) ve devam");
kontrol("markdown link: metin + https href", j(linkler(d)) === j([["KUBRA'ya sor", "https://aidag-chain.com/ask"]]) && d.textContent === "Detay: KUBRA'ya sor ve devam");
d = isle("Katılmak için aidag-chain.com/katil adresine git (www.aidag-chain.com/ask)");
kontrol("protokolsuz aidag-chain.com/xxx kisaltmasi link olur", j(linkler(d)) === j([["aidag-chain.com/katil", "https://aidag-chain.com/katil"], ["www.aidag-chain.com/ask", "https://www.aidag-chain.com/ask"]]));
d = isle("evil-aidag-chain.com/ask ve xaidag-chain.com/ask");
kontrol("baska alan adinin parcasi olan aidag-chain.com link yapilmaz", d.querySelectorAll("a").length === 0);
d = isle("Kaynak: https://example.org/yol?a=1&b=2, sonra [dok](https://docs.example.org/x#y).");
kontrol("dis https linkleri calisir, & korunur", j(linkler(d)) === j([["https://example.org/yol?a=1&b=2", "https://example.org/yol?a=1&b=2"], ["dok", "https://docs.example.org/x#y"]]));
kontrol("linkler target=_blank + rel=noopener noreferrer", [...d.querySelectorAll("a")].every(a => a.target === "_blank" && a.rel === "noopener noreferrer"));
d = isle("satir1\nsatir2 https://aidag-chain.com/how\n");
kontrol("satir sonlari korunur (pre-wrap)", d.textContent === "satir1\nsatir2 https://aidag-chain.com/how\n" && d.querySelectorAll("a").length === 1);

} // if (G)

// ── 6) Sayfa entegrasyonu (tam HTML, sahte fetch) ──
function sayfa(dosya, url, fetchFn, ekHazirlik) {
  let html = oku(dosya);
  return new JSDOM(html, {
    url, runScripts: "dangerously", pretendToBeVisual: true,
    beforeParse(w) {
      w.TextEncoder = TextEncoder; w.TextDecoder = TextDecoder;
      w.fetch = fetchFn; w.__uyari = []; w.alert = m => w.__uyari.push(m); w.confirm = () => true;
      w.URL.createObjectURL = () => "blob:" + SITE + "/00000000-0000-0000-0000-000000000000"; w.URL.revokeObjectURL = () => {};
      w.HTMLElement.prototype.scrollIntoView = () => {};
      const st = w.setTimeout.bind(w); w.setTimeout = (f, ms) => st(f, Math.min(ms || 0, 5));
      if (ekHazirlik) ekHazirlik(w);
    },
  }).window;
}
const yanit = (govde, o = {}) => ({ ok: o.ok !== false, status: o.status || 200, headers: { get: k => (o.basliklar || {})[k.toLowerCase()] || null },
  json: async () => govde, text: async () => (typeof govde === "string" ? govde : JSON.stringify(govde)), blob: async () => ({}), body: o.body || null });
function sse(olaylar) {
  const enc = new TextEncoder(), parca = olaylar.map(([ev, data]) => enc.encode(`event: ${ev}\ndata: ${data}\n\n`));
  let i = 0; return { getReader: () => ({ read: async () => (i < parca.length ? { value: parca[i++], done: false } : { done: true }) }) };
}
const HASH = "ab".repeat(32), SALT = "cd".repeat(32);
const XSS_CEVAP = `Merhaba [a](https://x"onmouseover="alert(1)) <img src=x onerror=alert(1)> [j](javascript:alert(1)) "><svg onload=alert(1)> — resmi sayfa: https://aidag-chain.com/ask`;

(async () => {
  // 6a) ask.html (akış): token + kaynak + done (kötü verify_path, kötü ts) + error
  for (const ad of ["ask.html", "ask-stream.html"]) {
    const w = sayfa(path.join(WEB, ad), SITE + "/ask", async () => yanit(null, { body: sse([
      ["sources", JSON.stringify([{ baslik: "<img src=x onerror=alert(2)>" }])],
      ...XSS_CEVAP.split(/(?= )/).map(p => ["token", p]),
      ["done", JSON.stringify({ proof_hash: HASH, salt: SALT, ts: "<img src=x onerror=alert(3)>", chain: { verify_path: "javascript:alert(4)" } })],
    ]) }));
    w.document.getElementById("soru").value = "KUBRA nedir?"; w.document.getElementById("sor").click();
    const k = w.document.getElementById("konusma");
    await kadar(() => !w.document.getElementById("sor").disabled && k.querySelector(".proof"));
    const v = domIhlalleri(k, ["DIV", "A", "SPAN", "CODE", "BUTTON"]);
    const proof = k.querySelector("a.proof");
    kontrol(`${ad}: KUBRA cevabinda saldiri izi yok (on*, script, img, javascript:)`, v.length === 0, v.join("; "));
    kontrol(`${ad}: kotu verify_path reddedildi -> /rpc/belge/<hash>`, proof && proof.getAttribute("href") === "/rpc/belge/" + HASH && proof.rel === "noopener noreferrer");
    kontrol(`${ad}: guvenli link hala calisir`, [...k.querySelectorAll(".cevap a:not(.proof)")].some(a => a.getAttribute("href") === "https://aidag-chain.com/ask"));
    kontrol(`${ad}: alert hic cagrilmadi`, w.__uyari.length === 0);
    // kötü proof_hash -> kanıt linki hiç çizilmez
    const w2 = sayfa(path.join(WEB, ad), SITE + "/ask", async () => yanit(null, { body: sse([["token", "x"], ["done", JSON.stringify({ proof_hash: `"><img src=x onerror=alert(5)>` + "a".repeat(40) })]]) }));
    w2.document.getElementById("soru").value = "s"; w2.document.getElementById("sor").click();
    await kadar(() => !w2.document.getElementById("sor").disabled);
    const k2 = w2.document.getElementById("konusma");
    kontrol(`${ad}: 64-hex olmayan proof_hash cizilmez`, !k2.querySelector(".proof") && domIhlalleri(k2).length === 0);
    // error olayı
    const w3 = sayfa(path.join(WEB, ad), SITE + "/ask", async () => yanit(null, { body: sse([["error", `<img src=x onerror=alert(6)>`]]) }));
    w3.document.getElementById("soru").value = "s"; w3.document.getElementById("sor").click();
    await kadar(() => !w3.document.getElementById("sor").disabled);
    const k3 = w3.document.getElementById("konusma");
    kontrol(`${ad}: sunucu error metni duz metin`, domIhlalleri(k3, ["DIV", "SPAN"]).length === 0 && k3.textContent.includes("<img src=x onerror=alert(6)>"));
  }

  // 6b) gorsel.html: hata metni + kötü x-kubra-verify
  {
    const w = sayfa(path.join(WEB, "gorsel.html"), SITE + "/gorsel", async () => yanit(`<img src=x onerror=alert(1)><a href="javascript:alert(2)">x</a>`, { ok: false, status: 400 }));
    w.document.getElementById("istem").value = "kedi"; w.document.getElementById("uret").click();
    const s = w.document.getElementById("sahne");
    await kadar(() => s.querySelector(".err"));
    kontrol("gorsel.html: sunucu hata metni duz metin (bulgu 2)", domIhlalleri(s, ["SPAN"]).length === 0 && s.textContent.includes("<img src=x onerror=alert(1)>"), domIhlalleri(s).join("; "));
    for (const [ver, beklenen] of [[`javascript:alert(1)`, null], [`" onmouseover="alert(1)`, null], [`/rpc/belge/${HASH}`, `/rpc/belge/${HASH}`], [`//evil.com/x`, null]]) {
      const w2 = sayfa(path.join(WEB, "gorsel.html"), SITE + "/gorsel", async () => yanit({}, { basliklar: { "x-kubra-verify": ver } }));
      w2.document.getElementById("istem").value = "kedi"; w2.document.getElementById("uret").click();
      const s2 = w2.document.getElementById("sahne");
      await kadar(() => s2.querySelector("#indir"));
      const a = s2.querySelector(".cert a");
      const v = domIhlalleri(s2, ["IMG", "DIV", "B", "LABEL", "INPUT", "BUTTON", "A"]);
      kontrol("gorsel.html: x-kubra-verify " + JSON.stringify(ver) + (beklenen ? " -> link" : " -> link YOK"), v.length === 0 && (beklenen ? a && a.getAttribute("href") === beklenen : !a) && s2.querySelector("img").getAttribute("src").startsWith("blob:"), v.join("; "));
    }
  }

  // 6c) soulware-web/ask.html: iş kuyruğu cevabı + görsel paneli
  {
    const w = sayfa(path.join(SOUL, "ask.html"), SITE + "/ask", async u => /job\/create/.test(u) ? yanit({ job_id: "1" }) : yanit({ ok: true, job: { status: "verified", verified_answer: XSS_CEVAP } }));
    w.document.getElementById("soru").value = "KUBRA nedir?"; w.document.getElementById("sor").click();
    const k = w.document.getElementById("konusma");
    await kadar(() => !w.document.getElementById("sor").disabled);
    const v = domIhlalleri(k, ["DIV", "A"]);
    kontrol("soulware ask: cevapta saldiri izi yok, guvenli link var", v.length === 0 && [...k.querySelectorAll("a")].some(a => a.getAttribute("href") === "https://aidag-chain.com/ask"), v.join("; "));
    for (const [durum, ver] of [[false, null], [true, "javascript:alert(1)"]]) {
      const w2 = sayfa(path.join(SOUL, "ask.html"), SITE + "/ask", async () => durum ? yanit({}, { basliklar: { "x-kubra-verify": ver } }) : yanit(`<img src=x onerror=alert(1)>`, { ok: false, status: 500 }));
      w2.document.getElementById("soru").value = "bir resim ciz"; w2.document.getElementById("sor").click();
      const k2 = w2.document.getElementById("konusma");
      await kadar(() => k2.querySelector(".inrow button")); k2.querySelector(".inrow button").click();
      await kadar(() => !k2.querySelector(".inrow button").disabled && (durum ? k2.querySelector("img") : k2.textContent.includes("🚫")));
      const v2 = domIhlalleri(k2, ["DIV", "A", "INPUT", "BUTTON", "SPAN", "IMG"]);
      kontrol("soulware ask gorsel: " + (durum ? "kotu verify basligi link olmaz" : "hata metni duz metin"), v2.length === 0 && (durum ? !k2.querySelector(".src a") : k2.textContent.includes("<img src=x onerror=alert(1)>")), v2.join("; "));
    }
  }

  // 6d) belge-dogrulama.html: zincirden gelen kurum adı / talep alanları
  {
    let html = oku(path.join(WEB, "belge-dogrulama.html"));
    const ic = f => "<script>" + oku(path.join(WEB, "lib", f)) + "</script>";
    kontrol("belge-dogrulama: belge-cikti.js onbellek surumu artirildi", html.includes('/lib/belge-cikti.js?v=2"'));
    html = html.replace('<script src="/lib/blake3.js"></script>', "<script>window.blake3hash = b => new Uint8Array(32).fill(7);</script>")
      .replace('<script src="/lib/nacl.min.js"></script>', "<script>window.nacl = {};</script>")
      .replace(/<script src="\/lib\/belge-kayit\.js[^"]*"><\/script>/, () => ic("belge-kayit.js"))
      .replace(/<script src="\/lib\/belge-cikti\.js[^"]*"><\/script>/, () => ic("belge-cikti.js"));
    const tmp = path.join(require("os").tmpdir(), "belge-dogrulama-test.html"); fs.writeFileSync(tmp, html);
    const SAHTE = "\u202eXYZ\u202c Tapu — doğrulanmış kurum\u2066\" ”<img src=x onerror=alert(1)>";
    const kurum = { kayitli: true, ad: SAHTE, kategori: "devlet", dogrulanmis: null };
    const w = sayfa(tmp, SITE + "/belge", async (u, o) => {
      if (/\/status$/.test(u)) return yanit({ vertex_count: 1 });
      if (/\/rpc\/belge\//.test(u)) return yanit({ kayitli: true, kaydeden: "11".repeat(20), zaman: 1758650000 });
      if (/\/rpc\/kurum\//.test(u)) return yanit(kurum);
      if (/belge\/hazirla/.test(u)) return yanit({ ok: true, talep: { zincir: { kayitli: false }, arac: `<img src=x onerror=alert(2)>`, belge_hash: HASH,
        durum: `"'><svg onload=alert(3)>`, sonraki_adim: "<script>alert(4)</script>", imzalayan: { adres: `<b onclick=alert(5)>`, kurum } } });
      return yanit({});
    });
    fs.unlinkSync(tmp);
    const $ = id => w.document.getElementById(id);
    $("dogrulaMetin").value = HASH; $("dogrulaBtn").click();
    await kadar(() => $("ciktiIz").textContent.includes("kurum") && /beyan|doğrulanmış/.test($("ciktiIz").textContent));
    const IZIN = ["DIV", "B", "CODE"];
    const satir = [...$("ciktiIz").querySelectorAll(".s")].find(s => s.querySelector("b").textContent === "kurum");
    const durum = satir && satir.querySelector("b.kurumDurum");
    const YASAK = /[\u0000-\u001f\u007f-\u009f\u061c\u200e\u200f\u202a-\u202e\u2066-\u2069]/;
    kontrol("belge-dogrulama cikti izi: kurum adinda saldiri izi yok", domIhlalleri($("ciktiIz"), IZIN).length === 0, domIhlalleri($("ciktiIz")).join("; "));
    kontrol("belge-dogrulama cikti izi: bidi/kontrol karakteri silindi", satir && !YASAK.test(satir.textContent), satir && JSON.stringify(satir.textContent));
    kontrol("belge-dogrulama cikti izi: ad “tirnak” icinde, etiket ayri <b> ve 'beyan (dogrulanmamis)'",
      satir && /“[^“”"]*”/.test(satir.textContent) && durum && durum.textContent === "— beyan (doğrulanmamış)");
    $("belgeMetin").value = "diploma"; $("hazirlaBtn").click();
    await kadar(() => $("talepIz").textContent.includes("imzalayan"));
    const v = domIhlalleri($("talepIz"), IZIN);
    kontrol("belge-dogrulama talep izi: KUBRA talep alanlari kacisli (arac, durum, not, adres, kurum)", v.length === 0 && $("talepIz").textContent.includes("<script>alert(4)</script>") && $("talepIz").querySelectorAll("b.kurumDurum").length === 1, v.join("; "));
    kontrol("belge-dogrulama: alert hic cagrilmadi", w.__uyari.length === 0);
  }

  // ── 7) Kaynak taraması: bilinen tehlikeli kalıplar geri gelmesin ──
  const SAYFALAR = [...fs.readdirSync(WEB).filter(f => f.endsWith(".html")).map(f => path.join(WEB, f)), ...fs.readdirSync(SOUL).filter(f => f.endsWith(".html")).map(f => path.join(SOUL, f))];
  const KALIP = [
    [/innerHTML\s*=\s*linkify\(/, "innerHTML = linkify("],
    [/function linkify\(/, "string-HTML linkify"],
    [/innerHTML[^;\n]*\+\s*(msg|m|verify|verifyPath|blobUrl|metin|ans|data)\s*\+/, "dinamik veri innerHTML'e kacissiz"],
    [/innerHTML[^;\n]*proof\.proof_hash/, "proof_hash innerHTML'e"],
    [/href="'\s*\+\s*(verify|verifyPath|vp)\b/, "sunucu URL'si href string'ine"],
    [/\(e\.message\)\s*\+/, "hata mesaji kacissiz"],
  ];
  for (const p of SAYFALAR) {
    const s = oku(p), bulunan = KALIP.filter(([r]) => r.test(s)).map(([, a]) => a);
    kontrol("kaynak taramasi: " + path.relative(path.join(WEB, "..", ".."), p), bulunan.length === 0, bulunan.join(", "));
  }

  // ── 8) Sözdizimi: tüm sayfaların satır içi script blokları + lib ──
  const hatalar = [];
  for (const p of SAYFALAR) {
    const s = oku(p); let m, i = 0; const re = /<script(?![^>]*\bsrc=)[^>]*>([\s\S]*?)<\/script>/g;
    while ((m = re.exec(s))) { i++; try { new vm.Script(m[1], { filename: p + "#" + i }); } catch (e) { hatalar.push(path.basename(p) + "#" + i + ": " + e.message); } }
  }
  for (const f of ["belge-cikti.js", "belge-kayit.js"]) { try { new vm.Script(oku(path.join(WEB, "lib", f))); } catch (e) { hatalar.push(f + ": " + e.message); } }
  kontrol("sozdizimi: tum satir ici script bloklari + lib gecerli", hatalar.length === 0, hatalar.join(" | "));

  console.log(`\nSONUC: ${gecen}/${gecen + kalan} gecti`);
  process.exit(kalan ? 1 : 0);
})().catch(e => { console.error("TEST HATASI:", e); process.exit(1); });
