/* MIT — independent OpenCL EVM vanity (secp256k1 + Keccak-256).
 * Candidates are always re-derived on CPU before export.
 * OpenCL 1.2 + cl_khr_global_int32_base_atomics.
 */

#pragma OPENCL EXTENSION cl_khr_global_int32_base_atomics : enable

typedef uchar u8;
typedef uint u32;
typedef ulong u64;

/* ========== Keccak-256 ========== */

constant u64 RC[24] = {
    0x0000000000000001UL, 0x0000000000008082UL, 0x800000000000808aUL,
    0x8000000080008000UL, 0x000000000000808bUL, 0x0000000080000001UL,
    0x8000000080008081UL, 0x8000000000008009UL, 0x000000000000008aUL,
    0x0000000000000088UL, 0x0000000080008009UL, 0x000000008000000aUL,
    0x000000008000808bUL, 0x800000000000008bUL, 0x8000000000008089UL,
    0x8000000000008003UL, 0x8000000000008002UL, 0x8000000000000080UL,
    0x000000000000800aUL, 0x800000008000000aUL, 0x8000000080008081UL,
    0x8000000000008080UL, 0x0000000080000001UL, 0x8000000080008008UL
};
constant int ROTC[24] = {
    1,3,6,10,15,21,28,36,45,55,2,14,27,41,56,8,25,43,62,18,39,61,20,44
};
constant int PILN[24] = {
    10,7,11,17,18,3,5,16,8,21,24,4,15,23,19,13,12,2,20,14,22,9,6,1
};

inline u64 rotl64(u64 x, int n) { return (x << n) | (x >> (64 - n)); }

void keccak_f(u64 st[25]) {
    for (int round = 0; round < 24; round++) {
        u64 bc[5];
        for (int i = 0; i < 5; i++)
            bc[i] = st[i] ^ st[i+5] ^ st[i+10] ^ st[i+15] ^ st[i+20];
        for (int i = 0; i < 5; i++) {
            u64 t = bc[(i+4)%5] ^ rotl64(bc[(i+1)%5], 1);
            for (int j = 0; j < 25; j += 5) st[j+i] ^= t;
        }
        u64 t = st[1];
        for (int i = 0; i < 24; i++) {
            int j = PILN[i];
            bc[0] = st[j];
            st[j] = rotl64(t, ROTC[i]);
            t = bc[0];
        }
        for (int j = 0; j < 25; j += 5) {
            for (int i = 0; i < 5; i++) bc[i] = st[j+i];
            for (int i = 0; i < 5; i++)
                st[j+i] ^= (~bc[(i+1)%5]) & bc[(i+2)%5];
        }
        st[0] ^= RC[round];
    }
}

void keccak256(const u8 *data, int len, u8 out[32]) {
    u64 st[25];
    for (int i = 0; i < 25; i++) st[i] = 0;
    const int rate = 136;
    int off = 0;
    while (len >= rate) {
        for (int i = 0; i < rate/8; i++) {
            u64 v = 0;
            for (int b = 0; b < 8; b++) v |= ((u64)data[off + i*8 + b]) << (8*b);
            st[i] ^= v;
        }
        keccak_f(st);
        off += rate; len -= rate;
    }
    u8 tmp[200];
    for (int i = 0; i < 200; i++) tmp[i] = 0;
    for (int i = 0; i < len; i++) tmp[i] = data[off+i];
    tmp[len] ^= 0x01;
    tmp[rate-1] ^= 0x80;
    for (int i = 0; i < rate/8; i++) {
        u64 v = 0;
        for (int b = 0; b < 8; b++) v |= ((u64)tmp[i*8+b]) << (8*b);
        st[i] ^= v;
    }
    keccak_f(st);
    for (int i = 0; i < 4; i++) {
        u64 v = st[i];
        for (int b = 0; b < 8; b++) out[i*8+b] = (u8)(v >> (8*b));
    }
}

/* ========== secp256k1 (8×u32 LE limbs) ========== */

