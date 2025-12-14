#pragma once

#include <string>
#include <variant>
#include <iostream>
#include <vector>
#include <map>

// 统一的值类型系统，支持 number/string/bool/array/hash
class Value {
public:
    // 值的类型枚举
    enum class Type {
        NUMBER,
        STRING,
        BOOLEAN,
        NIL,
        ARRAY,
        HASH
    };
    
    // 构造函数
    Value() : type_(Type::NIL), value_(nullptr) {}
    Value(double number) : type_(Type::NUMBER), value_(number) {}
    Value(int number) : type_(Type::NUMBER), value_(static_cast<double>(number)) {}
    Value(const std::string& str) : type_(Type::STRING), value_(str) {}
    Value(bool boolean) : type_(Type::BOOLEAN), value_(boolean) {}
    Value(const char* str) : type_(Type::STRING), value_(std::string(str)) {}
    Value(const std::vector<Value>& array) : type_(Type::ARRAY), value_(array) {}
    Value(const std::map<std::string, Value>& hash) : type_(Type::HASH), value_(hash) {}
    
    // 拷贝构造函数
    Value(const Value& other) : type_(other.type_), value_(other.value_) {}
    
    // 赋值操作符
    Value& operator=(const Value& other) {
        if (this != &other) {
            type_ = other.type_;
            value_ = other.value_;
        }
        return *this;
    }
    
    // 类型检查
    Type getType() const { return type_; }
    bool isNumber() const { return type_ == Type::NUMBER; }
    bool isString() const { return type_ == Type::STRING; }
    bool isBoolean() const { return type_ == Type::BOOLEAN; }
    bool isNil() const { return type_ == Type::NIL; }
    bool isArray() const { return type_ == Type::ARRAY; }
    bool isHash() const { return type_ == Type::HASH; }
    
    // 值获取（需要类型检查）
    double asNumber() const {
        if (type_ != Type::NUMBER) {
            throw std::runtime_error("Value is not a number");
        }
        return std::get<double>(value_);
    }
    
    std::string asString() const {
        if (type_ != Type::STRING) {
            throw std::runtime_error("Value is not a string");
        }
        return std::get<std::string>(value_);
    }
    
    bool asBoolean() const {
        if (type_ != Type::BOOLEAN) {
            throw std::runtime_error("Value is not a boolean");
        }
        return std::get<bool>(value_);
    }
    
    std::vector<Value>& asArray() {
        if (type_ != Type::ARRAY) {
            throw std::runtime_error("Value is not an array");
        }
        return std::get<std::vector<Value>>(value_);
    }
    
    const std::vector<Value>& asArray() const {
        if (type_ != Type::ARRAY) {
            throw std::runtime_error("Value is not an array");
        }
        return std::get<std::vector<Value>>(value_);
    }
    
    std::map<std::string, Value>& asHash() {
        if (type_ != Type::HASH) {
            throw std::runtime_error("Value is not a hash");
        }
        return std::get<std::map<std::string, Value>>(value_);
    }
    
    const std::map<std::string, Value>& asHash() const {
        if (type_ != Type::HASH) {
            throw std::runtime_error("Value is not a hash");
        }
        return std::get<std::map<std::string, Value>>(value_);
    }
    
    // 数组和哈希表操作
    void push(const Value& item) {
        asArray().push_back(item);
    }
    
    Value& operator[](const Value& index) {
        if (isArray()) {
            int idx = static_cast<int>(index.toNumber());
            auto& array = asArray();
            if (idx < 0 || idx >= static_cast<int>(array.size())) {
                throw std::runtime_error("Array index out of bounds");
            }
            return array[idx];
        } else if (isHash()) {
            return asHash()[index.asString()];
        } else {
            throw std::runtime_error("Indexing not supported for this type");
        }
    }
    
    // 转换为字符串表示
    std::string toString() const {
        switch (type_) {
            case Type::NUMBER:
                return std::to_string(asNumber());
            case Type::STRING:
                return asString();
            case Type::BOOLEAN:
                return asBoolean() ? "true" : "false";
            case Type::NIL:
                return "nil";
            case Type::ARRAY: {
                std::string result = "[";
                const auto& array = asArray();
                for (size_t i = 0; i < array.size(); ++i) {
                    if (i > 0) result += ", ";
                    result += array[i].toString();
                }
                result += "]";
                return result;
            }
            case Type::HASH: {
                std::string result = "{";
                const auto& hash = asHash();
                size_t i = 0;
                for (const auto& pair : hash) {
                    if (i > 0) result += ", ";
                    result += pair.first + ": " + pair.second.toString();
                    ++i;
                }
                result += "}";
                return result;
            }
            default:
                return "unknown";
        }
    }
    
