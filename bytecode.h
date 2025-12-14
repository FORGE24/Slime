#pragma once

#include <vector>
#include <string>
#include <cstdint>
#include <stdexcept>

// 字节码操作码定义
enum OpCode {
    OP_NOP = 0x00,          // 空操作
    OP_PUSH_NUM = 0x01,     // 压入数字常量
    OP_PUSH_STR = 0x02,     // 压入字符串常量
    OP_PUSH_CONST = 0x03,   // 压入常量池中的常量
    OP_POP = 0x04,          // 弹出栈顶元素
    OP_ADD = 0x05,          // 加法
    OP_SUB = 0x06,          // 减法
    OP_MUL = 0x07,          // 乘法
    OP_DIV = 0x08,          // 除法
    OP_MOD = 0x09,          // 取余
    OP_CALL = 0x0A,         // 调用函数
    OP_JMP = 0x0B,          // 无条件跳转
    OP_JMP_IF_FALSE = 0x0C, // 条件跳转（结果为假时跳转）
    OP_JMP_IF_TRUE = 0x0D,  // 条件跳转（结果为真时跳转）
    OP_LOAD = 0x0E,         // 加载变量
    OP_STORE = 0x0F,        // 存储变量
    OP_RET = 0x10,          // 返回
    OP_HALT = 0x11,         // 程序结束
    OP_COMPARE_EQ = 0x12,   // 比较相等
    OP_COMPARE_NE = 0x13,   // 比较不等
    OP_COMPARE_LT = 0x14,   // 比较小于
    OP_COMPARE_LE = 0x15,   // 比较小于等于
    OP_COMPARE_GT = 0x16,   // 比较大于
    OP_COMPARE_GE = 0x17,   // 比较大于等于
    OP_NOT = 0x18,          // 逻辑非
    OP_AND = 0x19,          // 逻辑与
    OP_OR = 0x1A,           // 逻辑或
    OP_LOOP = 0x1B,         // 循环开始
    OP_END_LOOP = 0x1C,     // 循环结束
    OP_IF = 0x1D,           // 条件语句开始
    OP_ELSE = 0x1E,         // 否则分支
    OP_END_IF = 0x1F,       // 条件语句结束
    OP_BREAK = 0x20,        // 跳出循环
    OP_CONTINUE = 0x21      // 继续下一次循环
};

// 字节码结构体
struct Bytecode {
    std::vector<uint8_t> code;       // 字节码指令序列
    std::vector<std::string> strings; // 字符串常量池
    std::vector<double> numbers;     // 数字常量池
    std::vector<std::string> constants; // 常量池
    std::vector<std::string> functions; // 函数名池
};

// 字节码写入器类
class BytecodeWriter {
public:
    BytecodeWriter(Bytecode& bytecode) : bytecode(bytecode) {}
    
    // 写入操作码
    void writeOpCode(OpCode op) {
        bytecode.code.push_back(static_cast<uint8_t>(op));
    }
    
    // 写入8位整数
    void writeByte(uint8_t value) {
        bytecode.code.push_back(value);
    }
    
    // 写入16位整数
    void writeShort(uint16_t value) {
        bytecode.code.push_back((value >> 8) & 0xFF);
        bytecode.code.push_back(value & 0xFF);
    }
    
    // 写入32位整数
    void writeInt(uint32_t value) {
        bytecode.code.push_back((value >> 24) & 0xFF);
        bytecode.code.push_back((value >> 16) & 0xFF);
        bytecode.code.push_back((value >> 8) & 0xFF);
        bytecode.code.push_back(value & 0xFF);
    }
    
    // 写入64位浮点数
    void writeDouble(double value) {
        union { double d; uint8_t bytes[8]; } u;
        u.d = value;
        for (int i = 7; i >= 0; i--) {
            bytecode.code.push_back(u.bytes[i]);
        }
    }
    
    // 写入字符串常量（返回索引）
    uint16_t writeString(const std::string& str) {
        bytecode.strings.push_back(str);
        return static_cast<uint16_t>(bytecode.strings.size() - 1);
    }
    
    // 写入数字常量（返回索引）
    uint16_t writeNumber(double num) {
        bytecode.numbers.push_back(num);
        return static_cast<uint16_t>(bytecode.numbers.size() - 1);
    }
    
