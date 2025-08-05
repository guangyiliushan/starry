// Starry语言示例 - Hello World

fun main() {
    // 打印欢迎信息
    println("Hello, Starry World!");
    
    // 变量声明示例
    var count = 10;
    val PI = 3.14159;
    
    // 条件语句示例
    if (count > 5) {
        println("计数大于5");
    } else {
        println("计数小于或等于5");
    }
    
    // 循环示例
    for (var i = 0; i < count; i++) {
        println("循环计数: " + i);
    }
    
    // 函数调用示例
    var result = calculateSum(5, 10);
    println("5 + 10 = " + result);
    
    // 空安全示例
    var name: str? = null;
    println("名字长度: " + (name?.length ?: 0));
    
    // 类型检查示例
    var value: any = 42;
    if (value is int) {
        println("值是整数类型");
    }
}

// 函数定义示例
fun calculateSum(a: int, b: int): int {
    return a + b;
}

// 类定义示例
class Person {
    // 属性
    val name: str;
    var age: int;
    
    // 构造函数
    constructor(name: str, age: int) {
        this.name = name;
        this.age = age;
    }
    
    // 方法
    fun introduce(): str {
        return "我是 " + name + "，今年 " + age + " 岁";
    }
    
    // 静态成员
    companion object {
        val ADULT_AGE = 18;
        
        fun isAdult(age: int): bool {
            return age >= ADULT_AGE;
        }
    }
}