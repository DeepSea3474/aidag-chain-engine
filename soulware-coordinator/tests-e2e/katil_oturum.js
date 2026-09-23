// katil.html tarayici isci <-> GERCEK soulware-coordinator (izole ag ad alani) uctan uca.
// Oturum anahtari akisi (MetaMask BIR KEZ imzalar) + saldiri testleri (baskasi adina 401).
//
// Kullanim (root; ayri ag ad alani, canli servislere DOKUNMAZ):
//   D=$(mktemp -d); cp katil_oturum.js <soulware-coordinator ikilisi> $D/
//   cd $D && env -u ANTHROPIC_API_KEY NODE_PATH=<jsdom iceren node_modules> COORD_BIN=$D/soulware-coordinator \
//     REPO=<depo koku> unshare -n bash -c 'ip link set lo up && node katil_oturum.js'
// Gerekli: node >= 18 + jsdom, python3 eth_account + blake3. KATIL=<html> ile baska kopya denenir.
const fs=require("fs"), path=require("path"), cp=require("child_process");
const {JSDOM}=require("jsdom");
const D=process.cwd(), REPO=process.env.REPO||path.resolve(__dirname,"../..");
const HTML=process.env.KATIL||REPO+"/web-yedek/aidag/katil.html";
const COORD="http://127.0.0.1:38757", BIN=process.env.COORD_BIN;
const sonuc=[]; function kontrol(ad,ok,ay=""){ sonuc.push(ok); console.log((ok?"GECTI ":"KALDI ")+ad+(ok?"":"  <- "+ay)); }
const uyu=ms=>new Promise(r=>setTimeout(r,ms));
let koord=null;
function koordBaslat(){
  const env={PATH:process.env.PATH, SOULWARE_COORD_KEY:D+"/havuz.key", SOULWARE_COORD_LISTEN:"127.0.0.1:38757",
    SOULWARE_CHAIN_RPC:"http://127.0.0.1:1", SOULWARE_NET_ID:"99999", SOULWARE_COORD_DATA:D+"/coord.json",
    SOULWARE_FREE_DAILY_PER_CLIENT:"100", SOULWARE_REDUNDANCY:"2", SOULWARE_MAX_ASSIGN:"3"};
  if(!fs.existsSync(D+"/havuz.key")) cp.execFileSync(BIN,["--yeni-anahtar-uret"],{env});
  koord=cp.spawn(BIN,[],{env,stdio:["ignore",fs.openSync(D+"/coord.out","a"),fs.openSync(D+"/coord.out","a")]});
}
async function hazirBekle(){ for(let i=0;i<200;i++){ try{ if((await fetch(COORD+"/health")).ok) return; }catch(e){} await uyu(100);} throw new Error("koordinator baslamadi"); }
async function isAc(){ const r=await (await fetch(COORD+"/job/create",{method:"POST",headers:{"content-type":"application/json"},body:JSON.stringify({prompt:"2+2?"})})).json(); if(!r.ok) throw new Error(JSON.stringify(r)); return r.job_id; }
const PY=`import sys
from eth_account import Account
from eth_account.messages import encode_defunct
k=sys.argv[1]
if sys.argv[2]=="adres": print(Account.from_key(k).address)
else: print(Account.sign_message(encode_defunct(hexstr=sys.argv[3]),k).signature.hex())`;
const py=(...a)=>cp.execFileSync("python3",["-c",PY,...a]).toString().trim();

