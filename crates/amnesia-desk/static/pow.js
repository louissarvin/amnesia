// Issuance proof of work: find nonce with SHA-256(salt || id || nonce) having `bits` leading
// zero bits. Pure JS so it needs no secure-context APIs. Without JS the form still posts.
(function () {
  "use strict";
  var K = new Uint32Array([
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2
  ]);
  var W = new Uint32Array(64);

  // Returns the first 32 bits of SHA-256(msg). Enough to count up to 32 leading zero bits.
  function sha256First(msg) {
    var len = msg.length, total = ((len + 9 + 63) >> 6) << 6;
    var buf = new Uint8Array(total);
    buf.set(msg);
    buf[len] = 0x80;
    var bitLen = len * 8;
    buf[total - 4] = bitLen >>> 24; buf[total - 3] = bitLen >>> 16;
    buf[total - 2] = bitLen >>> 8; buf[total - 1] = bitLen;
    var h0 = 0x6a09e667, h1 = 0xbb67ae85, h2 = 0x3c6ef372, h3 = 0xa54ff53a,
        h4 = 0x510e527f, h5 = 0x9b05688c, h6 = 0x1f83d9ab, h7 = 0x5be0cd19;
    for (var off = 0; off < total; off += 64) {
      for (var i = 0; i < 16; i++) {
        var j = off + i * 4;
        W[i] = (buf[j] << 24) | (buf[j + 1] << 16) | (buf[j + 2] << 8) | buf[j + 3];
      }
      for (i = 16; i < 64; i++) {
        var a = W[i - 15], b = W[i - 2];
        var s0 = ((a >>> 7) | (a << 25)) ^ ((a >>> 18) | (a << 14)) ^ (a >>> 3);
        var s1 = ((b >>> 17) | (b << 15)) ^ ((b >>> 19) | (b << 13)) ^ (b >>> 10);
        W[i] = (W[i - 16] + s0 + W[i - 7] + s1) | 0;
      }
      var A = h0, B = h1, C = h2, D = h3, E = h4, F = h5, G = h6, H = h7;
      for (i = 0; i < 64; i++) {
        var S1 = ((E >>> 6) | (E << 26)) ^ ((E >>> 11) | (E << 21)) ^ ((E >>> 25) | (E << 7));
        var t1 = (H + S1 + ((E & F) ^ (~E & G)) + K[i] + W[i]) | 0;
        var S0 = ((A >>> 2) | (A << 30)) ^ ((A >>> 13) | (A << 19)) ^ ((A >>> 22) | (A << 10));
        var t2 = (S0 + ((A & B) ^ (A & C) ^ (B & C))) | 0;
        H = G; G = F; F = E; E = (D + t1) | 0; D = C; C = B; B = A; A = (t1 + t2) | 0;
      }
      h0 = (h0 + A) | 0; h1 = (h1 + B) | 0; h2 = (h2 + C) | 0; h3 = (h3 + D) | 0;
      h4 = (h4 + E) | 0; h5 = (h5 + F) | 0; h6 = (h6 + G) | 0; h7 = (h7 + H) | 0;
    }
    return h0 >>> 0;
  }

  function hex(s) {
    var out = new Uint8Array(s.length / 2);
    for (var i = 0; i < out.length; i++) out[i] = parseInt(s.substr(i * 2, 2), 16);
    return out;
  }

  var form = document.getElementById("f");
  if (!form) return;
  var bits = parseInt(form.getAttribute("data-bits"), 10) || 0;
  form.addEventListener("submit", function (ev) {
    if (bits === 0 || bits > 32) return;
    ev.preventDefault();
    var btn = form.querySelector("button");
    btn.disabled = true;
    btn.textContent = "Working...";
    var prefix = new Uint8Array(32);
    prefix.set(hex(form.getAttribute("data-salt")), 0);
    prefix.set(hex(form.getAttribute("data-id")), 16);
    var mask = bits === 32 ? 0xffffffff : ((0xffffffff << (32 - bits)) >>> 0);
    var n = 0;
    function step() {
      for (var k = 0; k < 20000; k++, n++) {
        var digits = String(n);
        var msg = new Uint8Array(32 + digits.length);
        msg.set(prefix);
        for (var d = 0; d < digits.length; d++) msg[32 + d] = digits.charCodeAt(d);
        if ((sha256First(msg) & mask) === 0) {
          form.elements.nonce.value = digits;
          form.submit();
          return;
        }
      }
      setTimeout(step, 0);
    }
    step();
  });
})();
