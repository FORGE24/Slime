# Cython 性能测试 (编译为 C)
# cython: language_level=3
def main():
    cdef long long sum = 0
    cdef int i
    for i in range(1000000):
        sum += i
    print(sum)

if __name__ == "__main__":
    main()
