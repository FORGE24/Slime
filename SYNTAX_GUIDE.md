# Slime 语言语法指南

## 注释语法

Slime语言使用 `//` 作为单行注释标记。

```sme
// 这是一个单行注释
var x = 10 // 这也是一个注释
```

**注意：** 以 `#` 开头的标记不是注释，而是特殊指令（如 `#v`、`#n`、`#a`）。

## 常用语法示例

### 变量声明与赋值
```sme
// 变量声明与赋值
var x = 10
var y = "hello"
var z = true
```

### 函数创建与调用
```sme
// 创建函数
cra main{
    // 函数体
    use print "Hello World"
}

// 调用函数
main()

// 删除函数
del main
```

### 条件语句
```sme
// 条件语句
var x = 10
if x > 5 {
    use print "x 大于 5"
} else {
    use print "x 小于等于 5"
}
```

### 循环语句
```sme
// For 循环
for (var i = 0; i < 5; i = i + 1) {
    use print i
}

// While 循环
var j = 0
while j < 5 {
    use print j
    j = j + 1
}
```

### 数组与哈希表
```sme
// 数组
var arr = [1, 2, 3, 4, 5]
arr.push(6)

// 哈希表
var obj = { name: "Slime", version: "1.0" }
obj.author = "开发者"
```

### 特殊指令

Slime语言中，以 `#` 开头的标记是特殊指令，不是注释：

```sme
// 预执行库引入指令
#v introduce "baseline.smeh"

// 函数注册指令
#v register "print" to "System.Output.Print"

// 常量定义指令
#n define "PI" 3.14159

// 配置设置指令
#a set "strict_mode" ON
```

## 常见错误

错误的注释语法：
```sme
# 这是错误的注释方式
```

正确的注释语法：
```sme
// 这是正确的注释方式
```

## 总结

- 使用 `//` 进行单行注释
- 使用 `#` 开头的标记是特殊指令，不是注释
- 遵循一致的缩进和代码风格，提高可读性