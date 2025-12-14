#ifndef SLIME_GC_H
#define SLIME_GC_H

#ifdef __cplusplus
extern "C" {
#endif

// 前向声明垃圾回收器类型
typedef struct GarbageCollector GarbageCollector;

// 创建新的垃圾回收器
GarbageCollector* slime_gc_new();

// 销毁垃圾回收器
void slime_gc_destroy(GarbageCollector* gc);

// 注册对象
void slime_gc_register_object(GarbageCollector* gc, void* obj);

// 添加根对象
void slime_gc_mark_root(GarbageCollector* gc, void* obj);

// 移除根对象
void slime_gc_unmark_root(GarbageCollector* gc, void* obj);

// 批量添加根对象
void slime_gc_add_roots(GarbageCollector* gc, void** roots, int count);

// 批量移除根对象
void slime_gc_remove_roots(GarbageCollector* gc, void** roots, int count);

// 清除所有根对象标记
void slime_gc_clear_roots(GarbageCollector* gc);

// 获取当前根对象数量
int slime_gc_get_root_count(const GarbageCollector* gc);

// 添加引用关系
void slime_gc_add_reference(GarbageCollector* gc, void* from, void* to);

// 移除引用关系
void slime_gc_remove_reference(GarbageCollector* gc, void* from, void* to);

// 移除对象的所有引用
void slime_gc_clear_references(GarbageCollector* gc, void* obj);

// 获取对象的引用集合大小
int slime_gc_get_reference_count(const GarbageCollector* gc, void* obj);

// 批量添加引用
void slime_gc_add_references(GarbageCollector* gc, void* from, void** to_list, int count);

// 批量移除引用
void slime_gc_remove_references(GarbageCollector* gc, void* from, void** to_list, int count);

// 注销对象
void slime_gc_unregister_object(GarbageCollector* gc, void* obj);

// 执行垃圾回收
int slime_gc_collect(GarbageCollector* gc);

#ifdef __cplusplus
}
#endif

#endif // SLIME_GC_H