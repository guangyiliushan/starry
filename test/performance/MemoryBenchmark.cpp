/**
 * @file MemoryBenchmark.cpp
 * @brief Starry语言内存管理性能测试
 * @author Starry Team
 * @date 2024
 */

#include <benchmark/benchmark.h>
#include "starry/runtime/Memory.h"
#include <vector>
#include <random>
#include <cstring>

using namespace starry::runtime;

// 设置和清理函数
class MemoryFixture : public benchmark::Fixture {
public:
    void SetUp(const ::benchmark::State& state) override {
        initialize_memory(10 * 1024 * 1024); // 10MB内存池
    }
    
    void TearDown(const ::benchmark::State& state) override {
        cleanup_memory();
    }
};

// 基准测试：基本内存分配
BENCHMARK_F(MemoryFixture, BM_BasicAllocation)(benchmark::State& state) {
    for (auto _ : state) {
        void* ptr = allocate_memory(1024);
        benchmark::DoNotOptimize(ptr);
        if (ptr) {
            deallocate_memory(ptr);
        }
    }
}

// 基准测试：小块内存分配
BENCHMARK_F(MemoryFixture, BM_SmallAllocation)(benchmark::State& state) {
    for (auto _ : state) {
        void* ptr = allocate_memory(64);
        benchmark::DoNotOptimize(ptr);
        if (ptr) {
            deallocate_memory(ptr);
        }
    }
}

// 基准测试：大块内存分配
BENCHMARK_F(MemoryFixture, BM_LargeAllocation)(benchmark::State& state) {
    for (auto _ : state) {
        void* ptr = allocate_memory(64 * 1024); // 64KB
        benchmark::DoNotOptimize(ptr);
        if (ptr) {
            deallocate_memory(ptr);
        }
    }
}

// 基准测试：批量分配
BENCHMARK_F(MemoryFixture, BM_BatchAllocation)(benchmark::State& state) {
    const int batch_size = 100;
    
    for (auto _ : state) {
        std::vector<void*> pointers;
        pointers.reserve(batch_size);
        
        // 批量分配
        for (int i = 0; i < batch_size; ++i) {
            void* ptr = allocate_memory(128);
            if (ptr) {
                pointers.push_back(ptr);
            }
        }
        
        // 批量释放
        for (void* ptr : pointers) {
            deallocate_memory(ptr);
        }
        
        benchmark::DoNotOptimize(pointers);
    }
}

// 基准测试：随机大小分配
BENCHMARK_F(MemoryFixture, BM_RandomSizeAllocation)(benchmark::State& state) {
    std::random_device rd;
    std::mt19937 gen(rd());
    std::uniform_int_distribution<> dis(32, 2048);
    
    for (auto _ : state) {
        size_t size = dis(gen);
        void* ptr = allocate_memory(size);
        benchmark::DoNotOptimize(ptr);
        if (ptr) {
            deallocate_memory(ptr);
        }
    }
}

// 基准测试：内存写入性能
BENCHMARK_F(MemoryFixture, BM_MemoryWrite)(benchmark::State& state) {
    const size_t size = 4096;
    void* ptr = allocate_memory(size);
    
    if (!ptr) {
        state.SkipWithError("内存分配失败");
        return;
    }
    
    for (auto _ : state) {
        memset(ptr, 0xAA, size);
        benchmark::DoNotOptimize(ptr);
    }
    
    deallocate_memory(ptr);
}

// 基准测试：内存读取性能
BENCHMARK_F(MemoryFixture, BM_MemoryRead)(benchmark::State& state) {
    const size_t size = 4096;
    void* ptr = allocate_memory(size);
    
    if (!ptr) {
        state.SkipWithError("内存分配失败");
        return;
    }
    
    // 初始化内存
    memset(ptr, 0x55, size);
    
    for (auto _ : state) {
        volatile char sum = 0;
        char* char_ptr = static_cast<char*>(ptr);
        for (size_t i = 0; i < size; ++i) {
            sum += char_ptr[i];
        }
        benchmark::DoNotOptimize(sum);
    }
    
    deallocate_memory(ptr);
}

// 基准测试：内存拷贝性能
BENCHMARK_F(MemoryFixture, BM_MemoryCopy)(benchmark::State& state) {
    const size_t size = 4096;
    void* src = allocate_memory(size);
    void* dst = allocate_memory(size);
    
    if (!src || !dst) {
        state.SkipWithError("内存分配失败");
        if (src) deallocate_memory(src);
        if (dst) deallocate_memory(dst);
        return;
    }
    
    // 初始化源内存
    memset(src, 0x33, size);
    
    for (auto _ : state) {
        memcpy(dst, src, size);
        benchmark::DoNotOptimize(dst);
    }
    
    deallocate_memory(src);
    deallocate_memory(dst);
}

