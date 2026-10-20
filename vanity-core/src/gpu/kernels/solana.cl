/* MIT OpenCL Solana vanity — SHA-512 + ed25519 (TweetNaCl public-domain
 * algorithms, independent OpenCL port) + Base58.
 * Returned secret is the 32-byte seed (ed25519-dalek / solana-keygen compatible).
 * Host always re-derives before export.
 */

#pragma OPENCL EXTENSION cl_khr_global_int32_base_atomics : enable

typedef uchar u8;
typedef uint u32;
typedef ulong u64;
typedef long i64;
typedef i64 gf[16];

/* ========== SHA-512 (single block, len ≤ 111) ========== */

constant u64 K512[80] = {
0x428a2f98d728ae22UL,0x7137449123ef65cdUL,0xb5c0fbcfec4d3b2fUL,0xe9b5dba58189dbbcUL,
0x3956c25bf348b538UL,0x59f111f1b605d019UL,0x923f82a4af194f9bUL,0xab1c5ed5da6d8118UL,
0xd807aa98a3030242UL,0x12835b0145706fbeUL,0x243185be4ee4b28cUL,0x550c7dc3d5ffb4e2UL,
0x72be5d74f27b896fUL,0x80deb1fe3b1696b1UL,0x9bdc06a725c71235UL,0xc19bf174cf692694UL,
0xe49b69c19ef14ad2UL,0xefbe4786384f25e3UL,0x0fc19dc68b8cd5b5UL,0x240ca1cc77ac9c65UL,
0x2de92c6f592b0275UL,0x4a7484aa6ea6e483UL,0x5cb0a9dcbd41fbd4UL,0x76f988da831153b5UL,
0x983e5152ee66dfabUL,0xa831c66d2db43210UL,0xb00327c898fb213fUL,0xbf597fc7beef0ee4UL,
0xc6e00bf33da88fc2UL,0xd5a79147930aa725UL,0x06ca6351e003826fUL,0x142929670a0e6e70UL,
0x27b70a8546d22ffcUL,0x2e1b21385c26c926UL,0x4d2c6dfc5ac42aedUL,0x53380d139d95b3dfUL,
0x650a73548baf63deUL,0x766a0abb3c77b2a8UL,0x81c2c92e47edaee6UL,0x92722c851482353bUL,
0xa2bfe8a14cf10364UL,0xa81a664bbc423001UL,0xc24b8b70d0f89791UL,0xc76c51a30654be30UL,
0xd192e819d6ef5218UL,0xd69906245565a910UL,0xf40e35855771202aUL,0x106aa07032bbd1b8UL,
0x19a4c116b8d2d0c8UL,0x1e376c085141ab53UL,0x2748774cdf8eeb99UL,0x34b0bcb5e19b48a8UL,
0x391c0cb3c5c95a63UL,0x4ed8aa4ae3418acbUL,0x5b9cca4f7763e373UL,0x682e6ff3d6b2b8a3UL,
0x748f82ee5defb2fcUL,0x78a5636f43172f60UL,0x84c87814a1f0ab72UL,0x8cc702081a6439ecUL,
0x90befffa23631e28UL,0xa4506cebde82bde9UL,0xbef9a3f7b2c67915UL,0xc67178f2e372532bUL,
0xca273eceea26619cUL,0xd186b8c721c0c207UL,0xeada7dd6cde0eb1eUL,0xf57d4f7fee6ed178UL,
0x06f067aa72176fbaUL,0x0a637dc5a2c898a6UL,0x113f9804bef90daeUL,0x1b710b35131c471bUL,
0x28db77f523047d84UL,0x32caab7b40c72493UL,0x3c9ebe0a15c9bebcUL,0x431d67c49c100d4cUL,
0x4cc5d4becb3e42b6UL,0x597f299cfc657e2aUL,0x5fcb6fab3ad6faecUL,0x6c44198c4a475817UL
};

inline u64 rotr64(u64 x, int n) { return (x >> n) | (x << (64 - n)); }

