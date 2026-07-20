/* combo_ref.c — 五算子连招参考实现（公平金标准）
 * 编译：clang -O0 -fno-vectorize -fno-unroll-loops combo_ref.c -o combo_ref.exe -lm
 * 或：  cl /Od combo_ref.c
 * 输出：仅一行总秒数，保留 6 位小数
 */
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>

#ifdef _WIN32
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
static double mono_now(void) {
    static LARGE_INTEGER freq;
    static int init;
    LARGE_INTEGER c;
    if (!init) {
        QueryPerformanceFrequency(&freq);
        init = 1;
    }
    QueryPerformanceCounter(&c);
    return (double)c.QuadPart / (double)freq.QuadPart;
}
#else
#include <time.h>
static double mono_now(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (double)ts.tv_sec + (double)ts.tv_nsec * 1e-9;
}
#endif

/* ---- §1/§2 全局与派生（与 COMBO_SPEC.md 一致）---- */
enum {
    SCALE = 90,
    SEED = 666,
    MAT_N = SCALE,
    MAT_ROUNDS = SCALE * 55,
    MATH_ITERS = SCALE * SCALE * SCALE * 280,
    SORT_LEN = SCALE * SCALE * 280,
    HASH_OPS = SCALE * SCALE * 140,
    STR_ROUNDS = SCALE * SCALE * 14
};

/* ---- §3 统一 LCG ---- */
static uint64_t g_rng = (uint64_t)SEED;

static uint64_t rng_u64(void) {
    g_rng = g_rng * 6364136223846793005ULL + 1ULL;
    return g_rng;
}

static double rng_f64(void) {
    return (double)(rng_u64() >> 11) * (1.0 / 9007199254740992.0);
}

/* ---- MD5 (RFC 1321, 自包含) ---- */
typedef struct {
    uint32_t a, b, c, d;
    uint64_t nbits;
    unsigned char buf[64];
    size_t bi;
} MD5;

static uint32_t md5_rol(uint32_t x, int n) { return (x << n) | (x >> (32 - n)); }

static void md5_block(MD5 *m, const unsigned char *p) {
    static const uint32_t K[64] = {
        0xd76aa478,0xe8c7b756,0x242070db,0xc1bdceee,0xf57c0faf,0x4787c62a,0xa8304613,0xfd469501,
        0x698098d8,0x8b44f7af,0xffff5bb1,0x895cd7be,0x6b901122,0xfd987193,0xa679438e,0x49b40821,
        0xf61e2562,0xc040b340,0x265e5a51,0xe9b6c7aa,0xd62f105d,0x02441453,0xd8a1e681,0xe7d3fbc8,
        0x21e1cde6,0xc33707d6,0xf4d50d87,0x455a14ed,0xa9e3e905,0xfcefa3f8,0x676f02d9,0x8d2a4c8a,
        0xfffa3942,0x8771f681,0x6d9d6122,0xfde5380c,0xa4beea44,0x4bdecfa9,0xf6bb4b60,0xbebfbc70,
        0x289b7ec6,0xeaa127fa,0xd4ef3085,0x04881d05,0xd9d4d039,0xe6db99e5,0x1fa27cf8,0xc4ac5665,
        0xf4292244,0x432aff97,0xab9423a7,0xfc93a039,0x655b59c3,0x8f0ccc92,0xffeff47d,0x85845dd1,
        0x6fa87e4f,0xfe2ce6e0,0xa3014314,0x4e0811a1,0xf7537e82,0xbd3af235,0x2ad7d2bb,0xeb86d391
    };
    static const int S[64] = {
        7,12,17,22,7,12,17,22,7,12,17,22,7,12,17,22,
        5,9,14,20,5,9,14,20,5,9,14,20,5,9,14,20,
        4,11,16,23,4,11,16,23,4,11,16,23,4,11,16,23,
        6,10,15,21,6,10,15,21,6,10,15,21,6,10,15,21
    };
    uint32_t X[16], A = m->a, B = m->b, C = m->c, D = m->d;
    int i;
    for (i = 0; i < 16; i++) {
        X[i] = (uint32_t)p[i*4] | ((uint32_t)p[i*4+1] << 8) |
               ((uint32_t)p[i*4+2] << 16) | ((uint32_t)p[i*4+3] << 24);
    }
    for (i = 0; i < 64; i++) {
        uint32_t F, g;
        if (i < 16) { F = (B & C) | (~B & D); g = (uint32_t)i; }
        else if (i < 32) { F = (D & B) | (~D & C); g = (uint32_t)(5*i + 1) % 16; }
        else if (i < 48) { F = B ^ C ^ D; g = (uint32_t)(3*i + 5) % 16; }
        else { F = C ^ (B | ~D); g = (uint32_t)(7*i) % 16; }
        F = F + A + K[i] + X[g];
        A = D; D = C; C = B; B = B + md5_rol(F, S[i]);
    }
    m->a += A; m->b += B; m->c += C; m->d += D;
}

