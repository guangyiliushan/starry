/**
 * @file String.cpp
 * @brief Starry语言标准库字符串实现
 * @author Starry Team
 * @date 2024
 */

#include "starry/stdlib/String.h"
#include <algorithm>
#include <sstream>
#include <cctype>

namespace starry {
namespace stdlib {

// StarryString实现
StarryString::StarryString() : data_("") {}

StarryString::StarryString(const std::string& str) : data_(str) {}

StarryString::StarryString(const char* str) : data_(str ? str : "") {}

StarryString::StarryString(const StarryString& other) : data_(other.data_) {}

StarryString& StarryString::operator=(const StarryString& other) {
    if (this != &other) {
        data_ = other.data_;
    }
    return *this;
}

StarryString::StarryString(StarryString&& other) noexcept : data_(std::move(other.data_)) {}

StarryString& StarryString::operator=(StarryString&& other) noexcept {
    if (this != &other) {
        data_ = std::move(other.data_);
    }
    return *this;
}

size_t StarryString::length() const {
    return data_.length();
}

bool StarryString::empty() const {
    return data_.empty();
}

const char* StarryString::c_str() const {
    return data_.c_str();
}

const std::string& StarryString::str() const {
    return data_;
}

StarryString StarryString::substring(size_t start, size_t length) const {
    if (start >= data_.length()) {
        return StarryString();
    }
    return StarryString(data_.substr(start, length));
}

size_t StarryString::indexOf(const StarryString& substr) const {
    size_t pos = data_.find(substr.data_);
    return pos == std::string::npos ? SIZE_MAX : pos;
}

bool StarryString::contains(const StarryString& substr) const {
    return data_.find(substr.data_) != std::string::npos;
}

StarryString StarryString::toLowerCase() const {
    std::string result = data_;
    std::transform(result.begin(), result.end(), result.begin(), ::tolower);
    return StarryString(result);
}

StarryString StarryString::toUpperCase() const {
    std::string result = data_;
    std::transform(result.begin(), result.end(), result.begin(), ::toupper);
    return StarryString(result);
}

StarryString StarryString::trim() const {
    std::string result = data_;
    
    // 去除前导空白
    result.erase(result.begin(), std::find_if(result.begin(), result.end(), [](unsigned char ch) {
        return !std::isspace(ch);
    }));
    
    // 去除尾随空白
    result.erase(std::find_if(result.rbegin(), result.rend(), [](unsigned char ch) {
        return !std::isspace(ch);
    }).base(), result.end());
    
    return StarryString(result);
}

std::vector<StarryString> StarryString::split(const StarryString& delimiter) const {
    std::vector<StarryString> result;
    std::string str = data_;
    std::string delim = delimiter.data_;
    
    size_t start = 0;
    size_t end = str.find(delim);
    
    while (end != std::string::npos) {
        result.emplace_back(str.substr(start, end - start));
        start = end + delim.length();
        end = str.find(delim, start);
    }
    
    result.emplace_back(str.substr(start));
    return result;
}

StarryString StarryString::replace(const StarryString& from, const StarryString& to) const {
    std::string result = data_;
    std::string from_str = from.data_;
    std::string to_str = to.data_;
    
    size_t pos = 0;
    while ((pos = result.find(from_str, pos)) != std::string::npos) {
        result.replace(pos, from_str.length(), to_str);
        pos += to_str.length();
    }
    
    return StarryString(result);
}

// 运算符重载
StarryString StarryString::operator+(const StarryString& other) const {
    return StarryString(data_ + other.data_);
}

StarryString& StarryString::operator+=(const StarryString& other) {
    data_ += other.data_;
    return *this;
}

bool StarryString::operator==(const StarryString& other) const {
    return data_ == other.data_;
}

bool StarryString::operator!=(const StarryString& other) const {
    return data_ != other.data_;
}

bool StarryString::operator<(const StarryString& other) const {
    return data_ < other.data_;
}

char StarryString::operator[](size_t index) const {
    if (index >= data_.length()) {
        throw std::out_of_range("字符串索引超出范围");
    }
    return data_[index];
}

// 全局函数
StarryString toString(int value) {
    return StarryString(std::to_string(value));
}

StarryString toString(double value) {
    return StarryString(std::to_string(value));
}

StarryString toString(bool value) {
    return StarryString(value ? "true" : "false");
}

int toInt(const StarryString& str) {
    try {
        return std::stoi(str.str());
    } catch (const std::exception&) {
        return 0;
    }
}

double toDouble(const StarryString& str) {
    try {
        return std::stod(str.str());
    } catch (const std::exception&) {
        return 0.0;
    }
}

bool toBool(const StarryString& str) {
    std::string lower = str.toLowerCase().str();
    return lower == "true" || lower == "1" || lower == "yes";
}

} // namespace stdlib
} // namespace starry