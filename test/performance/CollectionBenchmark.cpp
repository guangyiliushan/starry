#include <benchmark/benchmark.h>
#include "starry/stdlib/Collection.h"
#include <vector>
#include <unordered_map>
#include <unordered_set>
#include <random>
#include <string>

using namespace starry::stdlib;

// 数组性能基准测试

static void BM_Array_Push(benchmark::State& state) {
    const int size = state.range(0);
    
    for (auto _ : state) {
        Array<int> arr;
        for (int i = 0; i < size; ++i) {
            arr.push(i);
        }
        benchmark::DoNotOptimize(arr);
    }
}

static void BM_StdVector_Push(benchmark::State& state) {
    const int size = state.range(0);
    
    for (auto _ : state) {
        std::vector<int> vec;
        for (int i = 0; i < size; ++i) {
            vec.push_back(i);
        }
        benchmark::DoNotOptimize(vec);
    }
}

static void BM_Array_RandomAccess(benchmark::State& state) {
    const int size = state.range(0);
    Array<int> arr;
    
    // 预填充数组
    for (int i = 0; i < size; ++i) {
        arr.push(i);
    }
    
    std::random_device rd;
    std::mt19937 gen(rd());
    std::uniform_int_distribution<> dis(0, size - 1);
    
    for (auto _ : state) {
        int index = dis(gen);
        benchmark::DoNotOptimize(arr.get(index));
    }
}

static void BM_StdVector_RandomAccess(benchmark::State& state) {
    const int size = state.range(0);
    std::vector<int> vec;
    
    // 预填充向量
    for (int i = 0; i < size; ++i) {
        vec.push_back(i);
    }
    
    std::random_device rd;
    std::mt19937 gen(rd());
    std::uniform_int_distribution<> dis(0, size - 1);
    
    for (auto _ : state) {
        int index = dis(gen);
        benchmark::DoNotOptimize(vec[index]);
    }
}

static void BM_Array_Insert(benchmark::State& state) {
    const int size = state.range(0);
    
    for (auto _ : state) {
        Array<int> arr;
        
        // 先添加一些元素
        for (int i = 0; i < size / 2; ++i) {
            arr.push(i);
        }
        
        // 在中间插入元素
        for (int i = 0; i < size / 2; ++i) {
            arr.insert(arr.size() / 2, i + 1000);
        }
        
        benchmark::DoNotOptimize(arr);
    }
}

static void BM_Array_Remove(benchmark::State& state) {
    const int size = state.range(0);
    
    for (auto _ : state) {
        state.PauseTiming();
        Array<int> arr;
        for (int i = 0; i < size; ++i) {
            arr.push(i);
        }
        state.ResumeTiming();
        
        // 从中间删除元素
        while (arr.size() > size / 2) {
            arr.remove(arr.size() / 2);
        }
        
        benchmark::DoNotOptimize(arr);
    }
}

// 链表性能基准测试

static void BM_LinkedList_PushFront(benchmark::State& state) {
    const int size = state.range(0);
    
    for (auto _ : state) {
        LinkedList<int> list;
        for (int i = 0; i < size; ++i) {
            list.pushFront(i);
        }
        benchmark::DoNotOptimize(list);
    }
}

static void BM_LinkedList_PushBack(benchmark::State& state) {
    const int size = state.range(0);
    
    for (auto _ : state) {
        LinkedList<int> list;
        for (int i = 0; i < size; ++i) {
            list.pushBack(i);
        }
        benchmark::DoNotOptimize(list);
    }
}

static void BM_LinkedList_PopFront(benchmark::State& state) {
    const int size = state.range(0);
    
    for (auto _ : state) {
        state.PauseTiming();
        LinkedList<int> list;
        for (int i = 0; i < size; ++i) {
            list.pushFront(i);
        }
        state.ResumeTiming();
        
        while (!list.empty()) {
            benchmark::DoNotOptimize(list.popFront());
        }
    }
}

static void BM_LinkedList_PopBack(benchmark::State& state) {
    const int size = state.range(0);
    
    for (auto _ : state) {
        state.PauseTiming();
        LinkedList<int> list;
        for (int i = 0; i < size; ++i) {
            list.pushBack(i);
        }
        state.ResumeTiming();
        
        while (!list.empty()) {
            benchmark::DoNotOptimize(list.popBack());
        }
    }
}

