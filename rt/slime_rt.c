/* slime_rt.c — runtime for slime2 combo benchmark codegen
 * Link: clang combo.ll slime_rt.c -o combo -lm
 */
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>

#ifdef _WIN32
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
double slime_mono_now(void) {
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
double slime_mono_now(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (double)ts.tv_sec + (double)ts.tv_nsec * 1e-9;
}
#endif

/* ---- MD5 (RFC 1321) ---- */
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

char *slime_md5_hex(const char *str) {
    MD5 ctx;
    unsigned char dig[16];
    char *out;
    size_t len, i;
    if (!str) str = "";
    len = strlen(str);
    md5_init(&ctx);
    md5_update(&ctx, (const unsigned char *)str, len);
    md5_final(&ctx, dig);
    out = (char *)malloc(33);
    if (!out) abort();
    for (i = 0; i < 16; i++)
        sprintf(out + i * 2, "%02x", dig[i]);
    out[32] = 0;
    return out;
}

/* ---- open-addressing hash (string key -> f64) ---- */
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
    m->tab = NULL;
    m->cap = 0;
    m->len = 0;
}

void *slime_dict_new(void) {
    HMap *m = (HMap *)calloc(1, sizeof(HMap));
    if (!m) abort();
    return m;
}

void slime_dict_put(void *d, const char *key, double val) {
    hmap_put((HMap *)d, key ? key : "", val);
}

void slime_dict_del_prefix1(void *d) {
    hmap_del_prefix1((HMap *)d);
}

double slime_dict_sum(void *d) {
    return hmap_sum((HMap *)d);
}

void slime_dict_free(void *d) {
    if (d) {
        hmap_free((HMap *)d);
        free(d);
    }
}

char *slime_itoa(long long i) {
    char buf[32];
    char *out;
    sprintf(buf, "%lld", i);
    out = (char *)malloc(strlen(buf) + 1);
    if (!out) abort();
    strcpy(out, buf);
    return out;
}

void slime_print_f6(double x) {
    printf("%.6f\n", x);
    fflush(stdout);
}

double *slime_alloc_f64(long long n) {
    double *p = (double *)calloc((size_t)n, sizeof(double));
    if (!p) abort();
    return p;
}

long long *slime_alloc_i64(long long n) {
    long long *p = (long long *)calloc((size_t)n, sizeof(long long));
    if (!p) abort();
    return p;
}

void slime_str_free(char *s) {
    free(s);
}

char *slime_strdup(const char *s) {
    size_t n;
    char *out;
    if (!s) s = "";
    n = strlen(s) + 1;
    out = (char *)malloc(n);
    if (!out) abort();
    memcpy(out, s, n);
    return out;
}

char *slime_strcat(const char *a, const char *b) {
    size_t la, lb;
    char *out;
    if (!a) a = "";
    if (!b) b = "";
    la = strlen(a);
    lb = strlen(b);
    out = (char *)malloc(la + lb + 1);
    if (!out) abort();
    memcpy(out, a, la);
    memcpy(out + la, b, lb + 1);
    return out;
}

char *slime_substr(const char *s, long long start, long long end) {
    size_t len, n;
    char *out;
    if (!s) s = "";
    len = strlen(s);
    if (start < 0) start = 0;
    if (end < start) end = start;
    if ((size_t)end > len) end = (long long)len;
    if ((size_t)start > len) start = (long long)len;
    n = (size_t)(end - start);
    out = (char *)malloc(n + 1);
    if (!out) abort();
    memcpy(out, s + (size_t)start, n);
    out[n] = 0;
    return out;
}