function sayfaYukle({ethereum=null}={}){
  let h=fs.readFileSync(HTML,"utf8");
  const lib=f=>fs.readFileSync(REPO+"/web/lib/"+f,"utf8");
  h=h.replace('<script src="/lib/nacl.min.js"></script>',()=>"<script>"+lib("nacl.min.js")+"</script>")
     .replace('<script src="/lib/blake3.js"></script>',()=>"<script>"+lib("blake3.js")+"</script>")
     .replace("<head>",()=>'<head><script>window.KUBRA_COORD="'+COORD+'";window.KUBRA_BRAIN="http://127.0.0.1:1";</script>');
  const istekler=[];
  const dom=new JSDOM(h,{runScripts:"dangerously",url:"https://aidag-chain.com/katil",pretendToBeVisual:true,
    beforeParse(w){
      Object.defineProperty(w.navigator,"gpu",{value:{requestAdapter:async()=>({})}});
      w.TextEncoder=class{ encode(s){ return w.Uint8Array.from(new TextEncoder().encode(s)); } }; w.URLSearchParams=URLSearchParams;
      w.fetch=async(u,o)=>{ const r=await fetch(u,o); const kl=r.clone(); istekler.push({u:String(u),o,status:r.status,govde:await kl.text()}); return r; };
      if(ethereum) w.ethereum=ethereum;
      w.crypto.getRandomValues=a=>require("crypto").getRandomValues(a);
    }});
  return {dom,w:dom.window,istekler};
}
async function motorStub(w){
  w.eval(`motorYukle=async()=>{ engine={chat:{completions:{create:async()=>({choices:[{message:{content:"  4\\n"}}]})}}}; };`);
}
const $=(w,id)=>w.document.getElementById(id);

