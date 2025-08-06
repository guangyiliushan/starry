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

// 显式实例化常用类型
template class Array<int>;
template class Array<float>;
template class Array<std::string>;

} // namespace stdlib
} // namespace starry