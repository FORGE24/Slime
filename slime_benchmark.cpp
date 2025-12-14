#include <iostream>
#include <fstream>
#include <string>
#include <vector>
#include <chrono>
#include <algorithm>
#include <numeric>
#include <iomanip>
#include <cstdlib>
#include <sstream>
#include <cmath>

// 执行模式枚举
enum ExecutionMode {
    INTERPRET,    // 直接解释执行
    BYTECODE      // 字节码编译后执行
};

class SlimeBenchmark {
public:
    SlimeBenchmark() {}
    
    // 运行基准测试
    void runBenchmark(const std::string& filename, ExecutionMode mode, int iterations = 10) {
        std::cout << "=== Slime Benchmark Tool ===\n";
        std::cout << "File: " << filename << "\n";
        std::cout << "Mode: " << (mode == INTERPRET ? "Direct Interpretation" : "Bytecode Execution") << "\n";
        std::cout << "Iterations: " << iterations << "\n";
        std::cout << "============================\n";
        
        // 检查解释器可执行文件是否存在
        if (!checkInterpreterExists()) {
            std::cerr << "Error: simple_interpreter.exe not found!\n";
            std::cerr << "Please compile the interpreter first using: g++ -o simple_interpreter simple_interpreter.cpp\n";
            return;
        }
        
        // 根据模式选择不同执行路径
        if (mode == INTERPRET) {
            // 直接解释执行路径
            executeInterpreted(filename, iterations);
        } else {
            // 字节码编译执行路径
            executeBytecode(filename, iterations);
        }
    }
    
private:
    
    // 检查解释器可执行文件是否存在
    bool checkInterpreterExists() {
        std::ifstream file("simple_interpreter.exe");
        return file.good();
    }
    
    // 直接解释执行路径
    void executeInterpreted(const std::string& filename, int iterations) {
        // 预热（运行一次以排除初始化影响）
        std::cout << "[Warming up...]\n";
        executeOnceInterpreted(filename);
        
        // 运行基准测试
        std::vector<double> executionTimes;
        executionTimes.reserve(iterations);
        
        std::cout << "[Running benchmark...]\n";
        for (int i = 0; i < iterations; i++) {
            double time = executeOnceInterpreted(filename);
            executionTimes.push_back(time);
            std::cout << "Iteration " << (i + 1) << ": " << time << " ms\n";
        }
        
        // 计算统计结果
        calculateStatistics(executionTimes);
    }
    
    // 字节码执行路径
    void executeBytecode(const std::string& filename, int iterations) {
        // 先编译为字节码
        std::string btcFilename = "temp_benchmark.btc";
        compileToBytecode(filename, btcFilename);
        
        // 预热
        std::cout << "[Warming up...]\n";
        executeOnceBytecode(btcFilename);
        
        // 运行基准测试
        std::vector<double> executionTimes;
        executionTimes.reserve(iterations);
        
        std::cout << "[Running benchmark...]\n";
        for (int i = 0; i < iterations; i++) {
            double time = executeOnceBytecode(btcFilename);
            executionTimes.push_back(time);
            std::cout << "Iteration " << (i + 1) << ": " << time << " ms\n";
        }
        
        // 清理临时文件
        std::remove(btcFilename.c_str());
        
        // 计算统计结果
        calculateStatistics(executionTimes);
    }
    
    // 执行一次代码并返回执行时间（毫秒）- 直接解释模式
    double executeOnceInterpreted(const std::string& filename) {
        auto start = std::chrono::high_resolution_clock::now();
        
        // 构建命令行命令
        std::ostringstream cmd;
        cmd << "simple_interpreter.exe " << filename;
        
        // 执行命令
        int result = std::system(cmd.str().c_str());
        
        auto end = std::chrono::high_resolution_clock::now();
        auto duration = std::chrono::duration_cast<std::chrono::milliseconds>(end - start).count();
        
        if (result != 0) {
            std::cerr << "Error executing code (return code: " << result << ")\n";
            return 0;
        }
        
        return static_cast<double>(duration);
    }
    
