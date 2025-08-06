#include "starry/runtime/Thread.h"
#include <iostream>
#include <stdexcept>
#include <chrono>

namespace starry {
namespace runtime {

// 线程池实现
ThreadPool::ThreadPool(size_t numThreads) : stop_(false) {
    for (size_t i = 0; i < numThreads; ++i) {
        workers_.emplace_back([this] {
            for (;;) {
                std::function<void()> task;
                
                {
                    std::unique_lock<std::mutex> lock(queueMutex_);
                    condition_.wait(lock, [this] { return stop_ || !tasks_.empty(); });
                    
                    if (stop_ && tasks_.empty()) {
                        return;
                    }
                    
                    task = std::move(tasks_.front());
                    tasks_.pop();
                }
                
                task();
            }
        });
    }
}

ThreadPool::~ThreadPool() {
    {
        std::unique_lock<std::mutex> lock(queueMutex_);
        stop_ = true;
    }
    
    condition_.notify_all();
    
    for (std::thread& worker : workers_) {
        worker.join();
    }
}

void ThreadPool::enqueue(std::function<void()> task) {
    {
        std::unique_lock<std::mutex> lock(queueMutex_);
        
        if (stop_) {
            throw std::runtime_error("线程池已停止，无法添加新任务");
        }
        
        tasks_.emplace(std::move(task));
    }
    
    condition_.notify_one();
}

size_t ThreadPool::getWorkerCount() const {
    return workers_.size();
}

size_t ThreadPool::getQueueSize() const {
    std::unique_lock<std::mutex> lock(queueMutex_);
    return tasks_.size();
}

// 线程管理器实现
ThreadManager::ThreadManager() : defaultPool_(std::thread::hardware_concurrency()) {}

ThreadManager::~ThreadManager() = default;

ThreadManager& ThreadManager::getInstance() {
    static ThreadManager instance;
    return instance;
}

void ThreadManager::executeTask(std::function<void()> task) {
    defaultPool_.enqueue(std::move(task));
}

std::future<void> ThreadManager::executeTaskAsync(std::function<void()> task) {
    auto taskPtr = std::make_shared<std::packaged_task<void()>>(std::move(task));
    std::future<void> result = taskPtr->get_future();
    
    defaultPool_.enqueue([taskPtr]() {
        (*taskPtr)();
    });
    
    return result;
}

void ThreadManager::createCustomPool(const std::string& name, size_t numThreads) {
    std::lock_guard<std::mutex> lock(poolsMutex_);
    
    if (customPools_.find(name) != customPools_.end()) {
        throw std::runtime_error("线程池已存在: " + name);
    }
    
    customPools_[name] = std::make_unique<ThreadPool>(numThreads);
}

void ThreadManager::executeTaskInPool(const std::string& poolName, std::function<void()> task) {
    std::lock_guard<std::mutex> lock(poolsMutex_);
    
    auto it = customPools_.find(poolName);
    if (it == customPools_.end()) {
        throw std::runtime_error("线程池不存在: " + poolName);
    }
    
    it->second->enqueue(std::move(task));
}

void ThreadManager::removeCustomPool(const std::string& name) {
    std::lock_guard<std::mutex> lock(poolsMutex_);
    customPools_.erase(name);
}

ThreadStats ThreadManager::getStats() const {
    ThreadStats stats;
    stats.defaultPoolWorkers = defaultPool_.getWorkerCount();
    stats.defaultPoolQueueSize = defaultPool_.getQueueSize();
    
    std::lock_guard<std::mutex> lock(poolsMutex_);
    stats.customPoolCount = customPools_.size();
    
    for (const auto& pair : customPools_) {
        stats.totalWorkers += pair.second->getWorkerCount();
        stats.totalQueueSize += pair.second->getQueueSize();
    }
    
    return stats;
}

// 线程同步原语实现
Mutex::Mutex() = default;
Mutex::~Mutex() = default;

void Mutex::lock() {
    mutex_.lock();
}

void Mutex::unlock() {
    mutex_.unlock();
}

bool Mutex::tryLock() {
    return mutex_.try_lock();
}

Semaphore::Semaphore(int count) : count_(count) {}

void Semaphore::acquire() {
    std::unique_lock<std::mutex> lock(mutex_);
    condition_.wait(lock, [this] { return count_ > 0; });
    --count_;
}

void Semaphore::release() {
    std::lock_guard<std::mutex> lock(mutex_);
    ++count_;
    condition_.notify_one();
}

bool Semaphore::tryAcquire() {
    std::lock_guard<std::mutex> lock(mutex_);
    if (count_ > 0) {
        --count_;
        return true;
    }
    return false;
}

int Semaphore::getCount() const {
    std::lock_guard<std::mutex> lock(mutex_);
    return count_;
}

ConditionVariable::ConditionVariable() = default;
ConditionVariable::~ConditionVariable() = default;

void ConditionVariable::wait(Mutex& mutex) {
    std::unique_lock<std::mutex> lock(mutex.mutex_, std::adopt_lock);
    condition_.wait(lock);
    lock.release();
}

void ConditionVariable::notify() {
    condition_.notify_one();
}

void ConditionVariable::notifyAll() {
    condition_.notify_all();
}

// 线程本地存储实现
thread_local ThreadLocalStorage* ThreadLocalStorage::instance_ = nullptr;

ThreadLocalStorage::ThreadLocalStorage() = default;
ThreadLocalStorage::~ThreadLocalStorage() = default;

ThreadLocalStorage& ThreadLocalStorage::getInstance() {
    if (!instance_) {
        instance_ = new ThreadLocalStorage();
    }
    return *instance_;
}

void ThreadLocalStorage::setValue(const std::string& key, std::any value) {
    storage_[key] = std::move(value);
}

std::any ThreadLocalStorage::getValue(const std::string& key) const {
    auto it = storage_.find(key);
    if (it != storage_.end()) {
        return it->second;
    }
    return std::any{};
}

bool ThreadLocalStorage::hasValue(const std::string& key) const {
    return storage_.find(key) != storage_.end();
}

void ThreadLocalStorage::removeValue(const std::string& key) {
    storage_.erase(key);
}

void ThreadLocalStorage::clear() {
    storage_.clear();
}

// 工具函数实现
void sleep(int milliseconds) {
    std::this_thread::sleep_for(std::chrono::milliseconds(milliseconds));
}

std::thread::id getCurrentThreadId() {
    return std::this_thread::get_id();
}

size_t getHardwareConcurrency() {
    return std::thread::hardware_concurrency();
}

void yieldThread() {
    std::this_thread::yield();
}

} // namespace runtime
} // namespace starry