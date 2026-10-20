/* MIT — independent OpenCL EVM vanity (secp256k1 + Keccak-256).
 *
 * Incremental search: the host gives every work-item a random start point
 * B = k0·G (computed once, kept in device memory between launches). The kernel
 * then walks B + j·G for j = 1..GROUP with ONE shared field inversion per
 * group (Montgomery batch inversion), so a candidate costs ~6 field mults plus
 * one Keccak instead of a full 256-bit scalar multiplication.
 *
 * Candidates are always re-derived on CPU before export.
 * OpenCL 1.2 + cl_khr_global_int32_base_atomics.
 * Host prepends:  #define GROUP <points per inversion>  /  #define GROUPS <groups per launch>
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


inline u32 bswap32(u32 v) {
    return (v >> 24) | ((v >> 8) & 0xff00u) | ((v << 8) & 0xff0000u) | (v << 24);
}

/* Keccak-256 of the 64-byte X‖Y public key, built straight from field limbs. */
void keccak_xy(const fe *x, const fe *y, u8 hash[32]) {
    u64 st[25];
    for (int k = 0; k < 4; k++) {
        st[k]     = (u64)bswap32(x->d[7 - 2*k]) | ((u64)bswap32(x->d[6 - 2*k]) << 32);
        st[4 + k] = (u64)bswap32(y->d[7 - 2*k]) | ((u64)bswap32(y->d[6 - 2*k]) << 32);
    }
    st[8] = 0x01UL;
    for (int i = 9; i < 16; i++) st[i] = 0;
    st[16] = 0x8000000000000000UL;
    for (int i = 17; i < 25; i++) st[i] = 0;
    keccak_f(st);
    for (int i = 0; i < 4; i++)
        for (int b = 0; b < 8; b++) hash[i*8 + b] = (u8)(st[i] >> (8*b));
}

/* Wire layout = GpuPattern repr(C): kind@0 plen@4 slen@8 clen@12 ic@16
 * prefix@20 suffix@52 contains@84. Pattern match is inlined in the kernel —
 * Apple OpenCL miscompiles a helper that takes __global const u8 *pat. */

__kernel void evm_grind(
    __global const u8 *pat,
    __global volatile int *found,
    __global uint *out_off,
    __global uint *bx,
    __global uint *by,
    __global const uint *tabx,
    __global const uint *taby
){
    size_t gid = get_global_id(0);
    if (*found >= 0) return;

    fe Bx, By;
    for (int i = 0; i < 8; i++) { Bx.d[i] = bx[gid*8 + i]; By.d[i] = by[gid*8 + i]; }

    uint plen = (uint)pat[4] | ((uint)pat[5] << 8) | ((uint)pat[6] << 16) | ((uint)pat[7] << 24);
    uint slen = (uint)pat[8] | ((uint)pat[9] << 8) | ((uint)pat[10] << 16) | ((uint)pat[11] << 24);
    uint clen = (uint)pat[12] | ((uint)pat[13] << 8) | ((uint)pat[14] << 16) | ((uint)pat[15] << 24);

    fe pre[GROUP];

    for (int g = 0; g < GROUPS; g++) {
        if (*found >= 0) return;

        /* prefix products of dx_j = T_j.x - B.x */
        fe acc;
        for (int j = 0; j < GROUP; j++) {
            fe tx, dx;
            for (int i = 0; i < 8; i++) tx.d[i] = tabx[j*8 + i];
            fesub(&dx, &tx, &Bx);
            if (fe_is0(&dx)) fe1(&dx);   /* degenerate: keep the batch product usable */
            if (j == 0) acc = dx; else femul(&acc, &acc, &dx);
            pre[j] = acc;
        }
        fe inv;
        feinv(&inv, &acc);

        fe nx, ny;
        fe0(&nx); fe0(&ny);
        for (int j = GROUP - 1; j >= 0; j--) {
            fe tx, ty, dx, dy, invdx, lam, x3, y3, t;
            for (int i = 0; i < 8; i++) { tx.d[i] = tabx[j*8 + i]; ty.d[i] = taby[j*8 + i]; }
            fesub(&dx, &tx, &Bx);
            if (fe_is0(&dx)) {
                /* B == ±j·G: affine add is undefined. Do not keep walking with a
                 * broken Montgomery inv chain — flag the host and abort this WI. */
                out_off[1] = 1u;
                return;
            }
            if (j > 0) femul(&invdx, &inv, &pre[j - 1]); else invdx = inv;
            femul(&inv, &inv, &dx);

            fesub(&dy, &ty, &By);
            femul(&lam, &dy, &invdx);
            fesqr(&x3, &lam); fesub(&x3, &x3, &Bx); fesub(&x3, &x3, &tx);
            fesub(&t, &Bx, &x3); femul(&y3, &lam, &t); fesub(&y3, &y3, &By);

            if (j == GROUP - 1) { nx = x3; ny = y3; }

            u8 hash[32];
            keccak_xy(&x3, &y3, hash);

            /* Inline match — Apple OpenCL has miscompiled match_hex() pointer reads. */
            u8 h[40];
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
                    for (uint k = 0; k < clen; k++) if ((h[s + k] | 32) != (pat[84 + k] | 32)) { m = 0; break; }
                    if (m) { ok = 1; break; }
                }
            }
            if (ok) {
                if (atom_cmpxchg(found, -1, (int)gid) == -1) {
                    /* scalar of this point = k0 + launch_start + g*GROUP + (j+1) */
                    out_off[0] = (uint)(g * GROUP + j + 1);
                }
            }
        }
        Bx = nx; By = ny;
    }

    for (int i = 0; i < 8; i++) { bx[gid*8 + i] = Bx.d[i]; by[gid*8 + i] = By.d[i]; }
}
