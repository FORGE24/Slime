; ============================================================
; Sreyt GUI Runtime for Windows x64
; ============================================================
; Win32 GUI 运行时库 - 提供真实的窗口和控件支持
; ============================================================

; ============================================================
; 外部 Win32 API 声明
; ============================================================

extern GetModuleHandleA
extern RegisterClassExA
extern CreateWindowExA
extern ShowWindow
extern UpdateWindow
extern GetMessageA
extern TranslateMessage
extern DispatchMessageA
extern PostQuitMessage
extern DefWindowProcA
extern GetStdHandle
extern WriteFile
extern ExitProcess
extern SetWindowTextA
extern GetWindowTextA
extern GetWindowTextLengthA
extern SendMessageA
extern DestroyWindow
extern InvalidateRect
extern BeginPaint
extern EndPaint
extern FillRect
extern CreateSolidBrush
extern DeleteObject
extern SetBkMode
extern SetTextColor
extern TextOutA
extern GetDC
extern ReleaseDC
extern MoveWindow
extern SetFocus
extern MessageBoxA
extern LoadCursorA
extern GetClientRect

; ============================================================
; 常量定义
; ============================================================

; 窗口样式
%define WS_OVERLAPPEDWINDOW 0x00CF0000
%define WS_VISIBLE          0x10000000
%define WS_CHILD            0x40000000
%define WS_BORDER           0x00800000
%define WS_TABSTOP          0x00010000

; 按钮样式
%define BS_PUSHBUTTON       0x00000000
%define BS_DEFPUSHBUTTON    0x00000001

; 编辑框样式
%define ES_LEFT             0x00000000
%define ES_CENTER           0x00000001
%define ES_RIGHT            0x00000002
%define ES_AUTOHSCROLL      0x00000080

; 静态文本样式
%define SS_LEFT             0x00000000
%define SS_CENTER           0x00000001
%define SS_RIGHT            0x00000002

; 消息
%define WM_CREATE           0x0001
%define WM_DESTROY          0x0002
%define WM_PAINT            0x000F
%define WM_CLOSE            0x0010
%define WM_COMMAND          0x0111
%define WM_SETTEXT          0x000C
%define WM_GETTEXT          0x000D
%define WM_GETTEXTLENGTH    0x000E

; 通知码
%define BN_CLICKED          0

; ShowWindow 参数
%define SW_SHOW             5
%define SW_HIDE             0

; 光标
%define IDC_ARROW           32512

; 颜色
%define COLOR_WINDOW        5
%define COLOR_BTNFACE       15

; 背景模式
%define TRANSPARENT         1
%define OPAQUE              2

; ============================================================
; 数据段
; ============================================================

section .data
    ; 窗口类名
    sreyt_class_name: db "SreytWindowClass", 0
    
    ; 控件类名
    class_button: db "BUTTON", 0
    class_edit: db "EDIT", 0
    class_static: db "STATIC", 0
    
    ; 默认标题
    default_title: db "Sreyt Application", 0
    
    ; 空字符串
    empty_string: db 0

section .bss
    ; 实例句柄
    hInstance: resq 1
    
    ; 窗口类结构 (WNDCLASSEXA = 80 bytes)
    wndclass: resb 80
    
    ; 消息结构 (MSG = 48 bytes)  
    msg: resb 48
    
    ; 绘图结构 (PAINTSTRUCT = 72 bytes)
    ps: resb 72
    
    ; 矩形结构 (RECT = 16 bytes)
    rect: resb 16
    
    ; 窗口句柄数组 (最多 64 个窗口/控件)
    hwnd_array: resq 64
    hwnd_count: resq 1
    
    ; 事件回调数组 (控件ID -> 回调地址)
    callback_array: resq 64
    
    ; 主窗口句柄
    main_hwnd: resq 1
    
    ; 临时缓冲区
    text_buffer: resb 256
    
    ; 控件 ID 计数器
    next_ctrl_id: resd 1

