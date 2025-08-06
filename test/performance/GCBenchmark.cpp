#include <benchmark/benchmark.h>
#include "starry/runtime/Memory.h"
#include <vector>
#include <random>

using namespace starry::runtime;

// 垃圾回收性能基准测试

static void BM_GC_AllocationDeallocation(benchmark::State& state) {
    initialize_memory(64 * 1024 * 1024); // 64MB
    
    const size_t allocation_size = state.range(0);
    
    for (auto _ : state) {
        std::vector<void*> allocations;
        
        // 分配内存
        for (int i = 0; i < 1000; ++i) {
            void* ptr = allocate_memory(allocation_size);
            if (ptr) {
                allocations.push_back(ptr);
            }
        }
        
        // 释放内存
        for (void* ptr : allocations) {
            deallocate_memory(ptr);
        }
        
        // 触发垃圾回收
        trigger_gc();
    }
    
    cleanup_memory();
}

static void BM_GC_FragmentedAllocation(benchmark::State& state) {
    initialize_memory(64 * 1024 * 1024);
    
    std::random_device rd;
    std::mt19937 gen(rd());
    std::uniform_int_distribution<> size_dist(16, 1024);
    
    for (auto _ : state) {
        std::vector<void*> allocations;
        
        // 随机大小分配
        for (int i = 0; i < 500; ++i) {
            size_t size = size_dist(gen);
            void* ptr = allocate_memory(size);
            if (ptr) {
                allocations.push_back(ptr);
            }
        }
        
        // 随机释放一半内存
        for (size_t i = 0; i < allocations.size(); i += 2) {
            deallocate_memory(allocations[i]);
        }
        
        // 再次分配
        for (int i = 0; i < 250; ++i) {
            size_t size = size_dist(gen);
            void* ptr = allocate_memory(size);
            if (ptr) {
                allocations.push_back(ptr);
            }
        }
        
        // 清理剩余内存
        for (size_t i = 1; i < allocations.size(); i += 2) {
            deallocate_memory(allocations[i]);
        }
        
        trigger_gc();
    }
    
    cleanup_memory();
}

static void BM_GC_LargeObjectAllocation(benchmark::State& state) {
    initialize_memory(128 * 1024 * 1024); // 128MB
    
    const size_t large_size = 1024 * 1024; // 1MB
    
    for (auto _ : state) {
        std::vector<void*> large_objects;
        
        // 分配大对象
        for (int i = 0; i < 50; ++i) {
            void* ptr = allocate_memory(large_size);
            if (ptr) {
                large_objects.push_back(ptr);
            }
        }
        
        // 释放大对象
        for (void* ptr : large_objects) {
            deallocate_memory(ptr);
        }
        
        trigger_gc();
    }
    
    cleanup_memory();
}

static void BM_GC_MarkAndSweep(benchmark::State& state) {
    initialize_memory(64 * 1024 * 1024);
    
    // 预分配一些对象
    std::vector<void*> persistent_objects;
    for (int i = 0; i < 100; ++i) {
        void* ptr = allocate_memory(1024);
        if (ptr) {
            persistent_objects.push_back(ptr);
        }
    }
    
    for (auto _ : state) {
        // 分配临时对象
        std::vector<void*> temp_objects;
        for (int i = 0; i < 500; ++i) {
            void* ptr = allocate_memory(512);
            if (ptr) {
                temp_objects.push_back(ptr);
            }
        }
        
        // 只释放一部分临时对象，模拟垃圾
        for (size_t i = 0; i < temp_objects.size() / 2; ++i) {
            deallocate_memory(temp_objects[i]);
        }
        
        // 测量垃圾回收时间
        auto start = std::chrono::high_resolution_clock::now();
        trigger_gc();
        auto end = std::chrono::high_resolution_clock::now();
        
        auto duration = std::chrono::duration_cast<std::chrono::microseconds>(end - start);
        state.SetIterationTime(duration.count() / 1000000.0);
        
        // 清理剩余临时对象
        for (size_t i = temp_objects.size() / 2; i < temp_objects.size(); ++i) {
            deallocate_memory(temp_objects[i]);
        }
    }
    
    // 清理持久对象
    for (void* ptr : persistent_objects) {
        deallocate_memory(ptr);
    }
    
    cleanup_memory();
}

