(function(global){
  const IV = new Uint32Array([0x6A09E667,0xBB67AE85,0x3C6EF372,0xA54FF53A,0x510E527F,0x9B05688C,0x1F83D9AB,0x5BE0CD19]);
  const MSG_PERM = [2,6,3,10,7,0,4,13,1,11,12,5,9,14,15,8];
  const CHUNK_START=1, CHUNK_END=2, ROOT=8;
  function rotr(x,n){return ((x>>>n)|(x<<(32-n)))>>>0;}
  function g(s,a,b,c,d,mx,my){
    s[a]=(s[a]+s[b]+mx)>>>0; s[d]=rotr(s[d]^s[a],16);
    s[c]=(s[c]+s[d])>>>0;    s[b]=rotr(s[b]^s[c],12);
    s[a]=(s[a]+s[b]+my)>>>0; s[d]=rotr(s[d]^s[a],8);
    s[c]=(s[c]+s[d])>>>0;    s[b]=rotr(s[b]^s[c],7);
  }
  function round(s,m){
    g(s,0,4,8,12,m[0],m[1]); g(s,1,5,9,13,m[2],m[3]);
    g(s,2,6,10,14,m[4],m[5]); g(s,3,7,11,15,m[6],m[7]);
    g(s,0,5,10,15,m[8],m[9]); g(s,1,6,11,12,m[10],m[11]);
    g(s,2,7,8,13,m[12],m[13]); g(s,3,4,9,14,m[14],m[15]);
  }
  function permute(m){const out=new Uint32Array(16);for(let i=0;i<16;i++) out[i]=m[MSG_PERM[i]];return out;}
  function compress(cv,block,counter,blockLen,flags){
    let m=Uint32Array.from(block);
    const s=new Uint32Array(16);
    s.set(cv.subarray(0,8),0); s.set(IV.subarray(0,4),8);
    s[12]=counter>>>0; s[13]=Math.floor(counter/0x100000000)>>>0;
    s[14]=blockLen>>>0; s[15]=flags>>>0;
    for(let r=0;r<7;r++){ round(s,m); if(r<6) m=permute(m); }
    const out=new Uint32Array(8);
    for(let i=0;i<8;i++) out[i]=(s[i]^s[i+8])>>>0;
    return out;
  }
  function wordsFromLE(bytes){
    const w=new Uint32Array(16);
    for(let i=0;i<16;i++){const o=i*4;w[i]=(bytes[o]|(bytes[o+1]<<8)|(bytes[o+2]<<16)|(bytes[o+3]<<24))>>>0;}
    return w;
  }
  function hash(input){
    let cv=Uint32Array.from(IV);
    const n=input.length;
    let nBlocks=Math.ceil(n/64); if(nBlocks===0) nBlocks=1;
    for(let b=0;b<nBlocks;b++){
      const start=b*64;
      const blk=new Uint8Array(64);
      const len=Math.min(64,n-start);
      blk.set(input.subarray(start,start+len));
      let flags=0;
      if(b===0) flags|=CHUNK_START;
      if(b===nBlocks-1) flags|=CHUNK_END|ROOT;
      const blockLen=(b===nBlocks-1)?len:64;
      cv=compress(cv,wordsFromLE(blk),0,blockLen,flags);
    }
    const out=new Uint8Array(32);
    for(let i=0;i<8;i++){out[i*4]=cv[i]&0xff;out[i*4+1]=(cv[i]>>>8)&0xff;out[i*4+2]=(cv[i]>>>16)&0xff;out[i*4+3]=(cv[i]>>>24)&0xff;}
    return out;
  }
  global.blake3hash=hash;
})(typeof window!=='undefined'?window:globalThis);