section .text

; ============================================================
; GUI 初始化
; ============================================================

global _gui_init
_gui_init:
    push rbp
    mov rbp, rsp
    sub rsp, 32
    
    ; 获取实例句柄
    xor rcx, rcx
    call GetModuleHandleA
    mov [hInstance], rax
    
    ; 初始化控件ID
    mov dword [next_ctrl_id], 1000
    
    ; 初始化窗口计数
    mov qword [hwnd_count], 0
    
    leave
    ret

; ============================================================
; 注册窗口类
; ============================================================

global _gui_register_class
_gui_register_class:
    push rbp
    mov rbp, rsp
    sub rsp, 96
    
    ; 填充 WNDCLASSEXA 结构
    lea rdi, [wndclass]
    
    ; cbSize = 80
    mov dword [rdi], 80
    ; style = 0
    mov dword [rdi+4], 0
    ; lpfnWndProc
    lea rax, [_gui_wndproc]
    mov [rdi+8], rax
    ; cbClsExtra = 0
    mov dword [rdi+16], 0
    ; cbWndExtra = 0
    mov dword [rdi+20], 0
    ; hInstance
    mov rax, [hInstance]
    mov [rdi+24], rax
    ; hIcon = 0
    mov qword [rdi+32], 0
    ; hCursor - 加载箭头光标
    mov rcx, 0
    mov rdx, IDC_ARROW
    call LoadCursorA
    lea rdi, [wndclass]
    mov [rdi+40], rax
    ; hbrBackground = COLOR_BTNFACE + 1
    mov qword [rdi+48], COLOR_BTNFACE + 1
    ; lpszMenuName = 0
    mov qword [rdi+56], 0
    ; lpszClassName
    lea rax, [sreyt_class_name]
    mov [rdi+64], rax
    ; hIconSm = 0
    mov qword [rdi+72], 0
    
    ; 注册窗口类
    lea rcx, [wndclass]
    call RegisterClassExA
    
    leave
    ret

; ============================================================
; 窗口过程 (Window Procedure)
; ============================================================

global _gui_wndproc
_gui_wndproc:
    ; 参数: rcx=hwnd, rdx=msg, r8=wParam, r9=lParam
    push rbp
    mov rbp, rsp
    sub rsp, 64
    
    ; 保存参数
    mov [rbp-8], rcx    ; hwnd
    mov [rbp-16], rdx   ; msg
    mov [rbp-24], r8    ; wParam
    mov [rbp-32], r9    ; lParam
    
    ; 检查消息类型
    cmp edx, WM_DESTROY
    je .on_destroy
    
    cmp edx, WM_COMMAND
    je .on_command
    
    cmp edx, WM_CLOSE
    je .on_close
    
    ; 默认处理
    jmp .default
    
.on_destroy:
    ; PostQuitMessage(0)
    xor rcx, rcx
    call PostQuitMessage
    xor rax, rax
    jmp .done
    
.on_close:
    ; DestroyWindow(hwnd)
    mov rcx, [rbp-8]
    call DestroyWindow
    xor rax, rax
    jmp .done
    
.on_command:
    ; 处理控件命令
    ; LOWORD(wParam) = 控件ID, HIWORD(wParam) = 通知码
    mov rax, [rbp-24]   ; wParam
    movzx ecx, ax       ; 控件 ID
    shr rax, 16
    movzx edx, ax       ; 通知码
    
    ; 检查是否是按钮点击
    cmp edx, BN_CLICKED
    jne .default
    
    ; 查找并调用回调函数
    ; 控件ID在1000-1063范围内
    sub ecx, 1000
    cmp ecx, 64
    jae .default
    
    ; 获取回调地址
    lea rax, [callback_array]
    mov rax, [rax + rcx*8]
    test rax, rax
    jz .default
    
    ; 调用回调函数
    push rbp
    call rax
    pop rbp
    
    xor rax, rax
    jmp .done
    