// 哈希表性能基准测试

static void BM_HashMap_Put(benchmark::State& state) {
    const int size = state.range(0);
    
    for (auto _ : state) {
        HashMap<std::string, int> map;
        for (int i = 0; i < size; ++i) {
            map.put("key" + std::to_string(i), i);
        }
        benchmark::DoNotOptimize(map);
    }
}

static void BM_StdUnorderedMap_Put(benchmark::State& state) {
    const int size = state.range(0);
    
    for (auto _ : state) {
        std::unordered_map<std::string, int> map;
        for (int i = 0; i < size; ++i) {
            map["key" + std::to_string(i)] = i;
        }
        benchmark::DoNotOptimize(map);
    }
}

static void BM_HashMap_Get(benchmark::State& state) {
    const int size = state.range(0);
    HashMap<std::string, int> map;
    
    // 预填充哈希表
    for (int i = 0; i < size; ++i) {
        map.put("key" + std::to_string(i), i);
    }
    
    std::random_device rd;
    std::mt19937 gen(rd());
    std::uniform_int_distribution<> dis(0, size - 1);
    
    for (auto _ : state) {
        int index = dis(gen);
        std::string key = "key" + std::to_string(index);
        benchmark::DoNotOptimize(map.get(key));
    }
}

static void BM_StdUnorderedMap_Get(benchmark::State& state) {
    const int size = state.range(0);
    std::unordered_map<std::string, int> map;
    
    // 预填充哈希表
    for (int i = 0; i < size; ++i) {
        map["key" + std::to_string(i)] = i;
    }
    
    std::random_device rd;
    std::mt19937 gen(rd());
    std::uniform_int_distribution<> dis(0, size - 1);
    
    for (auto _ : state) {
        int index = dis(gen);
        std::string key = "key" + std::to_string(index);
        benchmark::DoNotOptimize(map[key]);
    }
}

static void BM_HashMap_Contains(benchmark::State& state) {
    const int size = state.range(0);
    HashMap<std::string, int> map;
    
    // 预填充哈希表
    for (int i = 0; i < size; ++i) {
        map.put("key" + std::to_string(i), i);
    }
    
    std::random_device rd;
    std::mt19937 gen(rd());
    std::uniform_int_distribution<> dis(0, size * 2); // 包含不存在的键
    
    for (auto _ : state) {
        int index = dis(gen);
        std::string key = "key" + std::to_string(index);
        benchmark::DoNotOptimize(map.contains(key));
    }
}

static void BM_HashMap_Remove(benchmark::State& state) {
    const int size = state.range(0);
    
    for (auto _ : state) {
        state.PauseTiming();
        HashMap<std::string, int> map;
        for (int i = 0; i < size; ++i) {
            map.put("key" + std::to_string(i), i);
        }
        state.ResumeTiming();
        
        // 删除一半的元素
        for (int i = 0; i < size / 2; ++i) {
            map.remove("key" + std::to_string(i));
        }
        
        benchmark::DoNotOptimize(map);
    }
}

// 集合性能基准测试

static void BM_Set_Add(benchmark::State& state) {
    const int size = state.range(0);
    
    for (auto _ : state) {
        Set<int> set;
        for (int i = 0; i < size; ++i) {
            set.add(i);
        }
        benchmark::DoNotOptimize(set);
    }
}

static void BM_StdUnorderedSet_Add(benchmark::State& state) {
    const int size = state.range(0);
    
    for (auto _ : state) {
        std::unordered_set<int> set;
        for (int i = 0; i < size; ++i) {
            set.insert(i);
        }
        benchmark::DoNotOptimize(set);
    }
}

static void BM_Set_Contains(benchmark::State& state) {
    const int size = state.range(0);
    Set<int> set;
    
    // 预填充集合
    for (int i = 0; i < size; ++i) {
        set.add(i);
    }
    
    std::random_device rd;
    std::mt19937 gen(rd());
    std::uniform_int_distribution<> dis(0, size * 2); // 包含不存在的元素
    
    for (auto _ : state) {
        int value = dis(gen);
        benchmark::DoNotOptimize(set.contains(value));
    }
}

