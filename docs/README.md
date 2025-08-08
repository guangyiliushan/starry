# Starry 编程语言设计规范

## 一、概述

Starry 是一种基于 C/C++ 语言衍生的现代多范式编程语言，深度融合面向对象、泛型编程及函数式编程等理论范式。该语言设计遵循简约性、高效性与可维护性原则，其核心技术特征如下：

- **C/C++完全兼容**：保持与C/C++语法的高度一致性，支持直接调用C/C++库函数
- **静态类型系统**：采用编译期类型推导机制，通过严格的类型检查体系保障程序运行安全
- **跨平台兼容性**：原生支持 Windows、Linux、macOS 等主流操作系统及嵌入式系统
- **高性能执行效率**：生成的二进制文件体积小，运行效率高
- **开放生态体系**：通过标准化插件接口规范，支持第三方工具与框架的无缝集成
- **多范式融合架构**：提供结构化编程、面向对象编程、函数式编程及响应式编程等多范式统一编程模型
- **Kotlin风格语法糖**：在保持C/C++兼容性的基础上，提供更简洁优雅的语法特性

## 二、语言核心特性

### 2.1 关键字系统

#### 2.1.1 关键字总览

Starry 语言采用统一的关键字系统，消除了软硬关键字的区分，使其与C/C++保持高度一致性：

| 类别 | 关键字 | 描述 |
| --- | --- | --- |
| 类型定义 | `class`, `struct`, `enum`, `union`, `typedef`, `using` | 与C/C++完全一致的类型定义关键字 |
| 控制流 | `if`, `else`, `switch`, `case`, `default`, `for`, `while`, `do`, `break`, `continue`, `return`, `goto` | 与C/C++完全一致的控制流关键字 |
| 异常处理 | `try`, `catch`, `throw` | 与C++一致的异常处理机制 |
| 类型修饰 | `const`, `volatile`, `static`, `extern`, `inline`, `virtual`, `explicit`, `friend`, `mutable` | 与C/C++一致的类型修饰符 |
| 访问控制 | `public`, `private`, `protected` | 与C++一致的访问控制关键字 |
| 内存管理 | `new`, `delete`, `sizeof` | 与C++一致的内存管理关键字 |
| 模板 | `template`, `typename` | 与C++一致的模板关键字 |
| 命名空间 | `namespace`, `using` | 与C++一致的命名空间关键字 |
| 布尔值 | `true`, `false` | 与C++一致的布尔值常量 |
| 空值 | `null` | 统一的空值表示（替代C++的nullptr） |
| 变量声明 | `var`, `val` | Kotlin风格的变量声明关键字 |
| 类型检查 | `is`, `as`, `as?` | Kotlin风格的类型检查与转换 |
| 扩展函数 | `extension` | 支持Kotlin风格的扩展函数 |
| 空安全 | `?`, `?.`, `?:`, `!!` | Kotlin风格的空安全操作符 |

> **说明：**
> - 所有C/C++关键字保持原有语义，确保代码迁移的最小修改量
> - Kotlin风格关键字作为扩展，在不影响C/C++兼容性的前提下提供更优雅的编程体验
> - 统一使用`null`表示所有空值或空指针，提高代码一致性

### 2.2 运算符系统