void sha512(const u8 *msg, int len, u8 out[64]) {
    u64 h0=0x6a09e667f3bcc908UL,h1=0xbb67ae8584caa73bUL,h2=0x3c6ef372fe94f82bUL,h3=0xa54ff53a5f1d36f1UL;
    u64 h4=0x510e527fade682d1UL,h5=0x9b05688c2b3e6c1fUL,h6=0x1f83d9abfb41bd6bUL,h7=0x5be0cd19137e2179UL;
    u8 block[128];
    for (int i = 0; i < 128; i++) block[i] = 0;
    for (int i = 0; i < len; i++) block[i] = msg[i];
    block[len] = 0x80;
    u64 bitlen = (u64)len * 8;
    for (int i = 0; i < 8; i++) block[127 - i] = (u8)(bitlen >> (8 * i));
    u64 w[80];
    for (int i = 0; i < 16; i++) {
        w[i] = 0;
        for (int b = 0; b < 8; b++) w[i] = (w[i] << 8) | block[i * 8 + b];
    }
    for (int i = 16; i < 80; i++) {
        u64 s0 = rotr64(w[i-15],1) ^ rotr64(w[i-15],8) ^ (w[i-15] >> 7);
        u64 s1 = rotr64(w[i-2],19) ^ rotr64(w[i-2],61) ^ (w[i-2] >> 6);
        w[i] = w[i-16] + s0 + w[i-7] + s1;
    }
    u64 a=h0,b=h1,c=h2,d=h3,e=h4,f=h5,g=h6,hh=h7;
    for (int i = 0; i < 80; i++) {
        u64 S1 = rotr64(e,14) ^ rotr64(e,18) ^ rotr64(e,41);
        u64 ch = (e & f) ^ ((~e) & g);
        u64 t1 = hh + S1 + ch + K512[i] + w[i];
        u64 S0 = rotr64(a,28) ^ rotr64(a,34) ^ rotr64(a,39);
        u64 maj = (a & b) ^ (a & c) ^ (b & c);
        u64 t2 = S0 + maj;
        hh=g; g=f; f=e; e=d+t1; d=c; c=b; b=a; a=t1+t2;
    }
    h0+=a; h1+=b; h2+=c; h3+=d; h4+=e; h5+=f; h6+=g; h7+=hh;
    u64 hs[8] = {h0,h1,h2,h3,h4,h5,h6,h7};
    for (int i = 0; i < 8; i++)
        for (int b = 0; b < 8; b++)
            out[i * 8 + b] = (u8)(hs[i] >> (56 - 8 * b));
}

/* ========== ed25519 field (TweetNaCl) ========== */

constant i64 ED_D[16] = {
    0x78a3,0x1359,0x4dca,0x75eb,0xd8ab,0x4141,0x0a4d,0x0070,
    0xe898,0x7779,0x4079,0x8cc7,0xfe73,0x2b6f,0x6cee,0x5203
};
constant i64 ED_D2[16] = {
    0xf159,0x26b2,0x9b94,0xebd6,0xb156,0x8283,0x149a,0x00e0,
    0xd130,0xeef3,0x80f2,0x198e,0xfce7,0x56df,0xd9dc,0x2406
};
constant i64 ED_X[16] = {
    0xd51a,0x8f25,0x2d60,0xc956,0xa7b2,0x9525,0xc760,0x692c,
    0xdc5c,0xfdd6,0xe231,0xc0a4,0x53fe,0xcd6e,0x36d3,0x2169
};
constant i64 ED_Y[16] = {
    0x6658,0x6666,0x6666,0x6666,0x6666,0x6666,0x6666,0x6666,
    0x6666,0x6666,0x6666,0x6666,0x6666,0x6666,0x6666,0x6666
};

void gf0(gf r) { for (int i = 0; i < 16; i++) r[i] = 0; }
void gf1(gf r) { gf0(r); r[0] = 1; }
void set25519(gf r, const gf a) { for (int i = 0; i < 16; i++) r[i] = a[i]; }
void set25519_c(gf r, __constant const i64 *a) { for (int i = 0; i < 16; i++) r[i] = a[i]; }

void car25519(gf o) {
    i64 c = 1;
    for (int i = 0; i < 16; i++) {
        i64 v = o[i] + c + 65535;
        c = v >> 16;
        o[i] = v - c * 65536;
    }
    o[0] += c - 1 + 37 * (c - 1);
}

void sel25519(gf p, gf q, int b) {
    i64 c = ~(b - 1);
    for (int i = 0; i < 16; i++) {
        i64 t = c & (p[i] ^ q[i]);
        p[i] ^= t;
        q[i] ^= t;
    }
}

