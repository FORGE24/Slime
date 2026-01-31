// C 性能测试
#include <stdio.h>

int main() {
    long long sum = 0;
    for (int i = 0; i < 1000000; i++) {
        sum += i;
    }
    printf("%lld\n", sum);
    return 0;
}