(async()=>{
  koordBaslat(); await hazirBekle();
  // ── 1) MetaMask (EVM) cuzdan: BIR KEZ oturum imzasi, sonra oturum anahtariyla poll/submit ──
  const EVM_KEY="0x"+"11".repeat(32), EVM_W=py(EVM_KEY,"adres").toLowerCase();
  const imzalananlar=[];
  const eth={ request:async({method,params})=>{
    if(method==="eth_requestAccounts") return [EVM_W.replace("0x","0x").toUpperCase().replace("0X","0x")];
    if(method==="personal_sign"){ imzalananlar.push(params); return py(EVM_KEY,"imzala",params[0]); }
    throw new Error("desteklenmiyor "+method); } };
  let {w,istekler}=sayfaYukle({ethereum:eth}); await uyu(300); await motorStub(w);
  const is1=await isAc();
  $(w,"adres").value=EVM_W; $(w,"baslat").click();
  for(let i=0;i<50 && $(w,"katki").classList.contains("hidden");i++) await uyu(100);
  const pk=w.eval("oturumAnahtari().pkHex");
  const mm=imzalananlar.length===1 ? Buffer.from(imzalananlar[0][0].slice(2),"hex").toString() : "";
  kontrol("MetaMask yalniz BIR KEZ imzaladi, mesaj = AIDAG-WORKER|<wallet>|oturum:<pk>|<ts>",
    imzalananlar.length===1 && new RegExp("^AIDAG-WORKER\\|"+EVM_W+"\\|oturum:"+pk+"\\|\\d+$").test(mm) && imzalananlar[0][1]===EVM_W, mm);
  kontrol("oturum seed localStorage'da", /^[0-9a-f]{64}$/.test(w.localStorage.getItem("kubra_oturum_seed")||""));
  const reg=istekler.find(x=>x.u.endsWith("/worker/register"));
  kontrol("GERCEK koordinator register (EIP-191 oturum) -> 200 + oturum_bitis", reg && reg.status===200 && JSON.parse(reg.govde).oturum_bitis>0, reg&&reg.govde);
  await w.eval("katkiAdim()");
  const poll=istekler.find(x=>x.u.includes("/worker/poll/"));
  kontrol("oturum anahtariyla imzali poll -> 200 + is", poll && poll.status===200 && JSON.parse(poll.govde).job_id===is1, poll&&poll.govde);
  kontrol("poll sorgusu ts/imza/pubkey(=oturum) iceriyor", poll && new URL(poll.u).searchParams.get("pubkey")===pk);
  const sub=istekler.find(x=>x.u.endsWith("/worker/submit"));
  kontrol("oturum anahtariyla imzali submit -> 200 (cevap trim'li, is:<id>:<blake3> nonce)", sub && sub.status===200 && JSON.parse(sub.o.body).answer==="4", sub&&sub.govde);
  const job=await (await fetch(COORD+"/job/"+is1)).json();
  kontrol("is sonucu MetaMask cuzdani adina kaydedildi", JSON.stringify(job).includes(EVM_W), JSON.stringify(job).slice(0,300));

  // ── 2) SALDIRI: ayni oturum anahtariyla BASKA cuzdan (kurban) adina poll/submit -> 401 ──
  const KURBAN=py("0x"+"22".repeat(32),"adres").toLowerCase();
  const kts=Math.floor(Date.now()/1000);
  const kImza=py("0x"+"22".repeat(32),"imzala","0x"+Buffer.from(`AIDAG-WORKER|${KURBAN}|kayit|${kts}`).toString("hex"));
  const kr=await fetch(COORD+"/worker/register",{method:"POST",headers:{"content-type":"application/json"},body:JSON.stringify({wallet:KURBAN,ts:kts,imza:kImza})});
  kontrol("kurban kendi imzasiyla kayitli (kontrol)", kr.status===200);
  const is2=await isAc();
  w.eval(`cuzdan=${JSON.stringify(KURBAN)}`);  // sayfanin oturum anahtarini kurban adina kullan
  const q=new URLSearchParams(Object.entries(JSON.parse(w.eval(`JSON.stringify(oturumImza("poll"))`))).map(([k,v])=>[k,String(v)]));
  let r=await fetch(COORD+"/worker/poll/"+KURBAN+"?"+q); kontrol("SALDIRI: oturum anahtariyla kurban adina poll -> 401", r.status===401, r.status);
  const sImza=JSON.parse(w.eval(`JSON.stringify(oturumImza("is:${is2}:"+cevapOzeti("4")))`));
  r=await fetch(COORD+"/worker/submit",{method:"POST",headers:{"content-type":"application/json"},body:JSON.stringify({wallet:KURBAN,job_id:is2,answer:"4",...sImza})});
  kontrol("SALDIRI: oturum anahtariyla kurban adina submit -> 401", r.status===401, r.status);
  // saldirgan kurban icin oturum kurmaya calisir (kendi EVM anahtariyla imzalar)
  const ts=Math.floor(Date.now()/1000);
  const sahte=py(EVM_KEY,"imzala","0x"+Buffer.from(`AIDAG-WORKER|${KURBAN}|oturum:${pk}|${ts}`).toString("hex"));
  r=await fetch(COORD+"/worker/register",{method:"POST",headers:{"content-type":"application/json"},body:JSON.stringify({wallet:KURBAN,oturum_pubkey:pk,ts,imza:sahte})});
  kontrol("SALDIRI: yanlis cuzdan imzasiyla kurban icin oturum kurma -> 401", r.status===401, r.status);
  r=await fetch(COORD+"/worker/poll/"+KURBAN+"?"+new URLSearchParams(Object.entries(JSON.parse(w.eval(`JSON.stringify(oturumImza("poll"))`))).map(([k,v])=>[k,String(v)])));
  kontrol("...ve sonrasinda da kurban adina poll -> 401", r.status===401, r.status);
  r=await fetch(COORD+"/worker/poll/"+KURBAN); kontrol("imzasiz poll -> 401", r.status===401);
  w.eval(`cuzdan=${JSON.stringify(EVM_W)}`);

  // ── 3) Koordinator yeniden basladi: oturum kayboldu -> 401 -> "yeniden imzala" -> surer ──
  koord.kill("SIGTERM"); await new Promise(r=>koord.on("exit",r)); koordBaslat(); await hazirBekle();
  const n0=istekler.length; await w.eval("katkiAdim()");
  kontrol("restart sonrasi poll 401 -> katki durdu, yeniden-imza butonu gorunur",
    istekler.slice(n0).some(x=>x.status===401) && !$(w,"yenidenImza").classList.contains("hidden") && !w.eval("aktif"));
  $(w,"yenidenImza").click(); for(let i=0;i<50 && !w.eval("aktif");i++) await uyu(100);
  const is3=await isAc(); const n1=istekler.length; await w.eval("katkiAdim()");
  kontrol("yeniden imza (2. MetaMask imzasi) sonrasi poll+submit 200", imzalananlar.length===2 &&
    istekler.slice(n1).filter(x=>/poll|submit/.test(x.u)).every(x=>x.status===200) && istekler.slice(n1).some(x=>x.u.endsWith("/worker/submit")));
  w.eval("katkiDurdur()"); w.close();

  // ── 4) MetaMask'te baska hesap secili -> kayit YOK, duz metin hata ──
  const eth2={request:async({method})=>method==="eth_requestAccounts"?["0x"+"ab".repeat(20)]:(()=>{throw new Error("x")})()};
  ({w,istekler}=sayfaYukle({ethereum:eth2})); await uyu(300); await motorStub(w);
  $(w,"adres").value=EVM_W; $(w,"baslat").click(); await uyu(500);
  kontrol("yanlis MetaMask hesabi -> hata (textContent), register istegi YOK",
    !$(w,"hata").classList.contains("hidden") && $(w,"hata").children.length<=2 && /MetaMask/.test($(w,"hata").textContent) && !istekler.some(x=>x.u.includes("/worker/")), $(w,"hata").textContent);
  w.close();

  // ── 4b) MetaMask imzayi reddeder, hata metni HTML iceriyor -> duz metin gosterilir ──
  const eth3={request:async({method})=>{ if(method==="eth_requestAccounts") return [EVM_W]; throw new Error('<img src=x onerror="window.XSS=1">'); }};
  ({w,istekler}=sayfaYukle({ethereum:eth3})); await uyu(300); await motorStub(w);
  $(w,"adres").value=EVM_W; $(w,"baslat").click(); await uyu(500);
  kontrol("imza reddi: hata HTML olarak yorumlanmaz (img elemani yok), register YOK",
    $(w,"hata").querySelector("img")===null && $(w,"hata").textContent.includes("<img") && !w.XSS && !istekler.some(x=>x.u.includes("/worker/")), $(w,"hata").innerHTML);
  w.close();

  // ── 5) MetaMask yok + 0x adres -> ihtiyac mesaji; YEREL cuzdan -> dogrudan ed25519 ──
  ({w,istekler}=sayfaYukle()); await uyu(300); await motorStub(w);
  $(w,"adres").value=EVM_W; $(w,"baslat").click(); await uyu(300);
  kontrol("MetaMask yok + 0x adres -> 'MetaMask gerekli' mesaji, istek YOK", /MetaMask/.test($(w,"hata").textContent) && !istekler.some(x=>x.u.includes("/worker/")));
  $(w,"yerelBtn").click();
  const yerel=$(w,"adres").value;
  const beklenen="0x"+require("child_process").execFileSync("python3",["-c","import sys,blake3;print(blake3.blake3(bytes.fromhex(sys.argv[1])).digest()[:20].hex())",w.eval("oturumAnahtari().pkHex")]).toString().trim();
  kontrol("yerel cuzdan adresi = blake3(oturum_pk)[..20], yedek uyarisi gorunur", yerel===beklenen && !$(w,"yerelBilgi").classList.contains("hidden") && $(w,"yerelMetin").textContent.includes(yerel), yerel+" "+beklenen);
  $(w,"anahtarGoster").click();
  kontrol("anahtari goster -> seed goruntulenir", $(w,"yerelAnahtar").textContent.includes(w.localStorage.getItem("kubra_oturum_seed")));
  const is4=await isAc();
  $(w,"hata").classList.add("hidden"); $(w,"baslat").click(); for(let i=0;i<50 && $(w,"katki").classList.contains("hidden");i++) await uyu(100);
  const yreg=istekler.find(x=>x.u.endsWith("/worker/register"));
  kontrol("yerel cuzdan register (nonce kayit, ed25519) -> 200", yreg && yreg.status===200 && !("oturum_pubkey" in JSON.parse(yreg.o.body)), yreg&&yreg.govde);
  await w.eval("katkiAdim()");
  const yp=istekler.filter(x=>/poll|submit/.test(x.u));
  kontrol("yerel cuzdan poll+submit -> 200", yp.length===2 && yp.every(x=>x.status===200), JSON.stringify(yp.map(x=>[x.status,x.govde])));
  w.eval("katkiDurdur()"); w.close();

  koord.kill("SIGTERM");
  const k=sonuc.filter(x=>!x).length; console.log(`\nSONUC: ${sonuc.length-k}/${sonuc.length} gecti`); process.exit(k?1:0);
})().catch(e=>{ console.error("HATA",e); try{koord.kill()}catch(_){ } process.exit(2); });