.default:
    ; DefWindowProcA(hwnd, msg, wParam, lParam)
    mov rcx, [rbp-8]
    mov rdx, [rbp-16]
    mov r8, [rbp-24]
    mov r9, [rbp-32]
    call DefWindowProcA
    
.done:
    leave
    ret

; ============================================================
; 创建窗口
; rcx = title, rdx = width, r8 = height
; 返回: rax = hwnd
; ============================================================

global _gui_create_window
_gui_create_window:
    push rbp
    mov rbp, rsp
    sub rsp, 128
    
    ; 保存参数
    mov [rbp-8], rcx    ; title
    mov [rbp-16], rdx   ; width
    mov [rbp-24], r8    ; height
    
    ; CreateWindowExA(
    ;   dwExStyle = 0,
    ;   lpClassName,
    ;   lpWindowName,
    ;   dwStyle = WS_OVERLAPPEDWINDOW | WS_VISIBLE,
    ;   x = 100, y = 100,
    ;   width, height,
    ;   hWndParent = 0,
    ;   hMenu = 0,
    ;   hInstance,
    ;   lpParam = 0
    ; )
    
    xor rcx, rcx                        ; dwExStyle = 0
    lea rdx, [sreyt_class_name]         ; lpClassName
    mov r8, [rbp-8]                     ; lpWindowName (title)
    mov r9d, WS_OVERLAPPEDWINDOW        ; dwStyle
    or r9d, WS_VISIBLE
    
    ; 栈参数
    mov dword [rsp+32], 100             ; x
    mov dword [rsp+40], 100             ; y
    mov rax, [rbp-16]
    mov [rsp+48], eax                   ; width
    mov rax, [rbp-24]
    mov [rsp+56], eax                   ; height
    mov qword [rsp+64], 0               ; hWndParent
    mov qword [rsp+72], 0               ; hMenu
    mov rax, [hInstance]
    mov [rsp+80], rax                   ; hInstance
    mov qword [rsp+88], 0               ; lpParam
    
    call CreateWindowExA
    
    ; 保存主窗口句柄
    mov [main_hwnd], rax
    
    ; 保存到窗口数组
    mov rcx, [hwnd_count]
    lea rdx, [hwnd_array]
    mov [rdx + rcx*8], rax
    inc qword [hwnd_count]
    
    leave
    ret

; ============================================================
; 创建按钮
; rcx = parent_hwnd, rdx = text, r8 = x, r9 = y
; [rsp+32] = width, [rsp+40] = height
; 返回: rax = 控件ID
; ============================================================

global _gui_create_button
_gui_create_button:
    push rbp
    mov rbp, rsp
    sub rsp, 128
    
    ; 保存参数
    mov [rbp-8], rcx    ; parent
    mov [rbp-16], rdx   ; text
    mov [rbp-24], r8    ; x
    mov [rbp-32], r9    ; y
    
    ; 获取宽高
    mov rax, [rbp+48]   ; width
    mov [rbp-40], rax
    mov rax, [rbp+56]   ; height
    mov [rbp-48], rax
    
    ; 获取下一个控件ID
    mov eax, [next_ctrl_id]
    mov [rbp-56], eax
    inc dword [next_ctrl_id]
    
    ; CreateWindowExA
    xor rcx, rcx                        ; dwExStyle
    lea rdx, [class_button]             ; lpClassName = "BUTTON"
    mov r8, [rbp-16]                    ; lpWindowName (text)
    mov r9d, WS_CHILD                   ; dwStyle
    or r9d, WS_VISIBLE
    or r9d, WS_TABSTOP
    or r9d, BS_PUSHBUTTON
    
    mov eax, [rbp-24]
    mov [rsp+32], eax                   ; x
    mov eax, [rbp-32]
    mov [rsp+40], eax                   ; y
    mov rax, [rbp-40]
    mov [rsp+48], eax                   ; width
    mov rax, [rbp-48]
    mov [rsp+56], eax                   ; height
    mov rax, [rbp-8]
    mov [rsp+64], rax                   ; hWndParent
    mov eax, [rbp-56]
    mov [rsp+72], rax                   ; hMenu = 控件ID
    mov rax, [hInstance]
    mov [rsp+80], rax                   ; hInstance
    mov qword [rsp+88], 0               ; lpParam
    
    call CreateWindowExA
    
    ; 保存句柄
    mov rcx, [hwnd_count]
    lea rdx, [hwnd_array]
    mov [rdx + rcx*8], rax
    inc qword [hwnd_count]
    
    ; 返回控件ID
    mov eax, [rbp-56]
    
    leave
    ret

