#pragma once

#include <cstddef>
#include <functional>
#include <iterator>
#include <memory>

namespace starry {
namespace stdlib {

/**
 * @brief 动态数组类模板
 * 
 * 提供类似于std::vector的功能，但针对Starry语言进行了优化
 */
template<typename T>
class Array {
public:
    using value_type = T;
    using size_type = size_t;
    using reference = T&;
    using const_reference = const T&;
    using pointer = T*;
    using const_pointer = const T*;

    /**
     * @brief 默认构造函数
     */
    Array();

    /**
     * @brief 带初始容量的构造函数
     * @param initialCapacity 初始容量
     */
    explicit Array(size_t initialCapacity);

    /**
     * @brief 拷贝构造函数
     * @param other 要拷贝的数组
     */
    Array(const Array& other);

    /**
     * @brief 移动构造函数
     * @param other 要移动的数组
     */
    Array(Array&& other) noexcept;

    /**
     * @brief 拷贝赋值操作符
     * @param other 要拷贝的数组
     * @return 当前数组的引用
     */
    Array& operator=(const Array& other);

    /**
     * @brief 移动赋值操作符
     * @param other 要移动的数组
     * @return 当前数组的引用
     */
    Array& operator=(Array&& other) noexcept;

    /**
     * @brief 析构函数
     */
    ~Array();

    /**
     * @brief 在数组末尾添加元素
     * @param value 要添加的元素
     */
    void push(const T& value);

    /**
     * @brief 移除并返回数组末尾的元素
     * @return 末尾元素
     */
    T pop();

    /**
     * @brief 获取指定索引的元素
     * @param index 索引
     * @return 元素的引用
     */
    T& get(size_t index);
    const T& get(size_t index) const;

    /**
     * @brief 设置指定索引的元素
     * @param index 索引
     * @param value 新值
     */
    void set(size_t index, const T& value);

    /**
     * @brief 在指定位置插入元素
     * @param index 插入位置
     * @param value 要插入的元素
     */
    void insert(size_t index, const T& value);

    /**
     * @brief 移除指定索引的元素
     * @param index 要移除的索引
     */
    void remove(size_t index);

    /**
     * @brief 清空数组
     */
    void clear();

    /**
     * @brief 获取数组大小
     * @return 数组大小
     */
    size_t size() const { return size_; }

    /**
     * @brief 获取数组容量
     * @return 数组容量
     */
    size_t capacity() const { return capacity_; }

    /**
     * @brief 检查数组是否为空
     * @return 为空返回true，否则返回false
     */
    bool empty() const { return size_ == 0; }

    /**
     * @brief 重新调整数组大小
     * @param newCapacity 新容量
     */
    void resize(size_t newCapacity);

    /**
     * @brief 查找元素
     * @param value 要查找的值
     * @return 元素索引，未找到返回-1
     */
    int indexOf(const T& value) const;

    /**
     * @brief 检查是否包含指定元素
     * @param value 要检查的值
     * @return 包含返回true，否则返回false
     */
    bool contains(const T& value) const;

private:
    T* data_;           ///< 数据指针
    size_t size_;       ///< 当前大小
    size_t capacity_;   ///< 容量
};

/**
 * @brief 链表节点
 */
template<typename T>
struct ListNode {
    T data;
    std::shared_ptr<ListNode<T>> next;
    std::weak_ptr<ListNode<T>> prev;

    ListNode(const T& value) : data(value) {}
};

/**
 * @brief 双向链表类模板
 */
template<typename T>
class LinkedList {
public:
    using value_type = T;
    using size_type = size_t;
    using reference = T&;
    using const_reference = const T&;

    /**
     * @brief 默认构造函数
     */
    LinkedList();

    /**
     * @brief 拷贝构造函数
     * @param other 要拷贝的链表
     */
    LinkedList(const LinkedList& other);

    /**
     * @brief 移动构造函数
     * @param other 要移动的链表
     */
    LinkedList(LinkedList&& other) noexcept;

