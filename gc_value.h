#pragma once

#include "value.h"
#include "slime_gc.h"

// 扩展的Value类，集成了垃圾回收功能
class GCValue : public Value {
public:
    // 构造函数
    GCValue() : Value() {
        initGC();
        slime_gc_register_object(gc_, this);
    }
    
    GCValue(double number) : Value(number) {
        initGC();
        slime_gc_register_object(gc_, this);
    }
    
    GCValue(int number) : Value(number) {
        initGC();
        slime_gc_register_object(gc_, this);
    }
    
    GCValue(const std::string& str) : Value(str) {
        initGC();
        slime_gc_register_object(gc_, this);
    }
    
    GCValue(bool boolean) : Value(boolean) {
        initGC();
        slime_gc_register_object(gc_, this);
    }
    
    GCValue(const char* str) : Value(str) {
        initGC();
        slime_gc_register_object(gc_, this);
    }
    
    GCValue(const std::vector<GCValue>& array) {
        // 将std::vector<GCValue>转换为std::vector<Value>
        std::vector<Value> valueArray;
        valueArray.reserve(array.size());
        for (const auto& item : array) {
            valueArray.push_back(item);
        }
        
        // 调用Value的构造函数
        *static_cast<Value*>(this) = Value(valueArray);
        
        // 初始化GC
        initGC();
        slime_gc_register_object(gc_, this);
    }
    
    GCValue(const std::map<std::string, GCValue>& hash) {
        // 将std::map<std::string, GCValue>转换为std::map<std::string, Value>
        std::map<std::string, Value> valueHash;
        for (const auto& pair : hash) {
            valueHash[pair.first] = pair.second;
        }
        
        // 调用Value的构造函数
        *static_cast<Value*>(this) = Value(valueHash);
        
        // 初始化GC
        initGC();
        slime_gc_register_object(gc_, this);
    }
    
    // 拷贝构造函数
    GCValue(const Value& other) : Value(other) {
        initGC();
        slime_gc_register_object(gc_, this);
        // 如果other是GCValue类型，建立引用关系
        if (typeid(other) == typeid(GCValue)) {
            const GCValue& otherGC = static_cast<const GCValue&>(other);
            addAllReferences(otherGC);
        }
    }
    
    // 赋值操作符
    GCValue& operator=(const Value& other) {
        if (this != &other) {
            // 移除旧的引用关系
            slime_gc_clear_references(gc_, this);
            
            // 执行基类赋值
            Value::operator=(other);
            
            // 添加新的引用关系
            if (typeid(other) == typeid(GCValue)) {
                const GCValue& otherGC = static_cast<const GCValue&>(other);
                addAllReferences(otherGC);
            }
        }
        return *this;
    }
    
    // GCValue拷贝构造函数
    GCValue(const GCValue& other) : Value(other) {
        initGC();
        slime_gc_register_object(gc_, this);
        addAllReferences(other);
    }
    
    // GCValue赋值操作符
    GCValue& operator=(const GCValue& other) {
        if (this != &other) {
            // 移除旧的引用关系
            slime_gc_clear_references(gc_, this);
            
            // 执行基类赋值
            Value::operator=(other);
            
            // 添加新的引用关系
            addAllReferences(other);
        }
        return *this;
    }
    
    // 递归添加所有引用（包括数组和哈希表内部的引用）
    void addAllReferences(const GCValue& other) {
        // 直接添加引用
        slime_gc_add_reference(gc_, this, const_cast<GCValue*>(&other));
        
        // 对于复杂类型的引用跟踪，需要重新设计GCValue类的结构
        // 当前实现只处理简单类型的引用跟踪
        if (other.isArray()) {
            const auto& array = other.asArray();
            // TODO: 实现数组内部元素的引用跟踪
        }
        else if (other.isHash()) {
            const auto& hash = other.asHash();
            // TODO: 实现哈希表内部值的引用跟踪
        }
        else {
            // 其他类型直接处理
        }
    }
    