    // 写入常量（返回索引）
    uint16_t writeConstant(const std::string& name) {
        bytecode.constants.push_back(name);
        return static_cast<uint16_t>(bytecode.constants.size() - 1);
    }
    
    // 写入函数名（返回索引）
    uint16_t writeFunction(const std::string& name) {
        bytecode.functions.push_back(name);
        return static_cast<uint16_t>(bytecode.functions.size() - 1);
    }
    
    // 获取当前位置
    size_t getPosition() const {
        return bytecode.code.size();
    }
    
    // 写入位置的占位符
    void writePositionPlaceholder() {
        writeInt(0); // 先写入4个字节的0作为占位符
    }
    
    // 更新位置占位符
    void updatePositionPlaceholder(size_t pos, size_t targetPos) {
        uint8_t* bytes = &bytecode.code[pos];
        bytes[0] = (targetPos >> 24) & 0xFF;
        bytes[1] = (targetPos >> 16) & 0xFF;
        bytes[2] = (targetPos >> 8) & 0xFF;
        bytes[3] = targetPos & 0xFF;
    }
    
private:
    Bytecode& bytecode;
};

// 字节码读取器类
class BytecodeReader {
public:
    BytecodeReader(const Bytecode& bytecode) : bytecode(bytecode), position(0) {}
    
    // 读取操作码
    OpCode readOpCode() {
        return static_cast<OpCode>(readByte());
    }
    
    // 读取8位整数
    uint8_t readByte() {
        if (position >= bytecode.code.size()) {
            throw std::runtime_error("BytecodeReader: end of code");
        }
        return bytecode.code[position++];
    }
    
    // 读取16位整数
    uint16_t readShort() {
        uint16_t value = 0;
        value |= static_cast<uint16_t>(readByte()) << 8;
        value |= static_cast<uint16_t>(readByte());
        return value;
    }
    
    // 读取32位整数
    uint32_t readInt() {
        uint32_t value = 0;
        value |= static_cast<uint32_t>(readByte()) << 24;
        value |= static_cast<uint32_t>(readByte()) << 16;
        value |= static_cast<uint32_t>(readByte()) << 8;
        value |= static_cast<uint32_t>(readByte());
        return value;
    }
    
    // 读取64位浮点数
    double readDouble() {
        union { double d; uint8_t bytes[8]; } u;
        for (int i = 7; i >= 0; i--) {
            u.bytes[i] = readByte();
        }
        return u.d;
    }
    
    // 读取字符串常量
    std::string readString() {
        uint16_t index = readShort();
        if (index >= bytecode.strings.size()) {
            throw std::runtime_error("BytecodeReader: invalid string index");
        }
        return bytecode.strings[index];
    }
    
    // 读取数字常量
    double readNumber() {
        uint16_t index = readShort();
        if (index >= bytecode.numbers.size()) {
            throw std::runtime_error("BytecodeReader: invalid number index");
        }
        return bytecode.numbers[index];
    }
    
    // 读取常量
    std::string readConstant() {
        uint16_t index = readShort();
        if (index >= bytecode.constants.size()) {
            throw std::runtime_error("BytecodeReader: invalid constant index");
        }
        return bytecode.constants[index];
    }
    
    // 读取函数名
    std::string readFunction() {
        uint16_t index = readShort();
        if (index >= bytecode.functions.size()) {
            throw std::runtime_error("BytecodeReader: invalid function index");
        }
        return bytecode.functions[index];
    }
    
    // 设置当前位置
    void setPosition(size_t pos) {
        if (pos > bytecode.code.size()) {
            throw std::runtime_error("BytecodeReader: position out of bounds");
        }
        position = pos;
    }
    
    // 获取当前位置
    size_t getPosition() const {
        return position;
    }
    
    // 检查是否到达末尾
    bool isAtEnd() const {
        return position >= bytecode.code.size();
    }
    
private:
    const Bytecode& bytecode;
    size_t position;
};

// 保存字节码到文件
void saveBytecodeToFile(const Bytecode& bytecode, const std::string& filename);

// 从文件加载字节码
Bytecode loadBytecodeFromFile(const std::string& filename);