static void BM_GC_MemoryPressure(benchmark::State& state) {
    initialize_memory(32 * 1024 * 1024); // 较小的内存池
    
    for (auto _ : state) {
        std::vector<void*> allocations;
        
        // 持续分配直到内存压力增大
        for (int i = 0; i < 2000; ++i) {
            void* ptr = allocate_memory(1024);
            if (ptr) {
                allocations.push_back(ptr);
            } else {
                // 内存不足，触发GC
                trigger_gc();
                ptr = allocate_memory(1024);
                if (ptr) {
                    allocations.push_back(ptr);
                }
            }
        }
        
        // 清理所有分配
        for (void* ptr : allocations) {
            deallocate_memory(ptr);
        }
        
        trigger_gc();
    }
    
    cleanup_memory();
}

static void BM_GC_ConcurrentAllocation(benchmark::State& state) {
    initialize_memory(64 * 1024 * 1024);
    
    const int num_threads = state.range(0);
    
    for (auto _ : state) {
        std::vector<std::thread> threads;
        std::vector<std::vector<void*>> thread_allocations(num_threads);
        
        // 启动多个线程同时分配内存
        for (int t = 0; t < num_threads; ++t) {
            threads.emplace_back([&, t]() {
                for (int i = 0; i < 200; ++i) {
                    void* ptr = allocate_memory(256);
                    if (ptr) {
                        thread_allocations[t].push_back(ptr);
                    }
                }
            });
        }
        
        // 等待所有线程完成
        for (auto& thread : threads) {
            thread.join();
        }
        
        // 清理所有分配
        for (const auto& allocations : thread_allocations) {
            for (void* ptr : allocations) {
                deallocate_memory(ptr);
            }
        }
        
        trigger_gc();
    }
    
    cleanup_memory();
}

static void BM_GC_GenerationalCollection(benchmark::State& state) {
    initialize_memory(64 * 1024 * 1024);
    
    for (auto _ : state) {
        // 模拟分代垃圾回收场景
        std::vector<void*> young_generation;
        std::vector<void*> old_generation;
        
        // 分配年轻代对象
        for (int i = 0; i < 1000; ++i) {
            void* ptr = allocate_memory(128);
            if (ptr) {
                young_generation.push_back(ptr);
            }
        }
        
        // 一些对象存活到老年代
        for (size_t i = 0; i < young_generation.size() / 10; ++i) {
            old_generation.push_back(young_generation[i]);
        }
        
        // 释放大部分年轻代对象
        for (size_t i = young_generation.size() / 10; i < young_generation.size(); ++i) {
            deallocate_memory(young_generation[i]);
        }
        
        // 触发年轻代GC
        trigger_gc();
        
        // 继续分配新的年轻代对象
        young_generation.clear();
        for (int i = 0; i < 800; ++i) {
            void* ptr = allocate_memory(128);
            if (ptr) {
                young_generation.push_back(ptr);
            }
        }
        
        // 清理所有对象
        for (void* ptr : young_generation) {
            deallocate_memory(ptr);
        }
        for (void* ptr : old_generation) {
            deallocate_memory(ptr);
        }
        
        trigger_gc();
    }
    
    cleanup_memory();
}

// 注册基准测试
BENCHMARK(BM_GC_AllocationDeallocation)->Range(64, 8192);
BENCHMARK(BM_GC_FragmentedAllocation);
BENCHMARK(BM_GC_LargeObjectAllocation);
BENCHMARK(BM_GC_MarkAndSweep)->UseManualTime();
BENCHMARK(BM_GC_MemoryPressure);
BENCHMARK(BM_GC_ConcurrentAllocation)->Range(1, 8);
BENCHMARK(BM_GC_GenerationalCollection);

BENCHMARK_MAIN();