void pack25519(u8 *o, const gf n) {
    gf m, t;
    set25519(t, n);
    car25519(t); car25519(t); car25519(t);
    for (int j = 0; j < 2; j++) {
        m[0] = t[0] - 0xffed;
        for (int i = 1; i < 15; i++) {
            m[i] = t[i] - 0xffff - ((m[i - 1] >> 16) & 1);
            m[i - 1] &= 0xffff;
        }
        m[15] = t[15] - 0x7fff - ((m[14] >> 16) & 1);
        int b = (int)((m[15] >> 16) & 1);
        m[14] &= 0xffff;
        sel25519(t, m, 1 - b);
    }
    for (int i = 0; i < 16; i++) {
        o[2 * i] = (u8)(t[i] & 0xff);
        o[2 * i + 1] = (u8)(t[i] >> 8);
    }
}

int par25519(const gf a) {
    u8 d[32];
    pack25519(d, a);
    return d[0] & 1;
}

void A_(gf o, const gf a, const gf b) { for (int i = 0; i < 16; i++) o[i] = a[i] + b[i]; }
void Z_(gf o, const gf a, const gf b) { for (int i = 0; i < 16; i++) o[i] = a[i] - b[i]; }

void M_(gf o, const gf a, const gf b) {
    i64 t[31];
    for (int i = 0; i < 31; i++) t[i] = 0;
    for (int i = 0; i < 16; i++)
        for (int j = 0; j < 16; j++)
            t[i + j] += a[i] * b[j];
    for (int i = 0; i < 15; i++) t[i] += 38 * t[i + 16];
    for (int i = 0; i < 16; i++) o[i] = t[i];
    car25519(o);
    car25519(o);
}

void S_(gf o, const gf a) { M_(o, a, a); }

void inv25519(gf o, const gf i) {
    gf c;
    set25519(c, i);
    for (int a = 253; a >= 0; a--) {
        S_(c, c);
        if (a != 2 && a != 4) M_(c, c, i);
    }
    set25519(o, c);
}

/* Extended twisted Edwards point: p[0]=X, p[1]=Y, p[2]=Z, p[3]=T */

void ed_add(gf p[4], gf q[4]) {
    gf a, b, c, d, e, f, g, h, t, d2;
    set25519_c(d2, ED_D2);
    Z_(a, p[1], p[0]);
    Z_(t, q[1], q[0]);
    M_(a, a, t);
    A_(b, p[0], p[1]);
    A_(t, q[0], q[1]);
    M_(b, b, t);
    M_(c, p[3], q[3]);
    M_(c, c, d2);
    M_(d, p[2], q[2]);
    A_(d, d, d);
    Z_(e, b, a);
    Z_(f, d, c);
    A_(g, d, c);
    A_(h, b, a);
    M_(p[0], e, f);
    M_(p[1], h, g);
    M_(p[2], g, f);
    M_(p[3], e, h);
}

void ed_cswap(gf p[4], gf q[4], int b) {
    for (int i = 0; i < 4; i++) sel25519(p[i], q[i], b);
}

void ed_pack(u8 r[32], gf p[4]) {
    gf tx, ty, zi;
    inv25519(zi, p[2]);
    M_(tx, p[0], zi);
    M_(ty, p[1], zi);
    pack25519(r, ty);
    r[31] ^= (u8)(par25519(tx) << 7);
}

void scalarmult(gf p[4], gf q[4], const u8 s[32]) {
    gf0(p[0]); gf1(p[1]); gf1(p[2]); gf0(p[3]);
    for (int i = 255; i >= 0; --i) {
        int b = (s[i / 8] >> (i & 7)) & 1;
        ed_cswap(p, q, b);
        ed_add(q, p);
        ed_add(p, p);
        ed_cswap(p, q, b);
    }
}

void scalarbase(gf p[4], const u8 s[32]) {
    gf q[4];
    set25519_c(q[0], ED_X);
    set25519_c(q[1], ED_Y);
    gf1(q[2]);
    M_(q[3], q[0], q[1]);
    scalarmult(p, q, s);
}

/* seed → pubkey (ed25519-dalek / Solana compatible) */
void ed25519_pubkey_from_seed(const u8 seed[32], u8 pk[32]) {
    u8 h[64];
    sha512(seed, 32, h);
    h[0] &= 248;
    h[31] &= 127;
    h[31] |= 64;
    gf p[4];
    scalarbase(p, h);
    ed_pack(pk, p);
}

