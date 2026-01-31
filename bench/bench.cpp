// C++ 性能测试
#include <iostream>

int main() {
    long long sum = 0;
    for (int i = 0; i < 1000000; i++) {
        sum += i;
    }
    std::cout << sum << std::endl;
    return 0;
}