typedef struct { u32 d[8]; } fe;
typedef struct { fe x, y; int inf; } ge;

constant u32 P[8] = {
    0xFFFFFC2Fu,0xFFFFFFFEu,0xFFFFFFFFu,0xFFFFFFFFu,
    0xFFFFFFFFu,0xFFFFFFFFu,0xFFFFFFFFu,0xFFFFFFFFu
};
constant u32 N[8] = {
    0xD0364141u,0xBFD25E8Cu,0xAF48A03Bu,0xBAAEDCE6u,
    0xFFFFFFFEu,0xFFFFFFFFu,0xFFFFFFFFu,0xFFFFFFFFu
};
constant u32 GX[8] = {
    0x16F81798u,0x59F2815Bu,0x2DCE28D9u,0x029BFCDBu,
    0xCE870B07u,0x55A06295u,0xF9DCBBACu,0x79BE667Eu
};
constant u32 GY[8] = {
    0xFB10D4B8u,0x9C47D08Fu,0xA6855419u,0xFD17B448u,
    0x0E1108A8u,0x5DA4FBFCu,0x26A3C465u,0x483ADA77u
};

inline void fe0(fe *a){ for(int i=0;i<8;i++) a->d[i]=0; }
inline void fe1(fe *a){ fe0(a); a->d[0]=1; }
inline void fecpy(fe *r,const fe *a){ for(int i=0;i<8;i++) r->d[i]=a->d[i]; }
inline int fecmp(const fe *a,const fe *b){
    for(int i=7;i>=0;i--){ if(a->d[i]<b->d[i])return -1; if(a->d[i]>b->d[i])return 1; }
    return 0;
}
inline int fe_is0(const fe *a){ for(int i=0;i<8;i++) if(a->d[i]) return 0; return 1; }

void fenorm(fe *a){
    fe p; for(int i=0;i<8;i++) p.d[i]=P[i];
    while(fecmp(a,&p)>=0){
        u64 br=0;
        for(int i=0;i<8;i++){
            u64 t=(u64)a->d[i]-P[i]-br;
            a->d[i]=(u32)t; br=(t>>63)&1;
        }
    }
}

void feadd(fe *r, const fe *a, const fe *b) {
    u64 c = 0;
    for (int i = 0; i < 8; i++) {
        u64 t = (u64)a->d[i] + b->d[i] + c;
        r->d[i] = (u32)t;
        c = t >> 32;
    }
    /* Carry out of 2^256: ≡ c*(2^32+977). Needed for e.g. 3·Gx². */
    while (c) {
        u64 t0 = (u64)r->d[0] + c * 977UL;
        r->d[0] = (u32)t0;
        u64 t1 = (u64)r->d[1] + c + (t0 >> 32);
        r->d[1] = (u32)t1;
        c = t1 >> 32;
        for (int i = 2; i < 8 && c; i++) {
            u64 t = (u64)r->d[i] + c;
            r->d[i] = (u32)t;
            c = t >> 32;
        }
    }
    fenorm(r);
}

void fesub(fe *r,const fe *a,const fe *b){
    if(fecmp(a,b)>=0){
        u64 br=0;
        for(int i=0;i<8;i++){ u64 t=(u64)a->d[i]-b->d[i]-br; r->d[i]=(u32)t; br=(t>>63)&1; }
    } else {
        fe tmp; u64 br=0;
        for(int i=0;i<8;i++){ u64 t=(u64)P[i]-b->d[i]-br; tmp.d[i]=(u32)t; br=(t>>63)&1; }
        feadd(r,a,&tmp);
    }
}

/* schoolbook mul + reduce: 2^256 ≡ 2^32 + 977 (mod p).
 * Carry during the product loop — naive u64 accum overflows (8*(2^32-1)^2). */
