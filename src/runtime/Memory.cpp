/**
 * @file Memory.cpp
 * @brief Starry语言运行时内存管理实现
 * @author Starry Team
 * @date 2024
 */

#include "starry/runtime/Memory.h"
#include <cstdlib>
#include <cstring>
#include <stdexcept>

namespace starry {
namespace runtime {

// 内存池实现
class MemoryPool {
private:
    struct Block {
        size_t size;
        bool is_free;
        Block* next;
        char data[];
    };
    
    Block* head;
    size_t total_size;
    
public:
    MemoryPool(size_t size) : head(nullptr), total_size(size) {
        head = static_cast<Block*>(std::malloc(size));
        if (!head) {
            throw std::bad_alloc();
        }
        head->size = size - sizeof(Block);
        head->is_free = true;
        head->next = nullptr;
    }
    
    ~MemoryPool() {
        if (head) {
            std::free(head);
        }
    }
    
    void* allocate(size_t size) {
        Block* current = head;
        while (current) {
            if (current->is_free && current->size >= size) {
                current->is_free = false;
                return current->data;
            }
            current = current->next;
        }
        return nullptr;
    }
    
    void deallocate(void* ptr) {
        if (!ptr) return;
        
        Block* current = head;
        while (current) {
            if (current->data == ptr) {
                current->is_free = true;
                return;
            }
            current = current->next;
        }
    }
};

// 全局内存池
static MemoryPool* g_memory_pool = nullptr;

void initialize_memory(size_t pool_size) {
    if (!g_memory_pool) {
        g_memory_pool = new MemoryPool(pool_size);
    }
}

void cleanup_memory() {
    delete g_memory_pool;
    g_memory_pool = nullptr;
}

void* allocate_memory(size_t size) {
    if (!g_memory_pool) {
        initialize_memory(1024 * 1024); // 默认1MB
    }
    return g_memory_pool->allocate(size);
}

void deallocate_memory(void* ptr) {
    if (g_memory_pool) {
        g_memory_pool->deallocate(ptr);
    }
}

} // namespace runtime
} // namespace starry