    // 析构函数
    ~GCValue() {
        if (gc_ != nullptr) {
            slime_gc_clear_references(gc_, this);
            slime_gc_unregister_object(gc_, this);
        }
    }
    
    // 执行垃圾回收
    static void collectGarbage() {
        if (gc_ != nullptr) {
            // 标记根对象：栈上的所有对象和变量
            markRoots();
            
            // 执行垃圾回收
            slime_gc_collect(gc_);
            
            // 清除根对象标记
            unmarkRoots();
        }
    }
    
    // 标记根对象
    static void markRoots() {
        if (gc_ == nullptr) return;
        
        // 标记所有变量为根对象
        if (variables_ != nullptr) {
            for (auto& pair : *variables_) {
                slime_gc_mark_root(gc_, &pair.second);
            }
        }
        
        // 标记所有栈对象为根对象
        if (stack_ != nullptr) {
            for (auto& value : *stack_) {
                slime_gc_mark_root(gc_, &value);
            }
        }
    }
    
    // 清除根对象标记
    static void unmarkRoots() {
        if (gc_ == nullptr) return;
        slime_gc_clear_roots(gc_);
    }
    
    // 注册栈和变量（供VM使用）
    static void registerVM(std::vector<GCValue>& stack, std::map<std::string, GCValue>& variables) {
        stack_ = &stack;
        variables_ = &variables;
    }
    
    // 只注册变量（供Interpreter使用）
    static void registerVariables(std::map<std::string, GCValue>& variables) {
        stack_ = nullptr;
        variables_ = &variables;
    }
    
    // 注销VM
    static void unregisterVM() {
        stack_ = nullptr;
        variables_ = nullptr;
    }
    
    // 数组操作 - 向数组添加元素
    void push(const GCValue& item) {
        if (isArray()) {
            // 获取数组并添加元素
            auto& array = asArray();
            array.push_back(item);
            // 添加引用关系
            slime_gc_add_reference(gc_, this, const_cast<GCValue*>(&item));
        }
    }
    
    // 数组操作 - 获取数组元素
    GCValue& at(const GCValue& index) {
        if (isArray()) {
            int idx = static_cast<int>(index.toNumber());
            auto& array = asArray();
            if (idx < 0 || idx >= static_cast<int>(array.size())) {
                throw std::runtime_error("Array index out of bounds");
            }
            return static_cast<GCValue&>(array[idx]);
        } else {
            throw std::runtime_error("Indexing not supported for this type");
        }
    }
    
    // 哈希表操作 - 设置键值对
    void set(const std::string& key, const GCValue& value) {
        if (isHash()) {
            auto& hash = asHash();
            // 设置新值
            hash[key] = value;
            // 添加新引用
            slime_gc_add_reference(gc_, this, const_cast<GCValue*>(&value));
        }
    }
    
    // 哈希表操作 - 获取键值
    GCValue& get(const std::string& key) {
        if (isHash()) {
            auto& hash = asHash();
            auto it = hash.find(key);
            if (it != hash.end()) {
                return static_cast<GCValue&>(it->second);
            } else {
                // 返回nil值
                static GCValue nilValue;
                return nilValue;
            }
        } else {
            throw std::runtime_error("Key access not supported for this type");
        }
    }
    
private:
    static void initGC() {
        if (gc_ == nullptr) {
            gc_ = slime_gc_new();
        }
    }
    
    // 全局GC实例
    static GarbageCollector* gc_;
    
    // VM的栈和变量引用（用于根对象标记）
    static std::vector<GCValue>* stack_;
    static std::map<std::string, GCValue>* variables_;
};

// 初始化静态成员
GarbageCollector* GCValue::gc_ = nullptr;
std::vector<GCValue>* GCValue::stack_ = nullptr;
std::map<std::string, GCValue>* GCValue::variables_ = nullptr;