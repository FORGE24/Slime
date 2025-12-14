#include <iostream>
#include "slime_gc.h"

// 简单的测试类
class TestObj {
public:
    TestObj(int value) : value_(value) {
        std::cout << "TestObj created: " << value_ << std::endl;
    }
    ~TestObj() {
        std::cout << "TestObj destroyed: " << value_ << std::endl;
    }
    int getValue() const { return value_; }
private:
    int value_;
};

int main() {
    std::cout << "Testing garbage collector..." << std::endl;
    
    // 创建垃圾收集器
    GarbageCollector* gc = slime_gc_new();
    std::cout << "GC created" << std::endl;
    
    // 创建一些对象
    TestObj* obj1 = new TestObj(1);
    TestObj* obj2 = new TestObj(2);
    TestObj* obj3 = new TestObj(3);
    
    // 注册对象
    slime_gc_register_object(gc, obj1);
    slime_gc_register_object(gc, obj2);
    slime_gc_register_object(gc, obj3);
    
    // 执行垃圾回收
    int count = slime_gc_collect(gc);
    std::cout << "GC collected " << count << " objects" << std::endl;
    
    // 注销对象
    slime_gc_unregister_object(gc, obj1);
    slime_gc_unregister_object(gc, obj2);
    slime_gc_unregister_object(gc, obj3);
    
    // 删除对象
    delete obj1;
    delete obj2;
    delete obj3;
    
    // 销毁垃圾收集器
    slime_gc_destroy(gc);
    std::cout << "GC destroyed" << std::endl;
    
    return 0;
}