| 类别          | 运算符                          | 描述                                                     |
| ------------- | ------------------------------- | -------------------------------------------------------- |
| 数学运算      | `+`, `-`, `*`, `/`, `%`         | 基础数学运算（加减乘除取余）                             |
| 复合赋值      | `+=`, `-=`, `*=`, `/=`, `%=`    | 复合赋值运算                                             |
| 自增自减      | `++`, `--`                      | 递增递减运算                                             |
| 逻辑运算      | `&&`, `\|\|`, `!`               | 逻辑运算（与/或/非）                                     |
| 相等比较      | `==`, `!=`, `===`, `!==`        | 相等性比较（值相等/引用相等）                            |
| 大小比较      | `<`, `>`, `<=`, `>=`            | 比较运算                                                 |
| 位运算        | `&`, `\|`, `^`, `~`             | 位运算（与/或/异或/非）                                  |
| 位赋值        | `&=`, `\|=`, `^=`, `<<=`, `>>=` | 位运算复合赋值                                           |
| 位移运算      | `<<`, `>>`                      | 位移运算                                                 |
| 空安全        | `?.`, `?:`, `!!`                | 空安全操作（安全调用/Elvis/非空断言）                    |
| 区间操作      | `..`, `..<`, `..=`              | 区间操作（半开/闭合区间）                                |
| 指针操作      | `&`, `*`                        | 取址/指针操作                                            |
| 类型/成员访问 | `::`, `.`, `?`                  | 类型/成员访问（类引用/成员访问/可空标记）                |
| 基础操作符    | `=`, `->`, `=>`                 | 基础操作符（赋值/返回类型/模式匹配）                     |
| 辅助符号      | `@`, `:`, `;`, `$`, `_`         | 辅助符号（模式绑定/类型声明/语句结束/模板变量/参数占位） |
| 类型操作      | `is`, `!is`, `as`, `as?`        | 类型检查与转换操作                                       |

### 2.3 数据类型

#### 2.3.1 基本类型

| C/C++类型    | Starry类型 | 描述             | 示例                                   |
| ------------ | ---------- | ---------------- | -------------------------------------- |
| char         | i8         | 8位有符号整数    | `var byte: i8 = 42`                    |
| short        | i16        | 16位有符号整数   | `var small: i16 = 1000`                |
| int32_t      | i32        | 32位有符号整数   | `var num: i32 = -42`                   |
| int          | int        | 平台相关整数类型 | `var count: int = 10`                  |
| int64_t      | i64        | 64位有符号整数   | `var big: i64 = 9000000`               |
| __int128     | i128       | 128位有符号整数  | `var huge: i128 = -1234567890123456789` |
| unsigned char | u8         | 8位无符号整数    | `var byte: u8 = 255`                   |
| unsigned short | u16        | 16位无符号整数   | `var small: u16 = 65535`               |
| unsigned int   | u32        | 32位无符号整数   | `var num: u32 = 4294967295`            |
| unsigned long  | u64        | 64位无符号整数   | `var big: u64 = 18446744073709551615`  |
| __uint128    | u128        | 128位无符号整数   | `var big: u128 = 9223372036854775807`   |
| float        | f32        | 32位浮点数       | `var ratio: f32 = 0.5`                 |
| double       | f64/float  | 64位浮点数       | `var pi: f64 = 3.14159`                |
| void         | void       | 无返回值类型     | `fun process(): void {}`               |
| bool         | bool       | 布尔类型         | `var flag: bool = true`                |
| std::string  | str        | 字符串类型       | `var name: str = "Starry"`             |
| auto         | any        | 动态类型         | `var data: any = 42`                   |
| nullptr_t    | null       | 空值类型         | `var ptr: i32? = null`                 |

#### 2.3.2 复合类型

| 类型     | Starry语法                              | 描述                   |
| -------- | --------------------------------------- | ---------------------- |
| 元组     | `var coords: (f64, f64) = (35.6, 139.6)` | 多值组合               |
| 固定数组 | `var matrix: [i32;4] = [1, 2, 3, 4]`     | 固定长度的同类元素集合 |
| 指针     | `var str: raw* i8`                       | 原始指针               |
| 引用     | `var a: & const int`                     | 引用类型               |
| 字符串   | `var greeting: str = "Hello, Starry!"`   | Unicode 字符序列       |
| 切片     | `var slice: slice(i32) = matrix[1..3]`   | 动态视图               |
| 可空类型 | `var name: str? = null`                  | 可为空的类型           |

### 2.3.3 变量声明方式

Starry支持Kotlin风格的变量声明语法，使代码更简洁明了：

```kotlin
// 可变变量（类似C++中的普通变量）
var count = 10
var name: str = "Starry"

// 不可变变量（类似C++中的const变量）
val PI = 3.14159
val MAX_COUNT: i32 = 100

// 显式类型声明
var users: List<User> = getUsers()

// 可空类型
var optionalData: Data? = null
```