void femul(fe *r, const fe *a, const fe *b) {
    u64 t[16];
    for (int i = 0; i < 16; i++) t[i] = 0;
    for (int i = 0; i < 8; i++) {
        u64 carry = 0;
        for (int j = 0; j < 8; j++) {
            u64 sum = t[i + j] + (u64)a->d[i] * (u64)b->d[j] + carry;
            t[i + j] = sum & 0xffffffffUL;
            carry = sum >> 32;
        }
        int k = i + 8;
        while (carry && k < 16) {
            u64 sum = t[k] + carry;
            t[k] = sum & 0xffffffffUL;
            carry = sum >> 32;
            k++;
        }
    }
    /* Fold hi (t[8..15]) via ≡ lo + hi*(2^32+977). */
    for (int pass = 0; pass < 3; pass++) {
        u64 hi[8];
        int any = 0;
        for (int i = 0; i < 8; i++) {
            hi[i] = t[i + 8];
            t[i + 8] = 0;
            if (hi[i]) any = 1;
        }
        if (!any) break;
        u64 c = 0;
        for (int i = 0; i < 8; i++) {
            u64 v = t[i] + hi[i] * 977UL + c;
            t[i] = v & 0xffffffffUL;
            c = v >> 32;
        }
        {
            u64 v = t[8] + c;
            t[8] = v & 0xffffffffUL;
            t[9] += v >> 32;
        }
        c = 0;
        for (int i = 0; i < 8; i++) {
            u64 v = t[i + 1] + hi[i] + c;
            t[i + 1] = v & 0xffffffffUL;
            c = v >> 32;
        }
        t[9] += c;
    }
    for (int i = 0; i < 8; i++) r->d[i] = (u32)t[i];
    fenorm(r);
}

void fesqr(fe *r,const fe *a){ femul(r,a,a); }

void feinv(fe *r,const fe *a){
    fe e; for(int i=0;i<8;i++) e.d[i]=P[i];
    u64 br=2;
    for(int i=0;i<8;i++){ u64 t=(u64)e.d[i]-br; e.d[i]=(u32)t; br=(t>>63)&1; }
    fe base; fecpy(&base,a); fe1(r);
    for(int i=0;i<256;i++){
        if((e.d[i/32]>>(i%32))&1u){ fe t; femul(&t,r,&base); fecpy(r,&t); }
        fe t; fesqr(&t,&base); fecpy(&base,&t);
    }
}

void fe_to_be(const fe *a,u8 b[32]){
    for(int i=0;i<8;i++){
        int o=28-4*i;
        b[o]=(u8)(a->d[i]>>24); b[o+1]=(u8)(a->d[i]>>16);
        b[o+2]=(u8)(a->d[i]>>8); b[o+3]=(u8)a->d[i];
    }
}

int sk_ok(const u8 sk[32]){
    int nz=0; for(int i=0;i<32;i++) if(sk[i]) nz=1;
    if(!nz) return 0;
    for(int i=0;i<32;i++){
        u8 nb=(u8)((N[(31-i)/4] >> (((31-i)%4)*8)) & 0xff);
        if(sk[i]<nb) return 1;
        if(sk[i]>nb) return 0;
    }
    return 0;
}

/* Affine EC (a=0). Jacobian dbl still mismatches random scalars after field
 * fixes; affine matches sk=1/2 and CPU re-verify. Feinv each op is slower. */
void ge_dbl(ge *r, const ge *p) {
    if (p->inf) { *r = *p; return; }
    fe xx, t, num, den, inv, lam, x3, y3;
    fesqr(&xx, &p->x);
    feadd(&t, &xx, &xx); feadd(&num, &t, &xx); /* 3x^2 */
    feadd(&den, &p->y, &p->y);                  /* 2y */
    feinv(&inv, &den);
    femul(&lam, &num, &inv);
    fesqr(&x3, &lam); fesub(&x3, &x3, &p->x); fesub(&x3, &x3, &p->x);
    fesub(&t, &p->x, &x3); femul(&y3, &lam, &t); fesub(&y3, &y3, &p->y);
    fecpy(&r->x, &x3); fecpy(&r->y, &y3); r->inf = 0;
}

