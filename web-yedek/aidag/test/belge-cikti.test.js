// Belge çıktısı testleri (node). Sitede YAYINLANAN dosyaları test eder:
//   lib/belge-cikti.js, lib/bwip-js-4.11.4.min.js (tarayıcı paketi), lib/jspdf-4.2.1.umd.min.js, lib/fonts/*.ttf
// Gerekenler (test-only, repoya girmez):
//   npm i @zxing/library@0.23.0          -> barkodları GÖRÜNTÜDEN geri okumak için
//   pip install --target <dir> pypdfium2 -> üretilen PDF'i piksele çevirmek için (PYTHONPATH=<dir>)
// Çalıştırma: NODE_PATH=<node_modules> PYTHONPATH=<py> node test/belge-cikti.test.js
"use strict";
const fs = require("fs"), path = require("path"), vm = require("vm"), os = require("os");
const { execFileSync } = require("child_process");
const ZX = require("@zxing/library");

const LIB = path.join(__dirname, "..", "lib");
const BelgeCikti = require(path.join(LIB, "belge-cikti.js"));
const { jsPDF } = require(path.join(LIB, "jspdf-4.2.1.umd.min.js"));
const ctx = { window: {}, self: {}, console, atob, btoa }; ctx.globalThis = ctx; vm.createContext(ctx);
vm.runInContext(fs.readFileSync(path.join(LIB, "bwip-js-4.11.4.min.js"), "utf8"), ctx);
const bwipKok = ctx.bwipjs || ctx.window.bwipjs || ctx.self.bwipjs;
if (!bwipKok || typeof bwipKok.raw !== "function") throw new Error("bwip-js tarayici paketi yuklenemedi");
// Secenek nesnesi paketin KENDI baglaminda kurulmali (vm: farkli realm -> "options not an object").
ctx.__bwip = bwipKok;
const hamRaw = vm.runInContext("(function(j){ return __bwip.raw(JSON.parse(j)); })", ctx);
const bwip = { raw: o => hamRaw(JSON.stringify(o)) };
const fontlar = {
  normal: fs.readFileSync(path.join(LIB, "fonts", "DejaVuSans-tr.ttf")).toString("base64"),
  kalin: fs.readFileSync(path.join(LIB, "fonts", "DejaVuSans-Bold-tr.ttf")).toString("base64"),
};

let gecen = 0, kalan = 0;
function kontrol(ad, ok, ek) { if (ok) gecen++; else kalan++; console.log((ok ? "GECTI " : "KALDI ") + ad + (ek ? "  [" + ek + "]" : "")); }
function atar(fn) { try { fn(); return false; } catch (e) { return true; } }

const HASH = "0dcce43d9a705bcd6f3b3a8a1b2c3d4e5f60718293a4b5c6d7e8f90112233445";
const VERI = {
  hash: HASH, olusturma: 1758660000,
  zincir: { kayitli: true, kaydeden: "0x11c1906e07508e0b83ef4afa042879281e196b9f", zaman: 1758650000 },
  kurum: { kayitli: true, ad: "Örnek Tapu Müdürlüğü", kategori: "devlet", dogrulanmis: null },
};
const URL_ = "https://aidag-chain.com/belge/" + HASH;

// ── Saf kurallar ──
kontrol("dogrulama adresi aidag-chain.com/belge/<hash>", BelgeCikti.dogrulamaUrl(HASH) === URL_);
kontrol("kisa referans = ilk 16 hane, buyuk harf", BelgeCikti.kisaRef(HASH) === "0DCCE43D9A705BCD" && BelgeCikti.kisaRefGoster(HASH) === "0DCC-E43D-9A70-5BCD");
kontrol("gecersiz hash reddedilir", atar(() => BelgeCikti.dogrulamaUrl(HASH.slice(1))) && atar(() => BelgeCikti.dogrulamaUrl(HASH.toUpperCase())));
kontrol("kayitsiz belge icin cikti URETILMEZ", atar(() => BelgeCikti.sema("kapak", { ...VERI, zincir: { kayitli: false } }, bwip)));
kontrol("bilinmeyen sablon reddedilir", atar(() => BelgeCikti.sema("afis", VERI, bwip)));

const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "belge-cikti-"));
const semalar = { kapak: BelgeCikti.sema("kapak", VERI, bwip), etiket: BelgeCikti.sema("etiket", VERI, bwip) };