; ============================================================
; 创建文本标签
; rcx = parent_hwnd, rdx = text, r8 = x, r9 = y
; [rsp+32] = width, [rsp+40] = height
; 返回: rax = 控件ID
; ============================================================

global _gui_create_label
_gui_create_label:
    push rbp
    mov rbp, rsp
    sub rsp, 128
    
    mov [rbp-8], rcx    ; parent
    mov [rbp-16], rdx   ; text
    mov [rbp-24], r8    ; x
    mov [rbp-32], r9    ; y
    mov rax, [rbp+48]
    mov [rbp-40], rax   ; width
    mov rax, [rbp+56]
    mov [rbp-48], rax   ; height
    
    mov eax, [next_ctrl_id]
    mov [rbp-56], eax
    inc dword [next_ctrl_id]
    
    ; CreateWindowExA for STATIC
    xor rcx, rcx
    lea rdx, [class_static]
    mov r8, [rbp-16]
    mov r9d, WS_CHILD
    or r9d, WS_VISIBLE
    or r9d, SS_LEFT
    
    mov eax, [rbp-24]
    mov [rsp+32], eax
    mov eax, [rbp-32]
    mov [rsp+40], eax
    mov rax, [rbp-40]
    mov [rsp+48], eax
    mov rax, [rbp-48]
    mov [rsp+56], eax
    mov rax, [rbp-8]
    mov [rsp+64], rax
    mov eax, [rbp-56]
    mov [rsp+72], rax
    mov rax, [hInstance]
    mov [rsp+80], rax
    mov qword [rsp+88], 0
    
    call CreateWindowExA
    
    mov rcx, [hwnd_count]
    lea rdx, [hwnd_array]
    mov [rdx + rcx*8], rax
    inc qword [hwnd_count]
    
    mov eax, [rbp-56]
    leave
    ret

; ============================================================
; 创建文本输入框
; rcx = parent_hwnd, rdx = x, r8 = y, r9 = width
; [rsp+32] = height
; 返回: rax = 控件ID
; ============================================================

global _gui_create_entry
_gui_create_entry:
    push rbp
    mov rbp, rsp
    sub rsp, 128
    
    mov [rbp-8], rcx    ; parent
    mov [rbp-16], rdx   ; x
    mov [rbp-24], r8    ; y
    mov [rbp-32], r9    ; width
    mov rax, [rbp+48]
    mov [rbp-40], rax   ; height
    
    mov eax, [next_ctrl_id]
    mov [rbp-56], eax
    inc dword [next_ctrl_id]
    
    ; CreateWindowExA for EDIT
    mov rcx, 0x200                      ; WS_EX_CLIENTEDGE
    lea rdx, [class_edit]
    lea r8, [empty_string]
    mov r9d, WS_CHILD
    or r9d, WS_VISIBLE
    or r9d, WS_BORDER
    or r9d, WS_TABSTOP
    or r9d, ES_LEFT
    or r9d, ES_AUTOHSCROLL
    
    mov eax, [rbp-16]
    mov [rsp+32], eax
    mov eax, [rbp-24]
    mov [rsp+40], eax
    mov rax, [rbp-32]
    mov [rsp+48], eax
    mov rax, [rbp-40]
    mov [rsp+56], eax
    mov rax, [rbp-8]
    mov [rsp+64], rax
    mov eax, [rbp-56]
    mov [rsp+72], rax
    mov rax, [hInstance]
    mov [rsp+80], rax
    mov qword [rsp+88], 0
    
    call CreateWindowExA
    
    ; 保存句柄到数组（用控件ID作为索引）
    mov ecx, [rbp-56]
    sub ecx, 1000
    lea rdx, [hwnd_array]
    mov [rdx + rcx*8], rax
    
    inc qword [hwnd_count]
    
    mov eax, [rbp-56]
    leave
    ret

