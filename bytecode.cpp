#include "bytecode.h"
#include <fstream>
#include <iostream>

// 字节码文件格式：
// 1. 魔数（4字节）："SLBT"
// 2. 版本号（2字节）：0x0100
// 3. 代码段长度（4字节）
// 4. 代码段数据
// 5. 字符串常量池大小（2字节）
// 6. 字符串常量池数据（每个字符串：长度(2字节) + 字符串内容）
// 7. 数字常量池大小（2字节）
// 8. 数字常量池数据（每个数字：8字节，双精度浮点数）
// 9. 常量池大小（2字节）
// 10. 常量池数据（每个常量：长度(2字节) + 常量内容）
// 11. 函数名池大小（2字节）
// 12. 函数名池数据（每个函数名：长度(2字节) + 函数名内容）

// 保存字节码到文件
void saveBytecodeToFile(const Bytecode& bytecode, const std::string& filename) {
    std::ofstream file(filename, std::ios::binary);
    if (!file.is_open()) {
        throw std::runtime_error("Could not open file for writing: " + filename);
    }
    
    // 写入魔数
    const char* magic = "SLBT";
    file.write(magic, 4);
    
    // 写入版本号
    uint16_t version = 0x0100;
    file.write(reinterpret_cast<const char*>(&version), 2);
    
    // 写入代码段
    uint32_t codeSize = static_cast<uint32_t>(bytecode.code.size());
    file.write(reinterpret_cast<const char*>(&codeSize), 4);
    file.write(reinterpret_cast<const char*>(bytecode.code.data()), codeSize);
    
    // 写入字符串常量池
    uint16_t stringCount = static_cast<uint16_t>(bytecode.strings.size());
    file.write(reinterpret_cast<const char*>(&stringCount), 2);
    for (const auto& str : bytecode.strings) {
        uint16_t strSize = static_cast<uint16_t>(str.size());
        file.write(reinterpret_cast<const char*>(&strSize), 2);
        file.write(str.c_str(), strSize);
    }
    
    // 写入数字常量池
    uint16_t numberCount = static_cast<uint16_t>(bytecode.numbers.size());
    file.write(reinterpret_cast<const char*>(&numberCount), 2);
    file.write(reinterpret_cast<const char*>(bytecode.numbers.data()), numberCount * sizeof(double));
    
    // 写入常量池
    uint16_t constantCount = static_cast<uint16_t>(bytecode.constants.size());
    file.write(reinterpret_cast<const char*>(&constantCount), 2);
    for (const auto& constant : bytecode.constants) {
        uint16_t constantSize = static_cast<uint16_t>(constant.size());
        file.write(reinterpret_cast<const char*>(&constantSize), 2);
        file.write(constant.c_str(), constantSize);
    }
    
    // 写入函数名池
    uint16_t functionCount = static_cast<uint16_t>(bytecode.functions.size());
    file.write(reinterpret_cast<const char*>(&functionCount), 2);
    for (const auto& func : bytecode.functions) {
        uint16_t funcSize = static_cast<uint16_t>(func.size());
        file.write(reinterpret_cast<const char*>(&funcSize), 2);
        file.write(func.c_str(), funcSize);
    }
    
    file.close();
    std::cout << "Bytecode saved to " << filename << std::endl;
}

// 从文件加载字节码
Bytecode loadBytecodeFromFile(const std::string& filename) {
    std::ifstream file(filename, std::ios::binary);
    if (!file.is_open()) {
        throw std::runtime_error("Could not open file for reading: " + filename);
    }
    
    Bytecode bytecode;
    
    // 检查魔数
    char magic[5] = {0};
    file.read(magic, 4);
    if (std::string(magic) != "SLBT") {
        throw std::runtime_error("Invalid bytecode file format");
    }
    
    // 检查版本号
    uint16_t version;
    file.read(reinterpret_cast<char*>(&version), 2);
    if (version != 0x0100) {
        throw std::runtime_error("Unsupported bytecode version");
    }
    
    // 读取代码段
    uint32_t codeSize;
    file.read(reinterpret_cast<char*>(&codeSize), 4);
    bytecode.code.resize(codeSize);
    file.read(reinterpret_cast<char*>(bytecode.code.data()), codeSize);
    
    // 读取字符串常量池
    uint16_t stringCount;
    file.read(reinterpret_cast<char*>(&stringCount), 2);
    bytecode.strings.resize(stringCount);
    for (auto& str : bytecode.strings) {
        uint16_t strSize;
        file.read(reinterpret_cast<char*>(&strSize), 2);
        str.resize(strSize);
        file.read(&str[0], strSize);
    }
    
    // 读取数字常量池
    uint16_t numberCount;
    file.read(reinterpret_cast<char*>(&numberCount), 2);
    bytecode.numbers.resize(numberCount);
    file.read(reinterpret_cast<char*>(bytecode.numbers.data()), numberCount * sizeof(double));
    
    // 读取常量池
    uint16_t constantCount;
    file.read(reinterpret_cast<char*>(&constantCount), 2);
    bytecode.constants.resize(constantCount);
    for (auto& constant : bytecode.constants) {
        uint16_t constantSize;
        file.read(reinterpret_cast<char*>(&constantSize), 2);
        constant.resize(constantSize);
        file.read(&constant[0], constantSize);
    }
    
    // 读取函数名池
    uint16_t functionCount;
    file.read(reinterpret_cast<char*>(&functionCount), 2);
    bytecode.functions.resize(functionCount);
    for (auto& func : bytecode.functions) {
        uint16_t funcSize;
        file.read(reinterpret_cast<char*>(&funcSize), 2);
        func.resize(funcSize);
        file.read(&func[0], funcSize);
    }
    
    file.close();
    std::cout << "Bytecode loaded from " << filename << std::endl;
    return bytecode;
}
