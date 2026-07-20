/* C reference for ctfe_fav.sm — same work at RUNTIME */
#include <stdio.h>
#include <stdint.h>
#ifdef _WIN32
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
static double mono_ms(void) {
    static LARGE_INTEGER freq;
    static int init;
    LARGE_INTEGER c;
    if (!init) { QueryPerformanceFrequency(&freq); init = 1; }
    QueryPerformanceCounter(&c);
    return (double)c.QuadPart * 1000.0 / (double)freq.QuadPart;
}
#else
#include <time.h>
static double mono_ms(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return ts.tv_sec * 1000.0 + ts.tv_nsec / 1e6;
}
#endif

static long long work(long long n) {
    long long s = 0, i;
    for (i = 0; i < n; i++)
        s = s + (i * i) + (i % 17);
    return s;
}

static long long fib(long long n) {
    long long a = 0, b = 1, i, t;
    if (n <= 1) return n;
    for (i = 2; i <= n; i++) {
        t = a + b; a = b; b = t;
    }
    return b;
}

int main(void) {
    const long long N = 2500000;
    double t0, t1;
    long long a, b, c, d, f, g, h, sink;

    t0 = mono_ms();
    a = work(N);
    b = work(N);
    c = work(N);
    d = work(N);
    f = fib(45);
    g = fib(46);
    h = fib(47);
    sink = a + b + c + d + f + g + h;
    t1 = mono_ms();

    printf("%lld\n%lld\n%lld\n", sink, a, f);
    printf("%.3f\n", t1 - t0); /* ms compute only */
    return 0;
}
