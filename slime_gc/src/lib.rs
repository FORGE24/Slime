//! Slime语言的垃圾回收器实现
//! 使用Rust编写以确保内存安全

use std::collections::{HashSet, HashMap};
use std::os::raw::{c_int, c_void};

/// 垃圾回收器
pub struct GarbageCollector {
    /// 所有对象的集合
    objects: HashSet<*mut c_void>,
    /// 根对象集合
    roots: HashSet<*mut c_void>,
    /// 对象引用关系：从一个对象到它引用的所有对象
    references: HashMap<*mut c_void, HashSet<*mut c_void>>,
}

impl GarbageCollector {
    /// 创建新的垃圾回收器
    pub fn new() -> Self {
        GarbageCollector {
            objects: HashSet::new(),
            roots: HashSet::new(),
            references: HashMap::new(),
        }
    }

    /// 注册新对象
    pub fn register_object(&mut self, obj: *mut c_void) {
        if !obj.is_null() {
            self.objects.insert(obj);
            self.references.insert(obj, HashSet::new());
        }
    }

    /// 注销对象
    pub fn unregister_object(&mut self, obj: *mut c_void) {
        if !obj.is_null() {
            self.objects.remove(&obj);
            self.roots.remove(&obj);
            self.references.remove(&obj);
            
            // 从其他对象的引用列表中移除该对象
            for refs in self.references.values_mut() {
                refs.remove(&obj);
            }
        }
    }

    /// 添加对象引用
    pub fn add_reference(&mut self, from: *mut c_void, to: *mut c_void) {
        if !from.is_null() && !to.is_null() {
            // 确保from对象已注册
            if self.objects.contains(&from) {
                // 获取或创建from对象的引用集合
                let refs = self.references.entry(from).or_insert(HashSet::new());
                // 添加引用
                refs.insert(to);
            }
        }
    }

    /// 移除对象引用
    pub fn remove_reference(&mut self, from: *mut c_void, to: *mut c_void) {
        if !from.is_null() && !to.is_null() {
            if let Some(refs) = self.references.get_mut(&from) {
                refs.remove(&to);
            }
        }
    }

    /// 移除对象的所有引用
    pub fn clear_references(&mut self, obj: *mut c_void) {
        if !obj.is_null() {
            self.references.remove(&obj);
        }
    }

    /// 获取对象的引用集合
    pub fn get_references(&self, obj: *mut c_void) -> Option<&HashSet<*mut c_void>> {
        self.references.get(&obj)
    }

    /// 批量添加引用
    pub fn add_references(&mut self, from: *mut c_void, to_list: &[*mut c_void]) {
        if !from.is_null() && !to_list.is_empty() {
            if self.objects.contains(&from) {
                let refs = self.references.entry(from).or_insert(HashSet::new());
                for &to in to_list {
                    if !to.is_null() {
                        refs.insert(to);
                    }
                }
            }
        }
    }

    /// 批量移除引用
    pub fn remove_references(&mut self, from: *mut c_void, to_list: &[*mut c_void]) {
        if !from.is_null() && !to_list.is_empty() {
            if let Some(refs) = self.references.get_mut(&from) {
                for &to in to_list {
                    refs.remove(&to);
                }
            }
        }
    }

    /// 将对象标记为根对象
    pub fn mark_root(&mut self, obj: *mut c_void) {
        if !obj.is_null() && self.objects.contains(&obj) {
            self.roots.insert(obj);
        }
    }

    /// 将对象标记为非根对象
    pub fn unmark_root(&mut self, obj: *mut c_void) {
        self.roots.remove(&obj);
    }

    /// 批量添加根对象
    pub fn add_roots(&mut self, roots: &[*mut c_void]) {
        for &obj in roots {
            self.mark_root(obj);
        }
    }

    /// 批量移除根对象
    pub fn remove_roots(&mut self, roots: &[*mut c_void]) {
        for &obj in roots {
            self.unmark_root(obj);
        }
    }

    /// 清除所有根对象标记
    pub fn clear_roots(&mut self) {
        self.roots.clear();
    }

    /// 获取当前根对象数量
    pub fn get_root_count(&self) -> usize {
        self.roots.len()
    }

    /// 执行垃圾回收
    pub fn collect_garbage(&mut self) -> usize {
        if self.objects.is_empty() {
            return 0;
        }

        // 步骤1: 标记所有可达对象
        let mut marked = HashSet::new();
        
        // 从根对象开始标记
        for &root in &self.roots {
            self.mark(root, &mut marked);
        }

        // 步骤2: 清除所有未标记的对象
        let mut collected = 0;
        let mut to_remove = Vec::new();

        for &obj in &self.objects {
            if !marked.contains(&obj) {
                // 注意：这里不直接释放对象，因为对象是在C++中用new创建的
                // 对象的释放由C++的析构函数负责
                to_remove.push(obj);
                collected += 1;
            }
        }

        // 从集合中移除已释放的对象
        for obj in to_remove {
            self.objects.remove(&obj);
            self.references.remove(&obj);
            
            // 从其他对象的引用列表中移除该对象
            for refs in self.references.values_mut() {
                refs.remove(&obj);
            }
        }

        collected
    }

