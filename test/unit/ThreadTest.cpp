#include <gtest/gtest.h>
#include "starry/runtime/Thread.h"
#include <chrono>
#include <atomic>

using namespace starry::runtime;

class ThreadTest : public ::testing::Test {
protected:
    void SetUp() override {}
    void TearDown() override {}
};

TEST_F(ThreadTest, ThreadPoolBasicFunctionality) {
    ThreadPool pool(2);
    
    std::atomic<int> counter{0};
    
    // 提交一些任务
    for (int i = 0; i < 10; ++i) {
        pool.enqueue([&counter]() {
            counter++;
            std::this_thread::sleep_for(std::chrono::milliseconds(10));
        });
    }
    
    // 等待任务完成
    std::this_thread::sleep_for(std::chrono::milliseconds(200));
    
    EXPECT_EQ(counter.load(), 10);
    EXPECT_EQ(pool.getWorkerCount(), 2);
}

TEST_F(ThreadTest, ThreadManagerSingleton) {
    ThreadManager& manager1 = ThreadManager::getInstance();
    ThreadManager& manager2 = ThreadManager::getInstance();
    
    EXPECT_EQ(&manager1, &manager2);
}

TEST_F(ThreadTest, ThreadManagerExecuteTask) {
    ThreadManager& manager = ThreadManager::getInstance();
    
    std::atomic<bool> taskExecuted{false};
    
    manager.executeTask([&taskExecuted]() {
        taskExecuted = true;
    });
    
    // 等待任务执行
    std::this_thread::sleep_for(std::chrono::milliseconds(100));
    
    EXPECT_TRUE(taskExecuted.load());
}

TEST_F(ThreadTest, ThreadManagerAsyncTask) {
    ThreadManager& manager = ThreadManager::getInstance();
    
    auto future = manager.executeTaskAsync([]() {
        std::this_thread::sleep_for(std::chrono::milliseconds(50));
    });
    
    // 等待任务完成
    future.wait();
    
    EXPECT_TRUE(future.valid());
}

TEST_F(ThreadTest, CustomThreadPool) {
    ThreadManager& manager = ThreadManager::getInstance();
    
    manager.createCustomPool("testPool", 3);
    
    std::atomic<int> counter{0};
    
    for (int i = 0; i < 5; ++i) {
        manager.executeTaskInPool("testPool", [&counter]() {
            counter++;
        });
    }
    
    // 等待任务完成
    std::this_thread::sleep_for(std::chrono::milliseconds(100));
    
    EXPECT_EQ(counter.load(), 5);
    
    manager.removeCustomPool("testPool");
}

TEST_F(ThreadTest, ThreadStats) {
    ThreadManager& manager = ThreadManager::getInstance();
    
    ThreadStats stats = manager.getStats();
    
    EXPECT_GT(stats.defaultPoolWorkers, 0);
    EXPECT_GE(stats.defaultPoolQueueSize, 0);
    EXPECT_GE(stats.customPoolCount, 0);
}

TEST_F(ThreadTest, MutexBasicFunctionality) {
    Mutex mutex;
    std::atomic<int> counter{0};
    std::vector<std::thread> threads;
    
    // 创建多个线程同时访问共享资源
    for (int i = 0; i < 5; ++i) {
        threads.emplace_back([&mutex, &counter]() {
            for (int j = 0; j < 100; ++j) {
                mutex.lock();
                counter++;
                mutex.unlock();
            }
        });
    }
    
    // 等待所有线程完成
    for (auto& thread : threads) {
        thread.join();
    }
    
    EXPECT_EQ(counter.load(), 500);
}

TEST_F(ThreadTest, MutexTryLock) {
    Mutex mutex;
    
    EXPECT_TRUE(mutex.tryLock());
    EXPECT_FALSE(mutex.tryLock()); // 已经被锁定
    
    mutex.unlock();
    EXPECT_TRUE(mutex.tryLock());
    mutex.unlock();
}