### 2.3.4 函数定义方式

Starry采用Kotlin风格的函数定义语法，更加简洁直观：

```kotlin
// 基本函数定义
fun add(a: int, b: int): int {
    return a + b
}

// 单表达式函数
fun multiply(a: int, b: int): int = a * b

// 泛型函数
fun <T> max(a: T, b: T): T where T: Comparable<T> {
    return if (a > b) a else b
}

// 扩展函数
extension fun str.isPalindrome(): bool {
    val reversed = this.reversed()
    return this == reversed
}
```

## 三、类型系统

### 3.1 混合类型系统

- 静态强类型：编译期类型检查确保安全
- 动态强类型：运行时类型检查支持
- 类型推断：基于Flow算法实现自动推导

### 3.2 泛型系统

- 支持协变与逆变类型参数
- 提供`where`子句实现泛型约束
- 支持高阶泛型（Higher-Kinded Types）编程

### 3.3 特殊类型

| 类型       | 符号    | 描述                                   |
| ---------- | ------- | -------------------------------------- |
| 可空类型   | `T?`    | 显式声明可为null的类型                 |
| 空值       | `null`  | 统一的空引用表示，适用于可空类型       |

### 3.4 类型交互与转换

```kotlin
// 类型检查
if (value is str) {
    // 在这个作用域中，value被智能转换为str类型
    println(value.length)
}

// 安全类型转换
val text = value as? str
if (text != null) {
    println(text.length)
}

// 非空断言
val nonNullValue = nullableValue!!  // 如果为null则抛出异常
```

## 四、内存管理

### 4.1 Starry安全模型

- 区域内存管理：对象生命周期绑定区域
- 借用检查机制：基于区域的引用权限控制
- 线性类型系统：确保资源精确释放与回收

### 4.2 分层内存架构

| 层级         | 策略                 | 适用场景     |
| ------------ | -------------------- | ------------ |
| 静态类型     | 区域内存管理         | 长期存活对象 |
| 动态小对象   | 栈内联存储（≤64 位） | 临时变量     |
| 动态大对象   | 引用计数 + 循环检测  | 复杂数据结构 |
| 长期动态对象 | 引用计数-清除         | 全局缓存     |

### 4.3 内存安全机制

```kotlin
// 区域内存管理
@region {
    val data = ByteArray(1024)
    processData(data)
} // 区域结束时自动释放所有资源

// 资源安全管理
use(File("data.txt")) { file ->
    val content = file.readText()
    processContent(content)
} // 文件自动关闭，即使发生异常

// 空安全
val name: str? = getName()
val greeting = "Hello, ${name ?: "Guest"}"
```

## 五、编程范式支持

### 5.1 面向对象编程

```kotlin
// 类定义
class Person {
    // 属性
    val name: str
    var age: int
    
    // 构造函数
    constructor(name: str, age: int) {
        this.name = name
        this.age = age
    }
    
    // 成员函数
    fun introduce(): str {
        return "I'm $name, $age years old"
    }
    
    // 静态成员
    companion object {
        val ADULT_AGE = 18
        
        fun isAdult(age: int): bool {
            return age >= ADULT_AGE
        }
    }
}

// 继承
class Employee : Person {
    val position: str
    
    constructor(name: str, age: int, position: str) : super(name, age) {
        this.position = position
    }
    
    override fun introduce(): str {
        return "${super.introduce()}, working as $position"
    }
}
```

### 5.2 函数式编程

```kotlin
// Lambda表达式
val sum = { a: int, b: int -> a + b }

// 高阶函数
fun <T, R> List<T>.map(transform: (T) -> R): List<R> {
    val result = mutableListOf<R>()
    for (item in this) {
        result.add(transform(item))
    }
    return result
}

// 使用高阶函数
val numbers = listOf(1, 2, 3, 4, 5)
val doubled = numbers.map { it * 2 }  // [2, 4, 6, 8, 10]

// 函数组合
val addOne = { x: int -> x + 1 }
val multiplyByTwo = { x: int -> x * 2 }
val composed = { x: int -> multiplyByTwo(addOne(x)) }  // (x + 1) * 2
```

