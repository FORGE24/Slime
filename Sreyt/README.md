# Sreyt GUI Library

**Sreyt** (slime GUI library) 是为 slime 编程语言设计的图形用户界面库，遵循 "万物皆接口" 的核心理念。

## 特性

- **接口驱动架构**: 所有GUI组件都基于静态接口定义
- **完整控件系统**: 支持 Label、Button、Entry、Canvas、Frame 等基础控件
- **灵活布局管理**: 提供 Grid、Pack、Place 三种布局策略
- **事件处理系统**: 完整的窗口和控件事件响应机制
- **响应式设计**: 自动布局调整和窗口缩放支持
- **主题系统**: 支持明亮/暗色主题切换

## 快速开始

### 1. 初始化Sreyt

```slime
call sreyt_init
```

### 2. 创建应用窗口

```slime  
let main_window = call sreyt_create_app, "My App", 800, 600
```

### 3. 添加控件

```slime
let hello_label = call label_create, main_window, "Hello, Sreyt!", 100, 50
let quit_button = call button_create, main_window, "Quit", 100, 100, 80, 30

call widget_show, hello_label
call widget_show, quit_button
```

### 4. 运行应用

```slime
call sreyt_run_app, main_window
```

## 库结构

```
Sreyt/
├── sreyt.sm      # 主库文件和统一接口
├── core.sm       # 核心系统（窗口管理、事件处理、图形接口）
├── widgets.sm    # 控件系统（Button、Label、Entry、Canvas、Frame）
├── layout.sm     # 布局管理器（Grid、Pack、Place）
├── demo.sm       # 完整演示应用程序
└── README.md     # 本文档
```

## 核心接口

### 窗口管理
- `window_create(title, width, height)` - 创建窗口
- `window_show(window_id)` - 显示窗口
- `window_destroy(window_id)` - 销毁窗口
- `window_set_resizable(window_id, resizable)` - 设置可缩放

### 基础控件

#### Label (文本标签)
```slime
let label_id = call label_create, parent_window, "Hello World", x, y
call label_set_text, label_id, "New Text"
call label_set_color, label_id, 0xFF0000  // 红色
```

#### Button (按钮)
```slime
let button_id = call button_create, parent_window, "Click Me", x, y, width, height
call button_on_click, button_id, my_callback_function
```

#### Entry (输入框)
```slime
let entry_id = call entry_create, parent_window, x, y, width, height
call entry_set_text, entry_id, "Default text"
let text = call entry_get_text, entry_id
```

#### Canvas (画布)
```slime
let canvas_id = call canvas_create, parent_window, x, y, width, height
call canvas_draw_rect, canvas_id, x, y, width, height, color
call canvas_draw_text, canvas_id, "Hello", x, y, color
call canvas_draw_line, canvas_id, x1, y1, x2, y2, color
```

### 布局管理

#### Grid布局 (网格)
```slime
let grid_layout = call grid_layout_create, parent_window, rows, cols
call grid_add_widget, grid_layout, widget_id, row, col, rowspan, colspan
```

#### Pack布局 (序列)
```slime
let pack_layout = call pack_layout_create, parent_window, direction
call pack_add_widget, pack_layout, widget_id, padding, align
```

#### Place布局 (绝对定位)
```slime
let place_layout = call place_layout_create, parent_window
call place_add_widget, place_layout, widget_id, x, y, width, height
```

## 事件处理

### 窗口事件
```slime
call on_window_close, window_id, cleanup_function
call on_window_resize, window_id, resize_handler
```

### 控件事件
```slime
call button_on_click, button_id, click_handler
call on_key_press, widget_id, key_handler
call on_mouse_move, widget_id, mouse_handler
```

## 完整示例

### Hello World 应用
```slime
fn create_hello_app()
    // 初始化
    call sreyt_init
    
    // 创建窗口
    let window = call sreyt_create_app, "Hello Sreyt", 300, 200
    
    // 添加控件
    let label = call label_create, window, "Hello, World!", 50, 50
    let button = call button_create, window, "Click Me", 50, 100, 80, 30
    
    // 显示控件
    call widget_show, label
    call widget_show, button
    
    // 运行应用
    call sreyt_run_app, window
end

call create_hello_app
```

### 计算器示例
```slime
fn create_calculator()
    call sreyt_init
    
    let window = call sreyt_create_app, "Calculator", 250, 300
    
    // 创建Grid布局
    let layout = call grid_layout_create, window, 5, 4
    
    // 显示屏
    let display = call entry_create, window, 0, 0, 200, 30
    call grid_add_widget, layout, display, 0, 0, 1, 4
    
    // 数字按钮
    let btn1 = call button_create, window, "1", 0, 0, 50, 40
    call grid_add_widget, layout, btn1, 1, 0, 1, 1
    
    let btn2 = call button_create, window, "2", 0, 0, 50, 40
    call grid_add_widget, layout, btn2, 1, 1, 1, 1
    
    // ... 更多按钮
    
    call widget_show, display
    call widget_show, btn1
    call widget_show, btn2
    
    call sreyt_run_app, window
end
```

## 运行演示

查看完整的演示应用程序：

```slime
call run_sreyt_demo      // 完整功能演示
call run_simple_demo     // 简化演示
call sreyt_run_hello_world  // Hello World示例
```

## 编译和运行

1. 确保 slime 编译器已正确安装
2. 将 Sreyt 目录添加到 slime 项目中
3. 在应用程序中导入 Sreyt 模块
4. 使用 slime 编译器编译项目

```bash
# Windows 编译示例
.\build.ps1
slimec.exe my_gui_app.sm
```

## 系统要求

- **slime 编译器**: 支持接口系统的版本
- **操作系统**: Windows (当前版本)
- **依赖项**: NASM, GoLink (通过 slime 构建系统)

## 设计理念

Sreyt 遵循 slime 语言的 "万物皆接口" 哲学：

1. **接口驱动**: 所有GUI操作都通过静态接口定义
2. **组合优于继承**: 通过接口组合实现复杂功能
3. **简洁明确**: API设计注重简洁性和可读性
4. **可扩展性**: 易于添加新的控件和功能

## 架构说明

```
Application Layer     (用户应用程序)
    ↓
Sreyt API Layer      (sreyt.sm - 统一接口)
    ↓
Widget System        (widgets.sm - 控件实现)
Layout System        (layout.sm - 布局管理)
    ↓
Core System          (core.sm - 核心功能)
    ↓
Interface Layer      (静态接口定义)
    ↓
Platform Backend     (Windows/NASM/GoLink)
```

## 开发状态

- ✅ 核心系统架构
- ✅ 基础控件实现  
- ✅ 布局管理器
- ✅ 事件处理框架
- ✅ 演示应用程序
- 🚧 主题系统 (基础版)
- 📋 平台特定实现
- 📋 高级控件 (树形视图、表格等)
- 📋 完整文档

## 贡献

欢迎为 Sreyt 贡献代码！请遵循 slime 语言的编码规范和 "万物皆接口" 的设计原则。

## 许可证

与 slime 编程语言项目相同的许可证。

---

**Sreyt** - 让 slime 拥有强大而优雅的图形界面能力！