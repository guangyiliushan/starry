#pragma once

#include <thread>
#include <mutex>
#include <condition_variable>
#include <atomic>
#include <functional>
#include <queue>
#include <vector>
#include <memory>
#include <future>

namespace starry {
namespace runtime {

/**
 * @brief 线程状态枚举
 */
enum class ThreadState {
    CREATED,    ///< 已创建
    RUNNING,    ///< 运行中
    BLOCKED,    ///< 阻塞
    WAITING,    ///< 等待
    TERMINATED  ///< 已终止
};

/**
 * @brief Starry线程类
 * 
 * 封装了标准库线程，提供Starry语言的线程功能
 */
class StarryThread {
public:
    using ThreadFunction = std::function<void()>;

    /**
     * @brief 构造函数
     * @param func 线程执行函数
     * @param name 线程名称
     */
    explicit StarryThread(ThreadFunction func, const std::string& name = "");

    /**
     * @brief 析构函数
     */
    ~StarryThread();

    /**
     * @brief 启动线程
     */
    void start();

    /**
     * @brief 等待线程结束
     */
    void join();

    /**
     * @brief 分离线程
     */
    void detach();

    /**
     * @brief 检查线程是否可连接
     * @return 可连接返回true，否则返回false
     */
    bool joinable() const;

    /**
     * @brief 获取线程ID
     * @return 线程ID
     */
    std::thread::id getId() const;

    /**
     * @brief 获取线程名称
     * @return 线程名称
     */
    const std::string& getName() const { return name_; }

    /**
     * @brief 获取线程状态
     * @return 线程状态
     */
    ThreadState getState() const { return state_; }

    /**
     * @brief 设置线程优先级
     * @param priority 优先级值
     */
    void setPriority(int priority);

    /**
     * @brief 获取线程优先级
     * @return 优先级值
     */
    int getPriority() const { return priority_; }

    /**
     * @brief 中断线程
     */
    void interrupt();

    /**
     * @brief 检查线程是否被中断
     * @return 被中断返回true，否则返回false
     */
    bool isInterrupted() const { return interrupted_; }

    /**
     * @brief 线程休眠
     * @param milliseconds 休眠时间（毫秒）
     */
    static void sleep(int milliseconds);

    /**
     * @brief 让出CPU时间片
     */
    static void yield();

    /**
     * @brief 获取当前线程
     * @return 当前线程指针
     */
    static StarryThread* currentThread();

private:
    void run();

private:
    std::unique_ptr<std::thread> thread_;    ///< 底层线程对象
    ThreadFunction function_;                ///< 线程执行函数
    std::string name_;                      ///< 线程名称
    std::atomic<ThreadState> state_;        ///< 线程状态
    std::atomic<bool> interrupted_;         ///< 中断标志
    int priority_;                          ///< 线程优先级
    mutable std::mutex mutex_;              ///< 互斥锁
};

/**
 * @brief 线程池类
 * 
 * 管理一组工作线程，用于执行任务
 */
class ThreadPool {
public:
    /**
     * @brief 构造函数
     * @param numThreads 线程数量
     */
    explicit ThreadPool(size_t numThreads = std::thread::hardware_concurrency());

    /**
     * @brief 析构函数
     */
    ~ThreadPool();

    /**
     * @brief 提交任务到线程池
     * @param func 任务函数
     * @param args 函数参数
     * @return 任务的future对象
     */
    template<typename F, typename... Args>
    auto submit(F&& func, Args&&... args) -> std::future<typename std::result_of<F(Args...)>::type>;

    /**
     * @brief 获取线程池大小
     * @return 线程数量
     */
    size_t size() const { return workers_.size(); }

    /**
     * @brief 获取活跃线程数量
     * @return 活跃线程数量
     */
    size_t activeThreads() const { return activeThreads_; }

    /**
     * @brief 获取待处理任务数量
     * @return 任务队列大小
     */
    size_t queueSize() const;

    /**
     * @brief 关闭线程池
     */
    void shutdown();

    /**
     * @brief 检查线程池是否已关闭
     * @return 已关闭返回true，否则返回false
     */
    bool isShutdown() const { return shutdown_; }

private:
    void workerThread();

private:
    std::vector<std::unique_ptr<StarryThread>> workers_;  ///< 工作线程
    std::queue<std::function<void()>> tasks_;            ///< 任务队列
    std::mutex queueMutex_;                              ///< 队列互斥锁
    std::condition_variable condition_;                   ///< 条件变量
    std::atomic<bool> shutdown_;                         ///< 关闭标志
    std::atomic<size_t> activeThreads_;                  ///< 活跃线程计数
};

/**
 * @brief 互斥锁类
 */
class Mutex {
public:
    Mutex() = default;
    ~Mutex() = default;

    /**
     * @brief 加锁
     */
    void lock();

    /**
     * @brief 尝试加锁
     * @return 成功返回true，否则返回false
     */
    bool tryLock();

    /**
     * @brief 解锁
     */
    void unlock();

private:
    std::mutex mutex_;
};

/**
 * @brief 条件变量类
 */
class ConditionVariable {
public:
    ConditionVariable() = default;
    ~ConditionVariable() = default;

    /**
     * @brief 等待条件
     * @param mutex 关联的互斥锁
     */
    void wait(Mutex& mutex);

    /**
     * @brief 带超时的等待
     * @param mutex 关联的互斥锁
     * @param timeoutMs 超时时间（毫秒）
     * @return 在超时前被唤醒返回true，否则返回false
     */
    bool waitFor(Mutex& mutex, int timeoutMs);

    /**
     * @brief 唤醒一个等待线程
     */
    void notify();

    /**
     * @brief 唤醒所有等待线程
     */
    void notifyAll();

private:
    std::condition_variable cv_;
};

/**
 * @brief 读写锁类
 */
class ReadWriteLock {
public:
    ReadWriteLock() = default;
    ~ReadWriteLock() = default;

    /**
     * @brief 获取读锁
     */
    void readLock();

    /**
     * @brief 尝试获取读锁
     * @return 成功返回true，否则返回false
     */
    bool tryReadLock();

    /**
     * @brief 释放读锁
     */
    void readUnlock();

    /**
     * @brief 获取写锁
     */
    void writeLock();

    /**
     * @brief 尝试获取写锁
     * @return 成功返回true，否则返回false
     */
    bool tryWriteLock();

    /**
     * @brief 释放写锁
     */
    void writeUnlock();

private:
    std::shared_mutex rwMutex_;
};

/**
 * @brief 原子操作工具类
 */
class Atomic {
public:
    /**
     * @brief 原子递增
     * @param value 要递增的值
     * @return 递增后的值
     */
    template<typename T>
    static T increment(std::atomic<T>& value);

    /**
     * @brief 原子递减
     * @param value 要递减的值
     * @return 递减后的值
     */
    template<typename T>
    static T decrement(std::atomic<T>& value);

    /**
     * @brief 原子比较并交换
     * @param value 目标值
     * @param expected 期望值
     * @param desired 新值
     * @return 交换成功返回true，否则返回false
     */
    template<typename T>
    static bool compareAndSwap(std::atomic<T>& value, T& expected, T desired);
};

// 线程本地存储
extern thread_local StarryThread* currentThread;

// 全局线程管理函数
void initializeThreadSystem();
void cleanupThreadSystem();
void registerThread(StarryThread* thread);
void unregisterThread(StarryThread* thread);
std::vector<StarryThread*> getAllThreads();

} // namespace runtime
} // namespace starry