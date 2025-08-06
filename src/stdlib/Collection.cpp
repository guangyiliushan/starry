#include "starry/stdlib/Collection.h"
#include <algorithm>
#include <stdexcept>
#include <sstream>

namespace starry {
namespace stdlib {

// 动态数组实现
template<typename T>
Array<T>::Array() : data_(nullptr), size_(0), capacity_(0) {}

template<typename T>
Array<T>::Array(size_t initialCapacity) : size_(0), capacity_(initialCapacity) {
    data_ = new T[capacity_];
}

template<typename T>
Array<T>::Array(const Array& other) : size_(other.size_), capacity_(other.capacity_) {
    data_ = new T[capacity_];
    for (size_t i = 0; i < size_; ++i) {
        data_[i] = other.data_[i];
    }
}

template<typename T>
Array<T>::Array(Array&& other) noexcept 
    : data_(other.data_), size_(other.size_), capacity_(other.capacity_) {
    other.data_ = nullptr;
    other.size_ = 0;
    other.capacity_ = 0;
}

template<typename T>
Array<T>& Array<T>::operator=(const Array& other) {
    if (this != &other) {
        delete[] data_;
        size_ = other.size_;
        capacity_ = other.capacity_;
        data_ = new T[capacity_];
        for (size_t i = 0; i < size_; ++i) {
            data_[i] = other.data_[i];
        }
    }
    return *this;
}

template<typename T>
Array<T>& Array<T>::operator=(Array&& other) noexcept {
    if (this != &other) {
        delete[] data_;
        data_ = other.data_;
        size_ = other.size_;
        capacity_ = other.capacity_;
        other.data_ = nullptr;
        other.size_ = 0;
        other.capacity_ = 0;
    }
    return *this;
}

template<typename T>
Array<T>::~Array() {
    delete[] data_;
}

template<typename T>
void Array<T>::push(const T& value) {
    if (size_ >= capacity_) {
        resize(capacity_ == 0 ? 1 : capacity_ * 2);
    }
    data_[size_++] = value;
}

template<typename T>
T Array<T>::pop() {
    if (size_ == 0) {
        throw std::runtime_error("数组为空，无法弹出元素");
    }
    return data_[--size_];
}

template<typename T>
T& Array<T>::get(size_t index) {
    if (index >= size_) {
        throw std::out_of_range("数组索引越界");
    }
    return data_[index];
}

template<typename T>
const T& Array<T>::get(size_t index) const {
    if (index >= size_) {
        throw std::out_of_range("数组索引越界");
    }
    return data_[index];
}

template<typename T>
void Array<T>::set(size_t index, const T& value) {
    if (index >= size_) {
        throw std::out_of_range("数组索引越界");
    }
    data_[index] = value;
}

template<typename T>
void Array<T>::insert(size_t index, const T& value) {
    if (index > size_) {
        throw std::out_of_range("插入位置越界");
    }
    
    if (size_ >= capacity_) {
        resize(capacity_ == 0 ? 1 : capacity_ * 2);
    }
    
    // 移动元素
    for (size_t i = index; i < size_ - 1; ++i) {
        data_[i] = data_[i + 1];
    }
    --size_;
}

template<typename T>
size_t Array<T>::size() const {
    return size_;
}

template<typename T>
size_t Array<T>::capacity() const {
    return capacity_;
}

template<typename T>
bool Array<T>::empty() const {
    return size_ == 0;
}

template<typename T>
void Array<T>::clear() {
    size_ = 0;
}

template<typename T>
void Array<T>::resize(size_t newCapacity) {
    if (newCapacity <= capacity_) {
        return;
    }
    
    T* newData = new T[newCapacity];
    for (size_t i = 0; i < size_; ++i) {
        newData[i] = data_[i];
    }
    
    delete[] data_;
    data_ = newData;
    capacity_ = newCapacity;
}

template<typename T>
T& Array<T>::operator[](size_t index) {
    return get(index);
}

template<typename T>
const T& Array<T>::operator[](size_t index) const {
    return get(index);
}

// 链表实现
template<typename T>
LinkedList<T>::LinkedList() : head_(nullptr), tail_(nullptr), size_(0) {}

template<typename T>
LinkedList<T>::~LinkedList() {
    clear();
}

template<typename T>
void LinkedList<T>::pushFront(const T& value) {
    Node* newNode = new Node{value, head_, nullptr};
    
    if (head_) {
        head_->prev = newNode;
    } else {
        tail_ = newNode;
    }
    
    head_ = newNode;
    ++size_;
}

template<typename T>
void LinkedList<T>::pushBack(const T& value) {
    Node* newNode = new Node{value, nullptr, tail_};
    
    if (tail_) {
        tail_->next = newNode;
    } else {
        head_ = newNode;
    }
    
    tail_ = newNode;
    ++size_;
}

template<typename T>
T LinkedList<T>::popFront() {
    if (!head_) {
        throw std::runtime_error("链表为空，无法弹出元素");
    }
    
    T value = head_->data;
    Node* oldHead = head_;
    head_ = head_->next;
    
    if (head_) {
        head_->prev = nullptr;
    } else {
        tail_ = nullptr;
    }
    
    delete oldHead;
    --size_;
    return value;
}

template<typename T>
T LinkedList<T>::popBack() {
    if (!tail_) {
        throw std::runtime_error("链表为空，无法弹出元素");
    }
    
    T value = tail_->data;
    Node* oldTail = tail_;
    tail_ = tail_->prev;
    
    if (tail_) {
        tail_->next = nullptr;
    } else {
        head_ = nullptr;
    }
    
    delete oldTail;
    --size_;
    return value;
}

template<typename T>
size_t LinkedList<T>::size() const {
    return size_;
}

template<typename T>
bool LinkedList<T>::empty() const {
    return size_ == 0;
}

template<typename T>
void LinkedList<T>::clear() {
    while (head_) {
        Node* next = head_->next;
        delete head_;
        head_ = next;
    }
    tail_ = nullptr;
    size_ = 0;
}

// 哈希表实现
template<typename K, typename V>
HashMap<K, V>::HashMap(size_t initialCapacity) 
    : buckets_(initialCapacity), size_(0), capacity_(initialCapacity) {}

template<typename K, typename V>
HashMap<K, V>::~HashMap() = default;

template<typename K, typename V>
void HashMap<K, V>::put(const K& key, const V& value) {
    size_t index = hash(key) % capacity_;
    
    // 查找是否已存在
    for (auto& pair : buckets_[index]) {
        if (pair.first == key) {
            pair.second = value;
            return;
        }
    }
    
    // 添加新键值对
    buckets_[index].emplace_back(key, value);
    ++size_;
    
    // 检查是否需要扩容
    if (size_ > capacity_ * 0.75) {
        rehash();
    }
}

template<typename K, typename V>
V HashMap<K, V>::get(const K& key) const {
    size_t index = hash(key) % capacity_;
    
    for (const auto& pair : buckets_[index]) {
        if (pair.first == key) {
            return pair.second;
        }
    }
    
    throw std::runtime_error("键不存在");
}

template<typename K, typename V>
bool HashMap<K, V>::contains(const K& key) const {
    size_t index = hash(key) % capacity_;
    
    for (const auto& pair : buckets_[index]) {
        if (pair.first == key) {
            return true;
        }
    }
    
    return false;
}

template<typename K, typename V>
void HashMap<K, V>::remove(const K& key) {
    size_t index = hash(key) % capacity_;
    
    auto& bucket = buckets_[index];
    for (auto it = bucket.begin(); it != bucket.end(); ++it) {
        if (it->first == key) {
            bucket.erase(it);
            --size_;
            return;
        }
    }
    
    throw std::runtime_error("键不存在");
}

template<typename K, typename V>
size_t HashMap<K, V>::size() const {
    return size_;
}

template<typename K, typename V>
bool HashMap<K, V>::empty() const {
    return size_ == 0;
}

template<typename K, typename V>
void HashMap<K, V>::clear() {
    for (auto& bucket : buckets_) {
        bucket.clear();
    }
    size_ = 0;
}

template<typename K, typename V>
size_t HashMap<K, V>::hash(const K& key) const {
    return std::hash<K>{}(key);
}

template<typename K, typename V>
void HashMap<K, V>::rehash() {
    size_t oldCapacity = capacity_;
    capacity_ *= 2;
    
    std::vector<std::vector<std::pair<K, V>>> oldBuckets = std::move(buckets_);
    buckets_ = std::vector<std::vector<std::pair<K, V>>>(capacity_);
    size_ = 0;
    
    // 重新插入所有元素
    for (const auto& bucket : oldBuckets) {
        for (const auto& pair : bucket) {
            put(pair.first, pair.second);
        }
    }
}

// 集合实现
template<typename T>
Set<T>::Set() = default;

template<typename T>
Set<T>::~Set() = default;

template<typename T>
void Set<T>::add(const T& value) {
    data_.insert(value);
}

template<typename T>
void Set<T>::remove(const T& value) {
    data_.erase(value);
}

template<typename T>
bool Set<T>::contains(const T& value) const {
    return data_.find(value) != data_.end();
}

template<typename T>
size_t Set<T>::size() const {
    return data_.size();
}

template<typename T>
bool Set<T>::empty() const {
    return data_.empty();
}

template<typename T>
void Set<T>::clear() {
    data_.clear();
}

template<typename T>
std::vector<T> Set<T>::toVector() const {
    return std::vector<T>(data_.begin(), data_.end());
}

// 显式实例化常用类型
template class Array<int>;
template class Array<float>;
template class Array<std::string>;

template class LinkedList<int>;
template class LinkedList<float>;
template class LinkedList<std::string>;

template class HashMap<std::string, int>;
template class HashMap<std::string, float>;
template class HashMap<std::string, std::string>;
template class HashMap<int, std::string>;

template class Set<int>;
template class Set<float>;
template class Set<std::string>;

} // namespace stdlib
} // namespace starry
    for (size_t i = size_; i > index; --i) {
        data_[i] = data_[i - 1];
    }
    
    data_[index] = value;
    ++size_;
}

template<typename T>
void Array<T>::remove(size_t index) {
    if (index >= size_) {
        throw std::out_of_range("数组索引越界");
    }
    
    // 移动元素