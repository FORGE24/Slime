/**
 * Java对比版本 - 标准实现
 * 编译: javac CompareJava.java
 * 运行: java CompareJava
 */

public class CompareJava {
    
    // 测试1: 斐波那契（递归实现）
    static long fibRuntime(int n) {
        if (n <= 1) return n;
        return fibRuntime(n - 1) + fibRuntime(n - 2);
    }
    
    // 测试2: 收敛计算
    static double convergenceDemo() {
        double sum = 0.0;
        double prev = 0.0;
        
        for (int i = 0; i < 1000000; i++) {
            prev = sum;
            sum = sum + 1.0 / ((i + 1) * (i + 1));
        }
        
        return sum;
    }
    
    // 测试3: 双重循环
    static int temporalCollapse(int iterations) {
        int result = 0;
        
        for (int t1 = 0; t1 < iterations; t1++) {
            for (int t2 = 0; t2 < iterations; t2++) {
                if ((t1 * t2) % 7 == 0) {
                    result++;
                }
            }
        }
        
        return result;
    }
    
    // 测试4: 多任务
    static long computeHeavy(int n) {
        long sum = 0;
        for (int i = 0; i < n; i++) {
            sum += i * i;
        }
        return sum;
    }
    
    static long parallelTasks() {
        long task1 = computeHeavy(100);
        long task2 = computeHeavy(200);
        long task3 = computeHeavy(300);
        return task1 + task2 + task3;
    }
    
    // 测试5: 并发求和
    static long sumRange(int start, int end) {
        long sum = 0;
        for (int i = start; i < end; i++) {
            sum += i;
        }
        return sum;
    }
    
    static long concurrentSum(int n) {
        long part1 = sumRange(0, n / 4);
        long part2 = sumRange(n / 4, n / 2);
        long part3 = sumRange(n / 2, n * 3 / 4);
        long part4 = sumRange(n * 3 / 4, n);
        return part1 + part2 + part3 + part4;
    }
    
    // 测试6: 矩阵计算
    static long matrixMultiplySnippet() {
        long result = 0;
        
        for (int i = 0; i < 1000; i++) {
            int temp = i * 3 + 5;
            result += temp * temp;
        }
        
        return result;
    }
    
    // 测试7: 多项式求值
    static double polynomialEval(double x, double[] coeffs) {
        double result = coeffs[0];
        double power = 1.0;
        
        for (int i = 1; i < 5; i++) {
            power = power * x;
            result = result + coeffs[i] * power;
        }
        
        return result;
    }
    
    public static void main(String[] args) {
        System.out.println("=== Java版本 (JIT优化) ===\n");
        
        double totalTime = 0;
        
        // 测试1: 斐波那契
        System.out.println("【1】斐波那契计算 (递归实现):");
        long start = System.nanoTime();
        long fibResult = fibRuntime(30);
        double elapsed = (System.nanoTime() - start) / 1_000_000.0;
        totalTime += elapsed;
        System.out.printf("  fib(30) = %d%n", fibResult);
        System.out.printf("  耗时: %.3f ms%n%n", elapsed);
        
        // 测试2: 收敛计算
        System.out.println("【2】收敛计算 (100万次迭代):");
        start = System.nanoTime();
        double convResult = convergenceDemo();
        elapsed = (System.nanoTime() - start) / 1_000_000.0;
        totalTime += elapsed;
        System.out.printf("  结果 = %.6f%n", convResult);
        System.out.printf("  耗时: %.3f ms%n%n", elapsed);
        
        // 测试3: 双重循环
        System.out.println("【3】双重循环 (100x100):");
        start = System.nanoTime();
        int tceResult = temporalCollapse(100);
        elapsed = (System.nanoTime() - start) / 1_000_000.0;
        totalTime += elapsed;
        System.out.printf("  结果 = %d%n", tceResult);
        System.out.printf("  耗时: %.3f ms%n%n", elapsed);
        
        // 测试4: 多任务
        System.out.println("【4】多任务计算 (顺序执行):");
        start = System.nanoTime();
        long schedResult = parallelTasks();
        elapsed = (System.nanoTime() - start) / 1_000_000.0;
        totalTime += elapsed;
        System.out.printf("  结果 = %d%n", schedResult);
        System.out.printf("  耗时: %.3f ms%n%n", elapsed);
        
        // 测试5: 并发求和
        System.out.println("【5】并发求和 (10000):");
        start = System.nanoTime();
        long prefoldResult = concurrentSum(10000);
        elapsed = (System.nanoTime() - start) / 1_000_000.0;
        totalTime += elapsed;
        System.out.printf("  结果 = %d%n", prefoldResult);
        System.out.printf("  耗时: %.3f ms%n%n", elapsed);
        
        // 测试6: 矩阵计算
        System.out.println("【6】矩阵计算 (1000次):");
        start = System.nanoTime();
        long ifmResult = matrixMultiplySnippet();
        elapsed = (System.nanoTime() - start) / 1_000_000.0;
        totalTime += elapsed;
        System.out.printf("  结果 = %d%n", ifmResult);
        System.out.printf("  耗时: %.3f ms%n%n", elapsed);
        
        // 测试7: 多项式求值
        System.out.println("【7】多项式求值:");
        start = System.nanoTime();
        double[] coeffs = {1, 2, 3, 4, 5};
        double dopeResult = polynomialEval(10.0, coeffs);
        elapsed = (System.nanoTime() - start) / 1_000_000.0;
        totalTime += elapsed;
        System.out.printf("  结果 = %.0f%n", dopeResult);
        System.out.printf("  耗时: %.3f ms%n%n", elapsed);
        
        System.out.printf("=== 总耗时: %.3f ms ===%n", totalTime);
    }
}