    // 执行一次代码并返回执行时间（毫秒）- 字节码模式
    double executeOnceBytecode(const std::string& btcFilename) {
        auto start = std::chrono::high_resolution_clock::now();
        
        // 构建命令行命令
        std::ostringstream cmd;
        cmd << "simple_interpreter.exe --run " << btcFilename;
        
        // 执行命令
        int result = std::system(cmd.str().c_str());
        
        auto end = std::chrono::high_resolution_clock::now();
        auto duration = std::chrono::duration_cast<std::chrono::milliseconds>(end - start).count();
        
        if (result != 0) {
            std::cerr << "Error executing bytecode (return code: " << result << ")\n";
            return 0;
        }
        
        return static_cast<double>(duration);
    }
    
    // 编译源代码为字节码
    void compileToBytecode(const std::string& filename, const std::string& btcFilename) {
        std::ostringstream cmd;
        cmd << "simple_interpreter.exe --compile " << filename << " " << btcFilename;
        
        int result = std::system(cmd.str().c_str());
        
        if (result != 0) {
            throw std::runtime_error("Failed to compile to bytecode");
        }
    }
    
    // 计算并输出统计结果
    void calculateStatistics(const std::vector<double>& times) {
        if (times.empty()) {
            std::cout << "No valid execution times to analyze.\n";
            return;
        }
        
        // 过滤掉0值（错误情况）
        std::vector<double> validTimes;
        for (double time : times) {
            if (time > 0) {
                validTimes.push_back(time);
            }
        }
        
        if (validTimes.empty()) {
            std::cout << "No valid execution times to analyze after filtering.\n";
            return;
        }
        
        double minTime = *std::min_element(validTimes.begin(), validTimes.end());
        double maxTime = *std::max_element(validTimes.begin(), validTimes.end());
        double totalTime = std::accumulate(validTimes.begin(), validTimes.end(), 0.0);
        double avgTime = totalTime / validTimes.size();
        
        // 计算标准差
        double sumSquares = std::inner_product(validTimes.begin(), validTimes.end(), validTimes.begin(), 0.0);
        double stdDev = std::sqrt(sumSquares / validTimes.size() - avgTime * avgTime);
        
        std::cout << "============================\n";
        std::cout << "Statistics (excluding errors):\n";
        std::cout << "Valid iterations: " << validTimes.size() << " out of " << times.size() << "\n";
        std::cout << "Min time: " << std::fixed << std::setprecision(3) << minTime << " ms\n";
        std::cout << "Max time: " << std::fixed << std::setprecision(3) << maxTime << " ms\n";
        std::cout << "Avg time: " << std::fixed << std::setprecision(3) << avgTime << " ms\n";
        std::cout << "Std dev: " << std::fixed << std::setprecision(3) << stdDev << " ms\n";
        std::cout << "Total time: " << std::fixed << std::setprecision(3) << totalTime << " ms\n";
        std::cout << "============================\n";
    }
};

int main(int argc, char* argv[]) {
    if (argc < 3 || argc > 4) {
        std::cerr << "Usage: " << argv[0] << " <filename> <mode> [iterations]\n";
        std::cerr << "  filename: Path to the Slime code file\n";
        std::cerr << "  mode: Execution mode (interpret or bytecode)\n";
        std::cerr << "  iterations: Number of iterations to run (default: 10)\n";
        return 1;
    }
    
    std::string filename = argv[1];
    std::string modeStr = argv[2];
    int iterations = (argc == 4) ? std::stoi(argv[3]) : 10;
    
    if (iterations <= 0) {
        std::cerr << "Error: Iterations must be positive\n";
        return 1;
    }
    
    // 解析执行模式
    ExecutionMode mode;
    if (modeStr == "interpret") {
        mode = INTERPRET;
    } else if (modeStr == "bytecode") {
        mode = BYTECODE;
    } else {
        std::cerr << "Error: Invalid mode. Use 'interpret' or 'bytecode'\n";
        return 1;
    }
    
    try {
        SlimeBenchmark benchmark;
        benchmark.runBenchmark(filename, mode, iterations);
    } catch (const std::exception& e) {
        std::cerr << "Error: " << e.what() << std::endl;
        return 1;
    }
    
    return 0;
}