    /**
     * @brief 拷贝赋值操作符
     * @param other 要拷贝的链表
     * @return 当前链表的引用
     */
    LinkedList& operator=(const LinkedList& other);

    /**
     * @brief 移动赋值操作符
     * @param other 要移动的链表
     * @return 当前链表的引用
     */
    LinkedList& operator=(LinkedList&& other) noexcept;

    /**
     * @brief 析构函数
     */
    ~LinkedList();

    /**
     * @brief 在链表头部添加元素
     * @param value 要添加的元素
     */
    void pushFront(const T& value);

    /**
     * @brief 在链表尾部添加元素
     * @param value 要添加的元素
     */
    void pushBack(const T& value);

    /**
     * @brief 移除并返回链表头部元素
     * @return 头部元素
     */
    T popFront();

    /**
     * @brief 移除并返回链表尾部元素
     * @return 尾部元素
     */
    T popBack();

    /**
     * @brief 获取链表头部元素
     * @return 头部元素的引用
     */
    T& front();
    const T& front() const;

    /**
     * @brief 获取链表尾部元素
     * @return 尾部元素的引用
     */
    T& back();
    const T& back() const;

    /**
     * @brief 获取链表大小
     * @return 链表大小
     */
    size_t size() const { return size_; }

    /**
     * @brief 检查链表是否为空
     * @return 为空返回true，否则返回false
     */
    bool empty() const { return size_ == 0; }

    /**
     * @brief 清空链表
     */
    void clear();

    /**
     * @brief 查找元素
     * @param value 要查找的值
     * @return 找到返回节点指针，否则返回nullptr
     */
    std::shared_ptr<ListNode<T>> find(const T& value) const;

    /**
     * @brief 移除指定值的元素
     * @param value 要移除的值
     * @return 成功移除返回true，否则返回false
     */
    bool remove(const T& value);

private:
    std::shared_ptr<ListNode<T>> head_;  ///< 头节点
    std::shared_ptr<ListNode<T>> tail_;  ///< 尾节点
    size_t size_;                        ///< 链表大小
};

/**
 * @brief 哈希表类模板
 */
template<typename K, typename V>
class HashMap {
public:
    using key_type = K;
    using mapped_type = V;
    using value_type = std::pair<K, V>;
    using size_type = size_t;

    /**
     * @brief 默认构造函数
     */
    HashMap();

    /**
     * @brief 带初始容量的构造函数
     * @param initialCapacity 初始容量
     */
    explicit HashMap(size_t initialCapacity);

    /**
     * @brief 析构函数
     */
    ~HashMap();

    /**
     * @brief 插入或更新键值对
     * @param key 键
     * @param value 值
     */
    void put(const K& key, const V& value);

    /**
     * @brief 获取指定键的值
     * @param key 键
     * @return 值的引用
     */
    V& get(const K& key);
    const V& get(const K& key) const;

    /**
     * @brief 检查是否包含指定键
     * @param key 键
     * @return 包含返回true，否则返回false
     */
    bool contains(const K& key) const;

    /**
     * @brief 移除指定键的键值对
     * @param key 键
     * @return 成功移除返回true，否则返回false
     */
    bool remove(const K& key);

    /**
     * @brief 获取哈希表大小
     * @return 键值对数量
     */
    size_t size() const { return size_; }

    /**
     * @brief 检查哈希表是否为空
     * @return 为空返回true，否则返回false
     */
    bool empty() const { return size_ == 0; }

    /**
     * @brief 清空哈希表
     */
    void clear();

    /**
     * @brief 获取负载因子
     * @return 负载因子
     */
    double loadFactor() const;

private:
    struct Entry {
        K key;
        V value;
        std::shared_ptr<Entry> next;

        Entry(const K& k, const V& v) : key(k), value(v) {}
    };

    void rehash();
    size_t hash(const K& key) const;
    std::shared_ptr<Entry> findEntry(const K& key) const;

private:
    std::vector<std::shared_ptr<Entry>> buckets_;  ///< 桶数组
    size_t size_;                                  ///< 键值对数量
    size_t capacity_;                              ///< 桶容量
    static constexpr double MAX_LOAD_FACTOR = 0.75;
};

/**
 * @brief 集合类模板
 */
template<typename T>
class Set {
public:
    using value_type = T;
    using size_type = size_t;