void ge_add(ge *r, const ge *p, const ge *q) {
    if (p->inf) { *r = *q; return; }
    if (q->inf) { *r = *p; return; }
    fe dx, dy, inv, lam, x3, y3, t;
    fesub(&dx, &q->x, &p->x);
    fesub(&dy, &q->y, &p->y);
    if (fe_is0(&dx)) {
        if (fe_is0(&dy)) { ge_dbl(r, p); return; }
        r->inf = 1; fe0(&r->x); fe0(&r->y); return;
    }
    feinv(&inv, &dx);
    femul(&lam, &dy, &inv);
    fesqr(&x3, &lam); fesub(&x3, &x3, &p->x); fesub(&x3, &x3, &q->x);
    fesub(&t, &p->x, &x3); femul(&y3, &lam, &t); fesub(&y3, &y3, &p->y);
    fecpy(&r->x, &x3); fecpy(&r->y, &y3); r->inf = 0;
}

void pubkey(ge *out, const u8 sk[32]) {
    ge g;
    for (int i = 0; i < 8; i++) { g.x.d[i] = GX[i]; g.y.d[i] = GY[i]; }
    g.inf = 0;
    ge r; r.inf = 1; fe0(&r.x); fe0(&r.y);
    for (int i = 0; i < 256; i++) {
        ge t; ge_dbl(&t, &r); r = t;
        int bit = (sk[i / 8] >> (7 - (i % 8))) & 1;
        if (bit) { ge_add(&t, &r, &g); r = t; }
    }
    *out = r;
}

/* Wire layout = GpuPattern repr(C): kind@0 plen@4 slen@8 clen@12 ic@16
 * prefix@20 suffix@52 contains@84. Pattern match is inlined in the kernel —
 * Apple OpenCL miscompiles a helper that takes __global const u8 *pat. */

