cmake_minimum_required(VERSION 3.10)
project(SlimeLang)

# 设置C++标准
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# 查找所有源文件
file(GLOB SOURCES "*.cpp")

# 移除独立的工具文件
list(FILTER SOURCES EXCLUDE REGEX "slime_benchmark.cpp")
list(FILTER SOURCES EXCLUDE REGEX "test_gc.cpp")

# 创建主可执行文件
add_executable(simple_interpreter ${SOURCES})

# 链接数学库（如果需要）
if(WIN32)
    # Windows可能需要链接额外的库
else()
    target_link_libraries(simple_interpreter m)
endif()

# 编译Rust GC库
if(WIN32)
    # 在Windows上使用cargo构建Rust库
    add_custom_command(
        OUTPUT slime_gc/target/release/slime_gc.dll
        COMMAND cargo build --release
        WORKING_DIRECTORY ${CMAKE_SOURCE_DIR}/slime_gc
        COMMENT "Building Rust GC library..."
    )
    
    # 创建Rust GC目标
    add_custom_target(slime_gc
        DEPENDS slime_gc/target/release/slime_gc.dll
        COMMENT "Ensuring Rust GC library is built"
    )
    
    # 设置Rust库和头文件的路径
    set(RUST_GC_DLL "${CMAKE_SOURCE_DIR}/slime_gc/target/release/slime_gc.dll")
    set(RUST_GC_LIB "${CMAKE_SOURCE_DIR}/slime_gc/target/release/slime_gc.dll.lib")
    
    # 确保Rust库在构建前被编译
    add_dependencies(simple_interpreter slime_gc)
    
    # 链接Rust库
    target_link_libraries(simple_interpreter "${RUST_GC_LIB}")
    
    # 将Rust DLL复制到可执行文件目录
    add_custom_command(
        TARGET simple_interpreter POST_BUILD
        COMMAND ${CMAKE_COMMAND} -E copy_if_different
        "${RUST_GC_DLL}"
        $<TARGET_FILE_DIR:simple_interpreter>
    )
endif()

# 编译基准测试工具
add_executable(slime_benchmark slime_benchmark.cpp)

# 编译垃圾回收测试工具
add_executable(test_gc test_gc.cpp)

# 链接Rust GC库到测试工具
if(WIN32)
    add_dependencies(test_gc slime_gc)
    target_link_libraries(test_gc "${RUST_GC_LIB}")
    
    # 将Rust DLL复制到测试工具目录
    add_custom_command(
        TARGET test_gc POST_BUILD
        COMMAND ${CMAKE_COMMAND} -E copy_if_different
        "${RUST_GC_DLL}"
        $<TARGET_FILE_DIR:test_gc>
    )
endif()

# 设置编译警告
if(MSVC)
    target_compile_options(simple_interpreter PRIVATE /W3)
    target_compile_options(slime_benchmark PRIVATE /W3)
else()
    target_compile_options(simple_interpreter PRIVATE -Wall -Wextra)
    target_compile_options(slime_benchmark PRIVATE -Wall -Wextra)
endif()