; ============================================================
; 设置控件文本
; rcx = 控件ID, rdx = text
; ============================================================

global _gui_set_text
_gui_set_text:
    push rbp
    mov rbp, rsp
    sub rsp, 48
    
    ; 获取控件句柄
    sub ecx, 1000
    lea rax, [hwnd_array]
    mov rcx, [rax + rcx*8]
    
    ; SetWindowTextA(hwnd, text)
    call SetWindowTextA
    
    leave
    ret

; ============================================================
; 获取控件文本
; rcx = 控件ID, rdx = buffer, r8 = buffer_size
; 返回: rax = 文本长度
; ============================================================

global _gui_get_text
_gui_get_text:
    push rbp
    mov rbp, rsp
    sub rsp, 48
    
    mov [rbp-8], rdx    ; buffer
    mov [rbp-16], r8    ; size
    
    ; 获取控件句柄
    sub ecx, 1000
    lea rax, [hwnd_array]
    mov rcx, [rax + rcx*8]
    mov rdx, [rbp-16]   ; size
    mov r8, [rbp-8]     ; buffer
    
    ; GetWindowTextA(hwnd, buffer, size)
    call GetWindowTextA
    
    leave
    ret

; ============================================================
; 注册按钮点击回调
; rcx = 控件ID, rdx = callback_address
; ============================================================

global _gui_on_click
_gui_on_click:
    push rbp
    mov rbp, rsp
    
    ; 存储回调地址
    sub ecx, 1000
    lea rax, [callback_array]
    mov [rax + rcx*8], rdx
    
    leave
    ret

; ============================================================
; 消息循环
; ============================================================

global _gui_message_loop
_gui_message_loop:
    push rbp
    mov rbp, rsp
    sub rsp, 64
    
.loop:
    ; GetMessageA(&msg, NULL, 0, 0)
    lea rcx, [msg]
    xor rdx, rdx
    xor r8, r8
    xor r9, r9
    call GetMessageA
    
    ; 如果返回 0 或 -1，退出循环
    test eax, eax
    jle .done
    
    ; TranslateMessage(&msg)
    lea rcx, [msg]
    call TranslateMessage
    
    ; DispatchMessageA(&msg)
    lea rcx, [msg]
    call DispatchMessageA
    
    jmp .loop
    
.done:
    leave
    ret

; ============================================================
; 显示消息框
; rcx = text, rdx = title
; ============================================================

global _gui_msgbox
_gui_msgbox:
    push rbp
    mov rbp, rsp
    sub rsp, 48
    
    ; MessageBoxA(NULL, text, title, MB_OK)
    mov r8, rdx         ; title
    mov rdx, rcx        ; text
    xor rcx, rcx        ; hWnd = NULL
    xor r9, r9          ; MB_OK = 0
    call MessageBoxA
    
    leave
    ret

; ============================================================
; 显示窗口
; rcx = hwnd 或 0 表示主窗口
; ============================================================

global _gui_show_window
_gui_show_window:
    push rbp
    mov rbp, rsp
    sub rsp, 32
    
    test rcx, rcx
    jnz .use_hwnd
    mov rcx, [main_hwnd]
.use_hwnd:
    mov rdx, SW_SHOW
    call ShowWindow
    
    leave
    ret

; ============================================================
; 获取主窗口句柄
; ============================================================

global _gui_get_main_window
_gui_get_main_window:
    mov rax, [main_hwnd]
    ret