static void md5_init(MD5 *m) {
    m->a = 0x67452301; m->b = 0xefcdab89; m->c = 0x98badcfe; m->d = 0x10325476;
    m->nbits = 0; m->bi = 0;
}

static void md5_update(MD5 *m, const unsigned char *data, size_t len) {
    size_t i = 0;
    m->nbits += (uint64_t)len * 8;
    while (i < len) {
        m->buf[m->bi++] = data[i++];
        if (m->bi == 64) { md5_block(m, m->buf); m->bi = 0; }
    }
}

static void md5_final(MD5 *m, unsigned char out[16]) {
    size_t i;
    m->buf[m->bi++] = 0x80;
    if (m->bi > 56) {
        while (m->bi < 64) m->buf[m->bi++] = 0;
        md5_block(m, m->buf);
        m->bi = 0;
    }
    while (m->bi < 56) m->buf[m->bi++] = 0;
    for (i = 0; i < 8; i++) m->buf[56 + i] = (unsigned char)((m->nbits >> (8 * i)) & 0xff);
    md5_block(m, m->buf);
    for (i = 0; i < 4; i++) {
        out[i] = (unsigned char)((m->a >> (8 * i)) & 0xff);
        out[4 + i] = (unsigned char)((m->b >> (8 * i)) & 0xff);
        out[8 + i] = (unsigned char)((m->c >> (8 * i)) & 0xff);
        out[12 + i] = (unsigned char)((m->d >> (8 * i)) & 0xff);
    }
}

/* ---- 手写堆排序 ---- */
static void sift_down(double *a, int start, int end) {
    int root = start;
    while (root * 2 + 1 <= end) {
        int child = root * 2 + 1;
        int swap = root;
        if (a[swap] < a[child]) swap = child;
        if (child + 1 <= end && a[swap] < a[child + 1]) swap = child + 1;
        if (swap == root) return;
        { double t = a[root]; a[root] = a[swap]; a[swap] = t; }
        root = swap;
    }
}

static void heapsort(double *a, int n) {
    int i;
    for (i = (n - 2) / 2; i >= 0; i--) sift_down(a, i, n - 1);
    for (i = n - 1; i > 0; i--) {
        double t = a[0]; a[0] = a[i]; a[i] = t;
        sift_down(a, 0, i - 1);
    }
}

/* ---- 简易开放寻址哈希（字符串键 → f64），禁止预留容量 ---- */
typedef struct {
    char *key;
    double val;
    int used;
} HEnt;

typedef struct {
    HEnt *tab;
    size_t cap;
    size_t len;
} HMap;

static size_t hash_str(const char *s) {
    size_t h = 5381;
    int c;
    while ((c = (unsigned char)*s++)) h = ((h << 5) + h) + (size_t)c;
    return h;
}

static void hmap_grow(HMap *m) {
    size_t ncap = m->cap ? m->cap * 2 : 8;
    HEnt *nt = (HEnt *)calloc(ncap, sizeof(HEnt));
    size_t i;
    if (!nt) abort();
    for (i = 0; i < m->cap; i++) {
        if (m->tab[i].used) {
            size_t j = hash_str(m->tab[i].key) % ncap;
            while (nt[j].used) j = (j + 1) % ncap;
            nt[j] = m->tab[i];
        }
    }
    free(m->tab);
    m->tab = nt;
    m->cap = ncap;
}

static void hmap_put(HMap *m, const char *key, double val) {
    size_t j;
    if (m->len * 2 >= m->cap) hmap_grow(m);
    j = hash_str(key) % m->cap;
    while (m->tab[j].used) {
        if (strcmp(m->tab[j].key, key) == 0) {
            m->tab[j].val = val;
            return;
        }
        j = (j + 1) % m->cap;
    }
    m->tab[j].key = (char *)malloc(strlen(key) + 1);
    if (!m->tab[j].key) abort();
    strcpy(m->tab[j].key, key);
    m->tab[j].val = val;
    m->tab[j].used = 1;
    m->len++;
}