// ── Barkod ölçüleri (baskı okunabilirliği) ──
const e = semalar.etiket.barkodlar;
kontrol("etiket Code128 modul >= 0.25 mm (eski okuyucular)", e.code128.modul >= 0.25, e.code128.modul.toFixed(3) + " mm");
kontrol("etiket QR modul >= 0.33 mm", e.qrcode.modul >= 0.33, e.qrcode.modul.toFixed(3) + " mm");
kontrol("etiket DataMatrix modul >= 0.33 mm", e.datamatrix.modul >= 0.33, e.datamatrix.modul.toFixed(3) + " mm");
kontrol("sablon olculeri: kapak A4, etiket 70x40", semalar.kapak.w === 210 && semalar.kapak.h === 297 && semalar.etiket.w === 70 && semalar.etiket.h === 40);
for (const [tur, s] of Object.entries(semalar)) {
  kontrol(`${tur}: tum ogeler sayfa icinde`, s.ogeler.every(o => o.t === "text" ? (o.x >= 0 && o.x <= s.w && o.y > 0 && o.y <= s.h) : (o.x >= 0 && o.y >= 0 && o.x + o.w <= s.w + 0.05 && o.y + o.h <= s.h + 0.05)));
}

// ── SVG (Yazdır yolu) ──
for (const [tur, s] of Object.entries(semalar)) {
  const svg = BelgeCikti.svg(s);
  const rectSay = (svg.match(/fill="#000" shape-rendering/g) || []).length;
  kontrol(`${tur} SVG: barkod dikdortgen sayisi semayla ayni`, rectSay === s.ogeler.filter(o => o.t === "rect").length);
  kontrol(`${tur} SVG: dis kaynak/betik yok`, !/<script|href=|xlink|url\(/i.test(svg));
  kontrol(`${tur} SVG: boyut mm cinsinden`, svg.includes(`width="${s.w}mm" height="${s.h}mm"`));
}
const kapakSvg = BelgeCikti.svg(semalar.kapak);
kontrol("kapak: tam hash (iki satir) yazili", kapakSvg.includes(HASH.slice(0, 32)) && kapakSvg.includes(HASH.slice(32)));
kontrol("kapak: kurum BEYAN olarak etiketli (dogrulanmis iddiasi yok)", kapakSvg.includes("beyan (doğrulanmamış)") && !kapakSvg.includes("doğrulanmış kurum"));
const dogKurum = BelgeCikti.svg(BelgeCikti.sema("kapak", { ...VERI, kurum: { ...VERI.kurum, dogrulanmis: true } }, bwip));
kontrol("kapak: zincir dogrulanmis derse 'dogrulanmis kurum' yazar", dogKurum.includes("doğrulanmış kurum") && !dogKurum.includes("beyan (doğrulanmamış)"));
const kurumsuz = BelgeCikti.svg(BelgeCikti.sema("kapak", { ...VERI, kurum: null }, bwip));
kontrol("kapak: kurum kaydi yoksa bunu acikca yazar", kurumsuz.includes("kurum olarak kayıtlı değil"));

// ── PDF (PDF indir yolu): üret → PDFium ile 600 dpi piksele çevir → barkodları görüntüden oku ──
const pdfYol = {};
for (const [tur, s] of Object.entries(semalar)) {
  const doc = BelgeCikti.pdf(s, jsPDF, fontlar);
  const buf = Buffer.from(doc.output("arraybuffer"));
  pdfYol[tur] = path.join(tmp, BelgeCikti.dosyaAdi(s, HASH));
  fs.writeFileSync(pdfYol[tur], buf);
  kontrol(`${tur} PDF: gecerli baslik + tek sayfa`, buf.slice(0, 5).toString() === "%PDF-" && doc.getNumberOfPages() === 1);
  kontrol(`${tur} PDF: Turkce font gomulu`, buf.includes("FontFile2"));
}
// Her metin, gomulu fontun GERCEK genisligiyle cercevenin icinde kalmali (tasma yok).
for (const [tur, s] of Object.entries(semalar)) {
  const doc = BelgeCikti.pdf(s, jsPDF, fontlar), [x0, , x1] = s.icAlan, tasan = [];
  for (const o of s.ogeler.filter(o => o.t === "text")) {
    doc.setFont("DejaVu", o.kalin ? "bold" : "normal"); doc.setFontSize(o.pt);
    const w = doc.getTextWidth(o.yazi), sol = o.hiza === "center" ? o.x - w / 2 : o.x;
    if (sol < x0 + 0.5 || sol + w > x1 - 0.5) tasan.push(o.yazi.slice(0, 40) + " (" + w.toFixed(1) + "mm)");
  }
  kontrol(`${tur}: hicbir metin cerceveden tasmaz`, tasan.length === 0, tasan.join(" | "));
}
kontrol("PDF dosya adi", path.basename(pdfYol.etiket) === "aidag-belge-etiket-0dcce43d9a705bcd.pdf");

// Kırpma kutuları (mm) — şablondaki barkod konumları (sessiz bölgeler dahil).
const KUTU = {
  kapak: { qrcode: null },
  etiket: { qrcode: [2, 2, 21, 21], datamatrix: [24, 3.5, 18, 18], code128: [0, 24, 70, 10.5] },
};
const qk = semalar.kapak.ogeler.filter(o => o.t === "rect" && o.y > 100);
const qx0 = Math.min(...qk.map(o => o.x)), qy0 = Math.min(...qk.map(o => o.y));
KUTU.kapak.qrcode = [qx0 - 6, qy0 - 6, 74, 74];
const PY = `
import sys, json, pypdfium2 as pdfium
yol, kutular, cikti = sys.argv[1], json.loads(sys.argv[2]), sys.argv[3]
pdf = pdfium.PdfDocument(yol); sayfa = pdf[0]
olcek = 600/72
img = sayfa.render(scale=olcek, grayscale=True).to_pil().convert("L")
mm = 600/25.4
meta = {}
for ad,(x,y,w,h) in kutular.items():
    k = img.crop((int(x*mm), int(y*mm), int((x+w)*mm), int((y+h)*mm)))
    open(f"{cikti}/{ad}.gray","wb").write(k.tobytes()); meta[ad] = k.size
tp = sayfa.get_textpage(); metin = tp.get_text_range()
open(f"{cikti}/metin.txt","w",encoding="utf-8").write(metin)
print(json.dumps(meta))
`;
function oku(bicim, dosya, w, h) {
  const lum = new Uint8ClampedArray(fs.readFileSync(dosya));
  const bmp = new ZX.BinaryBitmap(new ZX.HybridBinarizer(new ZX.RGBLuminanceSource(lum, w, h)));
  const ipucu = new Map([[ZX.DecodeHintType.POSSIBLE_FORMATS, [bicim]], [ZX.DecodeHintType.TRY_HARDER, true]]);
  const r = new ZX.MultiFormatReader(); r.setHints(ipucu);
  // zxing ic denemelerini console'a basar; okuma sirasinda sustur.
  const eski = { log: console.log, error: console.error, warn: console.warn };
  console.log = console.error = console.warn = () => {};
  try { return r.decode(bmp).getText(); } finally { Object.assign(console, eski); }
}
const BICIM = { qrcode: ZX.BarcodeFormat.QR_CODE, datamatrix: ZX.BarcodeFormat.DATA_MATRIX, code128: ZX.BarcodeFormat.CODE_128 };
for (const tur of ["kapak", "etiket"]) {
  const dir = fs.mkdtempSync(path.join(tmp, tur + "-"));
  const meta = JSON.parse(execFileSync("python3", ["-c", PY, pdfYol[tur], JSON.stringify(KUTU[tur]), dir], { encoding: "utf8" }));
  for (const [ad, [w, h]] of Object.entries(meta)) {
    let okunan = null, hata = "";
    try { okunan = oku(BICIM[ad], path.join(dir, ad + ".gray"), w, h); } catch (err) { hata = String(err && err.name || err); }
    const beklenen = semalar[tur].barkodlar[ad].icerik;
    kontrol(`${tur} PDF (600 dpi goruntu): ${ad} okunur ve icerik dogru`, okunan === beklenen, okunan || hata);
  }
  const metin = fs.readFileSync(path.join(dir, "metin.txt"), "utf8");
  if (tur === "kapak") {
    kontrol("kapak PDF metni: Turkce karakterler dogru", metin.includes("Belge Zincir Kaydı Kapağı") && metin.includes("Örnek Tapu Müdürlüğü") && metin.includes("beyan (doğrulanmamış)"));
    kontrol("kapak PDF metni: tam hash + kayit zamani + adres", metin.includes(HASH.slice(0, 32)) && metin.includes(HASH.slice(32)) && metin.includes("aidag-chain.com/belge/" + HASH.slice(0, 32)) && metin.includes("2025-09-23 17:53:20 UTC") && metin.includes(VERI.zincir.kaydeden));
  } else {
    kontrol("etiket PDF metni: insan-okur kisa referans", metin.includes("REF 0DCC-E43D-9A70-5BCD"));
  }
}
fs.rmSync(tmp, { recursive: true, force: true });
console.log(`\nSONUC: ${gecen}/${gecen + kalan} gecti`);
process.exit(kalan ? 1 : 0);