TEST_F(ThreadTest, SemaphoreBasicFunctionality) {
    Semaphore semaphore(2);
    
    EXPECT_EQ(semaphore.getCount(), 2);
    
    semaphore.acquire();
    EXPECT_EQ(semaphore.getCount(), 1);
    
    semaphore.acquire();
    EXPECT_EQ(semaphore.getCount(), 0);
    
    EXPECT_FALSE(semaphore.tryAcquire());
    
    semaphore.release();
    EXPECT_EQ(semaphore.getCount(), 1);
    EXPECT_TRUE(semaphore.tryAcquire());
}

TEST_F(ThreadTest, SemaphoreBlocking) {
    Semaphore semaphore(1);
    std::atomic<bool> taskStarted{false};
    std::atomic<bool> taskCompleted{false};
    
    // 获取信号量
    semaphore.acquire();
    
    // 启动一个线程尝试获取信号量
    std::thread worker([&semaphore, &taskStarted, &taskCompleted]() {
        taskStarted = true;
        semaphore.acquire(); // 这里会阻塞
        taskCompleted = true;
        semaphore.release();
    });
    
    // 等待工作线程启动
    while (!taskStarted.load()) {
        std::this_thread::sleep_for(std::chrono::milliseconds(1));
    }
    
    // 短暂等待确保工作线程被阻塞
    std::this_thread::sleep_for(std::chrono::milliseconds(50));
    EXPECT_FALSE(taskCompleted.load());
    
    // 释放信号量
    semaphore.release();
    
    // 等待工作线程完成
    worker.join();
    EXPECT_TRUE(taskCompleted.load());
}

TEST_F(ThreadTest, ConditionVariable) {
    Mutex mutex;
    ConditionVariable cv;
    std::atomic<bool> ready{false};
    std::atomic<bool> processed{false};
    
    std::thread worker([&mutex, &cv, &ready, &processed]() {
        mutex.lock();
        while (!ready.load()) {
            cv.wait(mutex);
        }
        processed = true;
        mutex.unlock();
    });
    
    // 短暂等待确保工作线程开始等待
    std::this_thread::sleep_for(std::chrono::milliseconds(50));
    
    mutex.lock();
    ready = true;
    mutex.unlock();
    cv.notify();
    
    worker.join();
    EXPECT_TRUE(processed.load());
}

TEST_F(ThreadTest, ThreadLocalStorage) {
    ThreadLocalStorage& tls = ThreadLocalStorage::getInstance();
    
    tls.setValue("test_key", std::string("test_value"));
    
    EXPECT_TRUE(tls.hasValue("test_key"));
    
    auto value = std::any_cast<std::string>(tls.getValue("test_key"));
    EXPECT_EQ(value, "test_value");
    
    tls.removeValue("test_key");
    EXPECT_FALSE(tls.hasValue("test_key"));
}

TEST_F(ThreadTest, ThreadLocalStorageIsolation) {
    std::atomic<int> completedThreads{0};
    
    std::thread thread1([&completedThreads]() {
        ThreadLocalStorage& tls = ThreadLocalStorage::getInstance();
        tls.setValue("thread_id", 1);
        
        auto value = std::any_cast<int>(tls.getValue("thread_id"));
        EXPECT_EQ(value, 1);
        
        completedThreads++;
    });
    
    std::thread thread2([&completedThreads]() {
        ThreadLocalStorage& tls = ThreadLocalStorage::getInstance();
        tls.setValue("thread_id", 2);
        
        auto value = std::any_cast<int>(tls.getValue("thread_id"));
        EXPECT_EQ(value, 2);
        
        completedThreads++;
    });
    
    thread1.join();
    thread2.join();
    
    EXPECT_EQ(completedThreads.load(), 2);
}

TEST_F(ThreadTest, UtilityFunctions) {
    auto threadId = getCurrentThreadId();
    EXPECT_NE(threadId, std::thread::id{});
    
    auto concurrency = getHardwareConcurrency();
    EXPECT_GT(concurrency, 0);
    
    // 测试sleep函数
    auto start = std::chrono::high_resolution_clock::now();
    sleep(100);
    auto end = std::chrono::high_resolution_clock::now();
    
    auto duration = std::chrono::duration_cast<std::chrono::milliseconds>(end - start);
    EXPECT_GE(duration.count(), 90); // 允许一些误差
    EXPECT_LE(duration.count(), 150);
}