### 5.3 响应式编程

```kotlin
// 观察者模式
class Observable<T> {
    private val observers = mutableListOf<(T) -> Unit>()
    
    fun subscribe(observer: (T) -> Unit) {
        observers.add(observer)
    }
    
    fun notify(value: T) {
        for (observer in observers) {
            observer(value)
        }
    }
}

// 使用响应式编程
val temperature = Observable<f32>()
temperature.subscribe { temp ->
    println("Temperature changed: $temp")
}
temperature.notify(25.5f)  // 输出: Temperature changed: 25.5
```

### 5.4 元编程

```kotlin
// 注解
@Target(Class)
annotation class Serializable

@Serializable
class User {
    val name: str = ""
    val age: int = 0
}

// 反射
fun printProperties(obj: any) {
    val properties = reflect.getProperties(obj)
    for (prop in properties) {
        println("${prop.name}: ${prop.getValue(obj)}")
    }
}
```

## 六、并发模型

### 6.1 多级并发架构

```kotlin
// 协程（轻量级线程）
async {
    val result = fetchData()  // 挂起函数
    processResult(result)
}

// 通道通信
val channel = Channel<str>()
launch {
    channel.send("Hello")
    channel.send("World")
}
launch {
    println(channel.receive())  // Hello
    println(channel.receive())  // World
}
```

### 6.2 并发原语

```kotlin
// 互斥锁
val mutex = Mutex()
mutex.withLock {
    // 临界区代码
    sharedResource.modify()
}

// 原子操作
val counter = atomic(0)
counter.incrementAndGet()  // 原子递增
```

## 七、互操作性

### 7.1 C/C++互操作

```kotlin
// 导入C函数
@extern("C")
fun printf(format: raw* i8, ...): i32

// 调用C函数
fun main() {
    printf("Hello, %s!\n".toCString(), "World".toCString())
}

// 导出为C API
@export("C")
fun add(a: int, b: int): int {
    return a + b
}
```

### 7.2 内存布局兼容

```kotlin
// 与C结构体兼容的类
@CCompatible
struct Point {
    var x: f32
    var y: f32
}

// 使用C内存布局
val point = Point(1.0f, 2.0f)
val rawPtr = point.toPointer()  // 可传递给C函数
```

## 八、编译器与工具链

### 8.1 编译器架构

Starry编译器采用多阶段设计，支持增量编译和全程序优化：

1. **前端**：词法分析、语法分析、语义分析
2. **中间表示**：基于LLVM IR的中间代码生成
3. **后端**：目标代码生成、优化、链接

### 8.2 代码转换工具

提供C/C++与Starry代码的双向转换工具：

1. **C++到Starry转换器**：自动将C++代码转换为Starry代码
2. **Starry到C++转换器**：将Starry代码转换回C++代码
3. **兼容性检查器**：检查代码在转换过程中可能出现的问题

## 九、应用场景

- **系统编程**：利用内存安全模型开发操作系统核心组件
- **高性能计算**：通过向量化与并发优化提升计算效率
- **跨平台应用**：实现一次编写多平台部署
- **嵌入式开发**：适合资源受限环境的轻量级部署
- **WebAssembly**：构建高效的前端计算模块

## 十、总结

Starry语言通过创新的设计，实现了与C/C++的完全兼容性，同时借鉴Kotlin的优雅特性，提供了更现代、更安全、更高效的编程体验。其核心优势包括：

1. **无缝C/C++集成**：可直接调用C/C++库函数，支持现有代码资产
2. **最小迁移成本**：C/C++开发者几乎无需学习新关键字，代码修改量极小
3. **现代语言特性**：提供空安全、扩展函数、Lambda表达式等现代特性
4. **多层次内存管理**：支持从手动内存管理到自动垃圾回收的多种模式
5. **改进开发体验**：解决C/C++常见痛点，提高开发效率和代码质量

Starry语言为系统编程、嵌入式开发、游戏开发、高性能计算等领域提供了理想的编程语言选择，既保持了C/C++的高性能和底层控制能力，又提供了现代编程语言的安全性和表达力。