    /**
     * @brief 默认构造函数
     */
    Set();

    /**
     * @brief 带初始容量的构造函数
     * @param initialCapacity 初始容量
     */
    explicit Set(size_t initialCapacity);

    /**
     * @brief 析构函数
     */
    ~Set();

    /**
     * @brief 添加元素
     * @param value 要添加的元素
     * @return 成功添加返回true，元素已存在返回false
     */
    bool add(const T& value);

    /**
     * @brief 移除元素
     * @param value 要移除的元素
     * @return 成功移除返回true，元素不存在返回false
     */
    bool remove(const T& value);

    /**
     * @brief 检查是否包含指定元素
     * @param value 要检查的元素
     * @return 包含返回true，否则返回false
     */
    bool contains(const T& value) const;

    /**
     * @brief 获取集合大小
     * @return 元素数量
     */
    size_t size() const { return map_.size(); }

    /**
     * @brief 检查集合是否为空
     * @return 为空返回true，否则返回false
     */
    bool empty() const { return map_.empty(); }

    /**
     * @brief 清空集合
     */
    void clear();

    /**
     * @brief 集合并集
     * @param other 另一个集合
     * @return 并集结果
     */
    Set<T> unionWith(const Set<T>& other) const;

    /**
     * @brief 集合交集
     * @param other 另一个集合
     * @return 交集结果
     */
    Set<T> intersectionWith(const Set<T>& other) const;

    /**
     * @brief 集合差集
     * @param other 另一个集合
     * @return 差集结果
     */
    Set<T> differenceWith(const Set<T>& other) const;

private:
    HashMap<T, bool> map_;  ///< 使用哈希表实现集合
};

/**
 * @brief 栈类模板
 */
template<typename T>
class Stack {
public:
    using value_type = T;
    using size_type = size_t;

    /**
     * @brief 默认构造函数
     */
    Stack();

    /**
     * @brief 析构函数
     */
    ~Stack();

    /**
     * @brief 压栈
     * @param value 要压入的元素
     */
    void push(const T& value);

    /**
     * @brief 弹栈
     * @return 栈顶元素
     */
    T pop();

    /**
     * @brief 获取栈顶元素
     * @return 栈顶元素的引用
     */
    T& top();
    const T& top() const;

    /**
     * @brief 获取栈大小
     * @return 栈大小
     */
    size_t size() const { return list_.size(); }

    /**
     * @brief 检查栈是否为空
     * @return 为空返回true，否则返回false
     */
    bool empty() const { return list_.empty(); }

    /**
     * @brief 清空栈
     */
    void clear();

private:
    LinkedList<T> list_;  ///< 使用链表实现栈
};

/**
 * @brief 队列类模板
 */
template<typename T>
class Queue {
public:
    using value_type = T;
    using size_type = size_t;

    /**
     * @brief 默认构造函数
     */
    Queue();

    /**
     * @brief 析构函数
     */
    ~Queue();

    /**
     * @brief 入队
     * @param value 要入队的元素
     */
    void enqueue(const T& value);

    /**
     * @brief 出队
     * @return 队首元素
     */
    T dequeue();

    /**
     * @brief 获取队首元素
     * @return 队首元素的引用
     */
    T& front();
    const T& front() const;

    /**
     * @brief 获取队尾元素
     * @return 队尾元素的引用
     */
    T& back();
    const T& back() const;

    /**
     * @brief 获取队列大小
     * @return 队列大小
     */
    size_t size() const { return list_.size(); }

    /**
     * @brief 检查队列是否为空
     * @return 为空返回true，否则返回false
     */
    bool empty() const { return list_.empty(); }

    /**
     * @brief 清空队列
     */
    void clear();

private:
    LinkedList<T> list_;  ///< 使用链表实现队列
};

} // namespace stdlib
} // namespace starry