/* ========== Base58 ========== */

constant char B58[] = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

int base58_encode_32(const u8 in[32], u8 out[64]) {
    u8 digits[64];
    int len = 0;
    u8 buf[32];
    for (int i = 0; i < 32; i++) buf[i] = in[i];
    int zeros = 0;
    while (zeros < 32 && buf[zeros] == 0) zeros++;
    int start = zeros;
    while (start < 32) {
        int rem = 0;
        for (int i = start; i < 32; i++) {
            int val = rem * 256 + buf[i];
            buf[i] = (u8)(val / 58);
            rem = val % 58;
        }
        digits[len++] = (u8)rem;
        while (start < 32 && buf[start] == 0) start++;
    }
    int o = 0;
    for (int i = 0; i < zeros; i++) out[o++] = (u8)'1';
    for (int i = len - 1; i >= 0; i--) out[o++] = (u8)B58[digits[i]];
    return o;
}

/* Pattern match inlined in sol_grind (Apple OpenCL helper-pointer workaround). */

__kernel void sol_grind(
    __global const u8 *seed,
    ulong batch_base,
    __global const u8 *pat,
    __global volatile int *found,
    __global u8 *out_sk
) {
    size_t gid = get_global_id(0);
    if (*found >= 0) return;

    u8 msg[40];
    for (int i = 0; i < 32; i++) msg[i] = seed[i];
    ulong ctr = batch_base + (ulong)gid;
    msg[32] = (u8)(ctr >> 56); msg[33] = (u8)(ctr >> 48);
    msg[34] = (u8)(ctr >> 40); msg[35] = (u8)(ctr >> 32);
    msg[36] = (u8)(ctr >> 24); msg[37] = (u8)(ctr >> 16);
    msg[38] = (u8)(ctr >> 8);  msg[39] = (u8)ctr;

    u8 h[64];
    sha512(msg, 40, h);
    u8 sk[32];
    for (int i = 0; i < 32; i++) sk[i] = h[i];

    u8 pk[32];
    ed25519_pubkey_from_seed(sk, pk);

    u8 addr[64];
    int alen = base58_encode_32(pk, addr);

    /* Inline match — same Apple OpenCL helper-pointer issue as EVM. */
    {
        uint plen = (uint)pat[4] | ((uint)pat[5] << 8) | ((uint)pat[6] << 16) | ((uint)pat[7] << 24);
        uint slen = (uint)pat[8] | ((uint)pat[9] << 8) | ((uint)pat[10] << 16) | ((uint)pat[11] << 24);
        uint clen = (uint)pat[12] | ((uint)pat[13] << 8) | ((uint)pat[14] << 16) | ((uint)pat[15] << 24);
        uint ignore_case = (uint)pat[16] | ((uint)pat[17] << 8) | ((uint)pat[18] << 16) | ((uint)pat[19] << 24);
        int ok = 1;
        for (uint i = 0; i < plen; i++) {
            u8 a = addr[i], b = pat[20 + i];
            if (ignore_case) {
                if (a >= 'A' && a <= 'Z') a += 32;
                if (b >= 'A' && b <= 'Z') b += 32;
            }
            if (a != b) { ok = 0; break; }
        }
        for (uint i = 0; i < slen && ok; i++) {
            u8 a = addr[alen - (int)slen + (int)i], b = pat[52 + i];
            if (ignore_case) {
                if (a >= 'A' && a <= 'Z') a += 32;
                if (b >= 'A' && b <= 'Z') b += 32;
            }
            if (a != b) { ok = 0; break; }
        }
        if (ok && clen) {
            ok = 0;
            for (int s = 0; s + (int)clen <= alen; s++) {
                int m = 1;
                for (uint j = 0; j < clen; j++) {
                    u8 a = addr[s + (int)j], b = pat[84 + j];
                    if (ignore_case) {
                        if (a >= 'A' && a <= 'Z') a += 32;
                        if (b >= 'A' && b <= 'Z') b += 32;
                    }
                    if (a != b) { m = 0; break; }
                }
                if (m) { ok = 1; break; }
            }
        }
        if (!ok) return;
    }

    if (atom_cmpxchg(found, -1, (int)gid) == -1) {
        for (int i = 0; i < 32; i++) out_sk[i] = sk[i];
    }
}