static void BM_Set_Remove(benchmark::State& state) {
    const int size = state.range(0);
    
    for (auto _ : state) {
        state.PauseTiming();
        Set<int> set;
        for (int i = 0; i < size; ++i) {
            set.add(i);
        }
        state.ResumeTiming();
        
        // 删除一半的元素
        for (int i = 0; i < size / 2; ++i) {
            set.remove(i);
        }
        
        benchmark::DoNotOptimize(set);
    }
}

// 混合操作基准测试

static void BM_HashMap_MixedOperations(benchmark::State& state) {
    const int size = state.range(0);
    
    std::random_device rd;
    std::mt19937 gen(rd());
    std::uniform_int_distribution<> op_dis(0, 2); // 0=put, 1=get, 2=remove
    std::uniform_int_distribution<> key_dis(0, size * 2);
    
    for (auto _ : state) {
        HashMap<std::string, int> map;
        
        // 预填充一些数据
        for (int i = 0; i < size / 2; ++i) {
            map.put("key" + std::to_string(i), i);
        }
        
        // 执行混合操作
        for (int i = 0; i < size; ++i) {
            int op = op_dis(gen);
            int key_num = key_dis(gen);
            std::string key = "key" + std::to_string(key_num);
            
            switch (op) {
                case 0: // put
                    map.put(key, key_num);
                    break;
                case 1: // get
                    if (map.contains(key)) {
                        benchmark::DoNotOptimize(map.get(key));
                    }
                    break;
                case 2: // remove
                    if (map.contains(key)) {
                        map.remove(key);
                    }
                    break;
            }
        }
        
        benchmark::DoNotOptimize(map);
    }
}

static void BM_Array_StringOperations(benchmark::State& state) {
    const int size = state.range(0);
    
    for (auto _ : state) {
        Array<std::string> arr;
        
        // 添加字符串
        for (int i = 0; i < size; ++i) {
            arr.push("string_" + std::to_string(i));
        }
        
        // 访问字符串
        for (int i = 0; i < size; ++i) {
            benchmark::DoNotOptimize(arr.get(i));
        }
        
        benchmark::DoNotOptimize(arr);
    }
}

// 注册基准测试
BENCHMARK(BM_Array_Push)->Range(1000, 100000);
BENCHMARK(BM_StdVector_Push)->Range(1000, 100000);
BENCHMARK(BM_Array_RandomAccess)->Range(1000, 100000);
BENCHMARK(BM_StdVector_RandomAccess)->Range(1000, 100000);
BENCHMARK(BM_Array_Insert)->Range(100, 10000);
BENCHMARK(BM_Array_Remove)->Range(100, 10000);

BENCHMARK(BM_LinkedList_PushFront)->Range(1000, 100000);
BENCHMARK(BM_LinkedList_PushBack)->Range(1000, 100000);
BENCHMARK(BM_LinkedList_PopFront)->Range(1000, 100000);
BENCHMARK(BM_LinkedList_PopBack)->Range(1000, 100000);

BENCHMARK(BM_HashMap_Put)->Range(1000, 100000);
BENCHMARK(BM_StdUnorderedMap_Put)->Range(1000, 100000);
BENCHMARK(BM_HashMap_Get)->Range(1000, 100000);
BENCHMARK(BM_StdUnorderedMap_Get)->Range(1000, 100000);
BENCHMARK(BM_HashMap_Contains)->Range(1000, 100000);
BENCHMARK(BM_HashMap_Remove)->Range(1000, 100000);

BENCHMARK(BM_Set_Add)->Range(1000, 100000);
BENCHMARK(BM_StdUnorderedSet_Add)->Range(1000, 100000);
BENCHMARK(BM_Set_Contains)->Range(1000, 100000);
BENCHMARK(BM_Set_Remove)->Range(1000, 100000);

BENCHMARK(BM_HashMap_MixedOperations)->Range(1000, 50000);
BENCHMARK(BM_Array_StringOperations)->Range(1000, 50000);

BENCHMARK_MAIN();