// 基准测试：内存碎片化测试
BENCHMARK_F(MemoryFixture, BM_MemoryFragmentation)(benchmark::State& state) {
    const int num_allocs = 100;
    std::vector<void*> pointers;
    
    for (auto _ : state) {
        pointers.clear();
        
        // 分配大量不同大小的内存块
        for (int i = 0; i < num_allocs; ++i) {
            size_t size = 64 + (i % 10) * 32; // 64到384字节
            void* ptr = allocate_memory(size);
            if (ptr) {
                pointers.push_back(ptr);
            }
        }
        
        // 释放一半内存块（造成碎片）
        for (size_t i = 0; i < pointers.size(); i += 2) {
            deallocate_memory(pointers[i]);
        }
        
        // 尝试分配新的内存块
        void* new_ptr = allocate_memory(256);
        if (new_ptr) {
            deallocate_memory(new_ptr);
        }
        
        // 释放剩余内存
        for (size_t i = 1; i < pointers.size(); i += 2) {
            deallocate_memory(pointers[i]);
        }
        
        benchmark::DoNotOptimize(pointers);
    }
}

// 基准测试：不同大小的内存分配性能
BENCHMARK_F(MemoryFixture, BM_AllocationSizes)(benchmark::State& state) {
    size_t size = state.range(0);
    
    for (auto _ : state) {
        void* ptr = allocate_memory(size);
        benchmark::DoNotOptimize(ptr);
        if (ptr) {
            deallocate_memory(ptr);
        }
    }
    
    state.SetBytesProcessed(state.iterations() * size);
}
BENCHMARK_F(MemoryFixture, BM_AllocationSizes)->Range(32, 32768)->RangeMultiplier(2);

// 基准测试：内存池压力测试
BENCHMARK_F(MemoryFixture, BM_MemoryPoolStress)(benchmark::State& state) {
    const int max_allocs = 1000;
    std::vector<void*> active_pointers;
    std::random_device rd;
    std::mt19937 gen(rd());
    std::uniform_int_distribution<> size_dis(32, 1024);
    std::uniform_real_distribution<> action_dis(0.0, 1.0);
    
    for (auto _ : state) {
        // 随机分配或释放内存
        if (active_pointers.empty() || (active_pointers.size() < max_allocs && action_dis(gen) < 0.7)) {
            // 分配内存
            size_t size = size_dis(gen);
            void* ptr = allocate_memory(size);
            if (ptr) {
                active_pointers.push_back(ptr);
            }
        } else {
            // 释放内存
            if (!active_pointers.empty()) {
                std::uniform_int_distribution<> index_dis(0, active_pointers.size() - 1);
                int index = index_dis(gen);
                deallocate_memory(active_pointers[index]);
                active_pointers.erase(active_pointers.begin() + index);
            }
        }
        
        benchmark::DoNotOptimize(active_pointers);
    }
    
    // 清理剩余内存
    for (void* ptr : active_pointers) {
        deallocate_memory(ptr);
    }
}

// 基准测试：并发内存分配
BENCHMARK_F(MemoryFixture, BM_ConcurrentAllocation)(benchmark::State& state) {
    for (auto _ : state) {
        void* ptr = allocate_memory(512);
        benchmark::DoNotOptimize(ptr);
        if (ptr) {
            // 写入一些数据
            char* char_ptr = static_cast<char*>(ptr);
            char_ptr[0] = 'A';
            char_ptr[511] = 'Z';
            
            deallocate_memory(ptr);
        }
    }
}
BENCHMARK_F(MemoryFixture, BM_ConcurrentAllocation)->Threads(4);

// 基准测试：内存对齐性能
BENCHMARK_F(MemoryFixture, BM_MemoryAlignment)(benchmark::State& state) {
    for (auto _ : state) {
        void* ptr = allocate_memory(1024);
        if (ptr) {
            // 检查对齐
            uintptr_t addr = reinterpret_cast<uintptr_t>(ptr);
            bool aligned = (addr % sizeof(void*)) == 0;
            
            state.counters["Aligned"] = aligned ? 1 : 0;
            deallocate_memory(ptr);
        }
        benchmark::DoNotOptimize(ptr);
    }
}

BENCHMARK_MAIN();