static void hmap_del_prefix1(HMap *m) {
    size_t i;
    for (i = 0; i < m->cap; i++) {
        if (m->tab[i].used && m->tab[i].key[0] == '1') {
            free(m->tab[i].key);
            m->tab[i].key = NULL;
            m->tab[i].used = 0;
            m->len--;
        }
    }
}

static double hmap_sum(HMap *m) {
    size_t i;
    double s = 0.0;
    for (i = 0; i < m->cap; i++)
        if (m->tab[i].used) s += m->tab[i].val;
    return s;
}

static void hmap_free(HMap *m) {
    size_t i;
    for (i = 0; i < m->cap; i++)
        if (m->tab[i].used) free(m->tab[i].key);
    free(m->tab);
}

/* ---- 任务 ---- */
static double task_matrix(void) {
    const int n = MAT_N;
    double *A = (double *)malloc((size_t)n * n * sizeof(double));
    double *B = (double *)malloc((size_t)n * n * sizeof(double));
    double *C = (double *)malloc((size_t)n * n * sizeof(double));
    int i, j, k, r;
    double sum = 0.0;
    if (!A || !B || !C) abort();
    for (i = 0; i < n * n; i++) {
        A[i] = rng_f64();
        B[i] = rng_f64();
        C[i] = 0.0;
    }
    for (r = 0; r < MAT_ROUNDS; r++) {
        for (i = 0; i < n; i++) {
            for (j = 0; j < n; j++) {
                double acc = C[i * n + j];
                for (k = 0; k < n; k++)
                    acc += A[i * n + k] * B[k * n + j];
                C[i * n + j] = acc;
            }
        }
    }
    for (i = 0; i < n * n; i++) sum += C[i];
    free(A); free(B); free(C);
    return sum;
}

static double task_math(void) {
    double fact[12];
    double acc = 0.0;
    int i;
    long long t;
    fact[0] = 1.0;
    for (i = 1; i < 12; i++) fact[i] = fact[i - 1] * (double)i;
    for (t = 0; t < (long long)MATH_ITERS; t++) {
        double x = rng_f64();
        acc += sin(x) + cos(x) + fact[(int)(t % 12)];
    }
    return acc;
}

static double task_sort(void) {
    int n = SORT_LEN;
    double *a = (double *)malloc((size_t)n * sizeof(double));
    int i;
    double guard;
    if (!a) abort();
    for (i = 0; i < n; i++) a[i] = rng_f64();
    heapsort(a, n);
    guard = a[0] + a[n - 1];
    free(a);
    return guard;
}

static double task_hash(void) {
    HMap m = {0};
    int i;
    char buf[32];
    double s;
    for (i = 0; i < HASH_OPS; i++) {
        sprintf(buf, "%d", i);
        hmap_put(&m, buf, rng_f64());
    }
    hmap_del_prefix1(&m);
    s = hmap_sum(&m);
    hmap_free(&m);
    return s;
}

static double task_string_md5(void) {
    char *s = (char *)malloc(1);
    size_t len = 0;
    int r;
    unsigned char dig[16];
    MD5 ctx;
    double sink = 0.0;
    if (!s) abort();
    s[0] = 0;
    for (r = 0; r < STR_ROUNDS; r++) {
        char piece[48];
        size_t plen;
        char *ns;
        sprintf(piece, "#%d;", r);
        plen = strlen(piece);
        ns = (char *)malloc(len + plen + 1);
        if (!ns) abort();
        memcpy(ns, s, len);
        memcpy(ns + len, piece, plen + 1);
        free(s);
        s = ns;
        len += plen;
    }
    md5_init(&ctx);
    md5_update(&ctx, (unsigned char *)s, len);
    md5_final(&ctx, dig);
    free(s);
    for (r = 0; r < 16; r++) sink += (double)dig[r];
    return sink;
}

int main(void) {
    double t0, t1;
    volatile double sink = 0.0;

    t0 = mono_now();
    sink += task_matrix();
    sink += task_math();
    sink += task_sort();
    sink += task_hash();
    sink += task_string_md5();
    t1 = mono_now();

    if (sink == 1.2345e308) putchar('?'); /* 永不触发，防整段优化掉 */
    printf("%.6f\n", t1 - t0);
    fflush(stdout);
    return 0;
}