__kernel void evm_grind(
    __global const u8 *seed,
    ulong batch_base,
    __global const u8 *pat,
    __global volatile int *found,
    __global u8 *out_sk
){
    size_t gid=get_global_id(0);
    if(*found >= 0) return;

#ifdef VANITY_DUMP_PAT
    /* Debug: copy first 32 bytes of pattern wire to out_sk from gid 0. */
    if (gid == 0 && atom_cmpxchg(found, -1, 0) == -1) {
        for (int i = 0; i < 32; i++) out_sk[i] = pat[i];
    }
    return;
#endif

#ifdef VANITY_VECTOR_SK
    /* seed[31]: 1/2 → address; 0xFE → dump GX; 0xFF → dump Gx². */
    if (gid == 0 && atom_cmpxchg(found, -1, 0) == -1) {
        u8 tmp[32];
        if (seed[31] == 0xFE) {
            fe gx;
            for (int i = 0; i < 8; i++) gx.d[i] = GX[i];
            fe_to_be(&gx, tmp);
            for (int i = 0; i < 32; i++) out_sk[i] = tmp[i];
        } else if (seed[31] == 0xFF) {
            fe gx, sq;
            for (int i = 0; i < 8; i++) gx.d[i] = GX[i];
            fesqr(&sq, &gx);
            fe_to_be(&sq, tmp);
            for (int i = 0; i < 32; i++) out_sk[i] = tmp[i];
        } else if (seed[31] == 0xFD) {
            /* dump inv(2*Gy) */
            fe gy, den, inv;
            for (int i = 0; i < 8; i++) gy.d[i] = GY[i];
            feadd(&den, &gy, &gy);
            feinv(&inv, &den);
            fe_to_be(&inv, tmp);
            for (int i = 0; i < 32; i++) out_sk[i] = tmp[i];
        } else if (seed[31] == 0xFC) {
            /* dump 2G.x via ge_dbl */
            ge g, g2;
            for (int i = 0; i < 8; i++) { g.x.d[i] = GX[i]; g.y.d[i] = GY[i]; }
            g.inf = 0;
            ge_dbl(&g2, &g);
            fe_to_be(&g2.x, tmp);
            for (int i = 0; i < 32; i++) out_sk[i] = tmp[i];
        } else if (batch_base == 0xC0FFEEULL) {
            /* seed[0..31] is the secret; write address bytes to out_sk */
            u8 sk[32];
            for (int i = 0; i < 32; i++) sk[i] = seed[i];
            ge pub; pubkey(&pub, sk);
            u8 xy[64]; fe_to_be(&pub.x, xy); fe_to_be(&pub.y, xy + 32);
            u8 hash[32]; keccak256(xy, 64, hash);
            for (int i = 0; i < 20; i++) out_sk[i] = hash[12 + i];
            for (int i = 20; i < 32; i++) out_sk[i] = 0;
        } else {
            u8 sk[32];
            for (int i = 0; i < 32; i++) sk[i] = 0;
            sk[31] = seed[31] ? seed[31] : 1;
            ge pub; pubkey(&pub, sk);
            u8 xy[64]; fe_to_be(&pub.x, xy); fe_to_be(&pub.y, xy + 32);
            u8 hash[32]; keccak256(xy, 64, hash);
            for (int i = 0; i < 20; i++) out_sk[i] = hash[12 + i];
            for (int i = 20; i < 32; i++) out_sk[i] = 0;
        }
    }
    return;
#endif

    u8 buf[40];
    for(int i=0;i<32;i++) buf[i]=seed[i];
    ulong ctr=batch_base+(ulong)gid;
    buf[32]=(u8)(ctr>>56); buf[33]=(u8)(ctr>>48); buf[34]=(u8)(ctr>>40); buf[35]=(u8)(ctr>>32);
    buf[36]=(u8)(ctr>>24); buf[37]=(u8)(ctr>>16); buf[38]=(u8)(ctr>>8);  buf[39]=(u8)ctr;

    u8 sk[32];
    keccak256(buf,40,sk);
    if(!sk_ok(sk)) return;

    ge pub; pubkey(&pub,sk);
    if(pub.inf) return;

    u8 xy[64]; fe_to_be(&pub.x,xy); fe_to_be(&pub.y,xy+32);
    u8 hash[32]; keccak256(xy,64,hash);

    /* Inline match — Apple OpenCL has miscompiled match_hex() pointer reads. */
    {
        u8 h[40];
        uint plen = (uint)pat[4] | ((uint)pat[5] << 8) | ((uint)pat[6] << 16) | ((uint)pat[7] << 24);
        uint slen = (uint)pat[8] | ((uint)pat[9] << 8) | ((uint)pat[10] << 16) | ((uint)pat[11] << 24);
        uint clen = (uint)pat[12] | ((uint)pat[13] << 8) | ((uint)pat[14] << 16) | ((uint)pat[15] << 24);
        for (int i = 0; i < 20; i++) {
            u8 v = hash[12 + i];
            int hi = v >> 4, lo = v & 15;
            h[i * 2] = (u8)(hi < 10 ? '0' + hi : 'a' + hi - 10);
            h[i * 2 + 1] = (u8)(lo < 10 ? '0' + lo : 'a' + lo - 10);
        }
        int ok = 1;
        for (uint i = 0; i < plen; i++) if ((h[i] | 32) != (pat[20 + i] | 32)) { ok = 0; break; }
        for (uint i = 0; i < slen && ok; i++) if ((h[40 - slen + i] | 32) != (pat[52 + i] | 32)) { ok = 0; break; }
        if (ok && clen) {
            ok = 0;
            for (uint s = 0; s + clen <= 40; s++) {
                int m = 1;
                for (uint j = 0; j < clen; j++) if ((h[s + j] | 32) != (pat[84 + j] | 32)) { m = 0; break; }
                if (m) { ok = 1; break; }
            }
        }
        if (!ok) return;
    }

    if(atom_cmpxchg(found,-1,(int)gid)==-1){
        for(int i=0;i<32;i++) out_sk[i]=sk[i];
    }
}