    /// 递归标记对象及其引用的对象
    fn mark(&self, obj: *mut c_void, marked: &mut HashSet<*mut c_void>) {
        // 检查对象是否为空或已标记
        if obj.is_null() || marked.contains(&obj) {
            return;
        }

        // 检查对象是否在已注册的对象集合中
        if !self.objects.contains(&obj) {
            return;
        }

        // 标记当前对象
        marked.insert(obj);

        // 标记所有引用的对象
        if let Some(refs) = self.references.get(&obj) {
            for &ref_obj in refs {
                self.mark(ref_obj, marked);
            }
        }
    }
}

/// C接口函数，用于创建垃圾回收器
#[unsafe(no_mangle)]
pub extern "C" fn slime_gc_new() -> *mut GarbageCollector {
    Box::into_raw(Box::new(GarbageCollector::new()))
}

/// C接口函数，用于销毁垃圾回收器
#[unsafe(no_mangle)]
pub extern "C" fn slime_gc_destroy(gc: *mut GarbageCollector) {
    if !gc.is_null() {
        unsafe {
            // 销毁GC之前，先释放所有对象
            (*gc).collect_garbage();
            drop(Box::from_raw(gc));
        }
    }
}

/// C接口函数，用于注册对象
#[unsafe(no_mangle)]
pub extern "C" fn slime_gc_register_object(gc: *mut GarbageCollector, obj: *mut c_void) {
    if !gc.is_null() && !obj.is_null() {
        unsafe {
            (*gc).register_object(obj);
        }
    }
}

/// C接口函数，用于注销对象
#[unsafe(no_mangle)]
pub extern "C" fn slime_gc_unregister_object(gc: *mut GarbageCollector, obj: *mut c_void) {
    if !gc.is_null() && !obj.is_null() {
        unsafe {
            (*gc).unregister_object(obj);
        }
    }
}

/// C接口函数，用于添加对象引用
#[unsafe(no_mangle)]
pub extern "C" fn slime_gc_add_reference(gc: *mut GarbageCollector, from: *mut c_void, to: *mut c_void) {
    if !gc.is_null() && !from.is_null() && !to.is_null() {
        unsafe {
            (*gc).add_reference(from, to);
        }
    }
}

/// C接口函数，用于移除对象引用
#[unsafe(no_mangle)]
pub extern "C" fn slime_gc_remove_reference(gc: *mut GarbageCollector, from: *mut c_void, to: *mut c_void) {
    if !gc.is_null() && !from.is_null() && !to.is_null() {
        unsafe {
            (*gc).remove_reference(from, to);
        }
    }
}

/// C接口函数，用于移除对象的所有引用
#[unsafe(no_mangle)]
pub extern "C" fn slime_gc_clear_references(gc: *mut GarbageCollector, obj: *mut c_void) {
    if !gc.is_null() && !obj.is_null() {
        unsafe {
            (*gc).clear_references(obj);
        }
    }
}

/// C接口函数，用于获取对象的引用集合大小
#[unsafe(no_mangle)]
pub extern "C" fn slime_gc_get_reference_count(gc: *const GarbageCollector, obj: *mut c_void) -> c_int {
    if !gc.is_null() && !obj.is_null() {
        unsafe {
            if let Some(refs) = (*gc).get_references(obj) {
                return refs.len() as c_int;
            }
        }
    }
    0
}

/// C接口函数，用于批量添加引用
#[unsafe(no_mangle)]
pub extern "C" fn slime_gc_add_references(gc: *mut GarbageCollector, from: *mut c_void, to_list: *const *mut c_void, count: c_int) {
    if !gc.is_null() && !from.is_null() && !to_list.is_null() && count > 0 {
        unsafe {
            let to_slice = std::slice::from_raw_parts(to_list, count as usize);
            (*gc).add_references(from, to_slice);
        }
    }
}

/// C接口函数，用于批量移除引用
#[unsafe(no_mangle)]
pub extern "C" fn slime_gc_remove_references(gc: *mut GarbageCollector, from: *mut c_void, to_list: *const *mut c_void, count: c_int) {
    if !gc.is_null() && !from.is_null() && !to_list.is_null() && count > 0 {
        unsafe {
            let to_slice = std::slice::from_raw_parts(to_list, count as usize);
            (*gc).remove_references(from, to_slice);
        }
    }
}

/// C接口函数，用于标记根对象
#[unsafe(no_mangle)]
pub extern "C" fn slime_gc_mark_root(gc: *mut GarbageCollector, obj: *mut c_void) {
    if !gc.is_null() && !obj.is_null() {
        unsafe {
            (*gc).mark_root(obj);
        }
    }
}

/// C接口函数，用于取消标记根对象
#[unsafe(no_mangle)]
pub extern "C" fn slime_gc_unmark_root(gc: *mut GarbageCollector, obj: *mut c_void) {
    if !gc.is_null() && !obj.is_null() {
        unsafe {
            (*gc).unmark_root(obj);
        }
    }
}

/// C接口函数，用于清除所有根对象标记
#[unsafe(no_mangle)]
pub extern "C" fn slime_gc_clear_roots(gc: *mut GarbageCollector) {
    if !gc.is_null() {
        unsafe {
            (*gc).clear_roots();
        }
    }
}

/// C接口函数，用于执行垃圾回收
#[unsafe(no_mangle)]
pub extern "C" fn slime_gc_collect(gc: *mut GarbageCollector) -> c_int {
    if !gc.is_null() {
        unsafe {
            (*gc).collect_garbage() as c_int
        }
    } else {
        0
    }
}