    // 转换为数字表示
    double toNumber() const {
        switch (type_) {
            case Type::NUMBER:
                return asNumber();
            case Type::STRING: {
                try {
                    return std::stod(asString());
                } catch (...) {
                    return 0.0;
                }
            }
            case Type::BOOLEAN:
                return asBoolean() ? 1.0 : 0.0;
            case Type::NIL:
                return 0.0;
            case Type::ARRAY:
                return static_cast<double>(asArray().size());
            case Type::HASH:
                return static_cast<double>(asHash().size());
            default:
                return 0.0;
        }
    }
    
    // 转换为布尔表示
    bool toBoolean() const {
        switch (type_) {
            case Type::NUMBER:
                return asNumber() != 0.0;
            case Type::STRING:
                return !asString().empty();
            case Type::BOOLEAN:
                return asBoolean();
            case Type::NIL:
                return false;
            case Type::ARRAY:
                return !asArray().empty();
            case Type::HASH:
                return !asHash().empty();
            default:
                return false;
        }
    }
    
    // 比较操作
    bool operator==(const Value& other) const {
        if (type_ != other.type_) return false;
        
        switch (type_) {
            case Type::NUMBER:
                return asNumber() == other.asNumber();
            case Type::STRING:
                return asString() == other.asString();
            case Type::BOOLEAN:
                return asBoolean() == other.asBoolean();
            case Type::NIL:
                return true;
            case Type::ARRAY: {
                const auto& arr1 = asArray();
                const auto& arr2 = other.asArray();
                if (arr1.size() != arr2.size()) return false;
                for (size_t i = 0; i < arr1.size(); ++i) {
                    if (arr1[i] != arr2[i]) return false;
                }
                return true;
            }
            case Type::HASH: {
                const auto& hash1 = asHash();
                const auto& hash2 = other.asHash();
                if (hash1.size() != hash2.size()) return false;
                for (const auto& pair : hash1) {
                    auto it = hash2.find(pair.first);
                    if (it == hash2.end() || pair.second != it->second) return false;
                }
                return true;
            }
            default:
                return false;
        }
    }
    
    bool operator!=(const Value& other) const {
        return !(*this == other);
    }
    
    // 算术操作
    Value operator+(const Value& other) const {
        if (isNumber() && other.isNumber()) {
            return Value(asNumber() + other.asNumber());
        } else {
            return Value(toString() + other.toString());
        }
    }
    
    Value operator-(const Value& other) const {
        if (isNumber() && other.isNumber()) {
            return Value(asNumber() - other.asNumber());
        } else {
            // 字符串减法不支持，返回0
            return Value(0.0);
        }
    }
    
    Value operator*(const Value& other) const {
        if (isNumber() && other.isNumber()) {
            return Value(asNumber() * other.asNumber());
        } else if (isString() && other.isNumber()) {
            // 字符串重复
            std::string result;
            int times = static_cast<int>(other.asNumber());
            for (int i = 0; i < times; i++) {
                result += asString();
            }
            return Value(result);
        } else if (isNumber() && other.isString()) {
            // 数字乘以字符串
            std::string result;
            int times = static_cast<int>(asNumber());
            for (int i = 0; i < times; i++) {
                result += other.asString();
            }
            return Value(result);
        } else {
            return Value(0.0);
        }
    }
    
    Value operator/(const Value& other) const {
        if (isNumber() && other.isNumber()) {
            if (other.asNumber() != 0.0) {
                return Value(asNumber() / other.asNumber());
            } else {
                throw std::runtime_error("Division by zero");
            }
        } else {
            return Value(0.0);
        }
    }
    
    Value operator%(const Value& other) const {
        if (isNumber() && other.isNumber()) {
            if (other.asNumber() != 0.0) {
                return Value(static_cast<double>(static_cast<int>(asNumber()) % static_cast<int>(other.asNumber())));
            } else {
                throw std::runtime_error("Modulo by zero");
            }
        } else {
            return Value(0.0);
        }
    }
    
private:
    Type type_;
    std::variant<std::nullptr_t, double, std::string, bool, std::vector<Value>, std::map<std::string, Value>> value_;
};