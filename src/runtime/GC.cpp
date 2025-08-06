#include "starry/runtime/GC.h"
#include "starry/runtime/Memory.h"
#include <algorithm>
#include <chrono>
#include <thread>

namespace starry {
namespace runtime {

// GarbageCollector 实现
GarbageCollector::GarbageCollector() 
    : isRunning_(false), 
      totalAllocated_(0),
      totalCollected_(0),
      collectionCount_(0),
      gcThreshold_(1024 * 1024), // 1MB
      maxHeapSize_(64 * 1024 * 1024), // 64MB
      generationCount_(3) {
    
    // 初始化分代
    generations_.resize(generationCount_);
    for (size_t i = 0; i < generationCount_; ++i) {
        generations_[i] = std::make_unique<Generation>(i);
    }
}

GarbageCollector::~GarbageCollector() {
    stop();
}

void GarbageCollector::start() {
    if (isRunning_) return;
    
    isRunning_ = true;
    gcThread_ = std::thread(&GarbageCollector::gcLoop, this);
}

void GarbageCollector::stop() {
    if (!isRunning_) return;
    
    isRunning_ = false;
    gcCondition_.notify_all();
    
    if (gcThread_.joinable()) {
        gcThread_.join();
    }
}

void* GarbageCollector::allocate(size_t size, ObjectType type) {
    std::lock_guard<std::mutex> lock(gcMutex_);
    
    // 检查是否需要触发GC
    if (shouldTriggerGC()) {
        triggerGC();
    }
    
    // 在年轻代分配对象
    void* ptr = generations_[0]->allocate(size, type);
    if (ptr) {
        totalAllocated_ += size;
        
        // 创建对象元数据
        ObjectMetadata metadata;
        metadata.size = size;
        metadata.type = type;
        metadata.generation = 0;
        metadata.marked = false;
        metadata.refCount = 1;
        metadata.allocTime = std::chrono::steady_clock::now();
        
        objectMetadata_[ptr] = metadata;
        
        // 添加到根集合（简化实现）
        if (type == ObjectType::ROOT) {
            rootSet_.insert(ptr);
        }
    }
    
    return ptr;
}

void GarbageCollector::deallocate(void* ptr) {
    if (!ptr) return;
    
    std::lock_guard<std::mutex> lock(gcMutex_);
    
    auto it = objectMetadata_.find(ptr);
    if (it != objectMetadata_.end()) {
        size_t size = it->second.size;
        size_t generation = it->second.generation;
        
        // 从对应代中释放
        generations_[generation]->deallocate(ptr, size);
        
        // 移除元数据
        objectMetadata_.erase(it);
        
        // 从根集合中移除
        rootSet_.erase(ptr);
        
        totalCollected_ += size;
    }
}

void GarbageCollector::addRoot(void* ptr) {
    std::lock_guard<std::mutex> lock(gcMutex_);
    rootSet_.insert(ptr);
}

void GarbageCollector::removeRoot(void* ptr) {
    std::lock_guard<std::mutex> lock(gcMutex_);
    rootSet_.erase(ptr);
}

void GarbageCollector::collect() {
    std::lock_guard<std::mutex> lock(gcMutex_);
    performCollection();
}

void GarbageCollector::forceFullCollection() {
    std::lock_guard<std::mutex> lock(gcMutex_);
    performFullCollection();
}

GCStats GarbageCollector::getStats() const {
    std::lock_guard<std::mutex> lock(gcMutex_);
    
    GCStats stats;
    stats.totalAllocated = totalAllocated_;
    stats.totalCollected = totalCollected_;
    stats.currentHeapSize = getCurrentHeapSize();
    stats.collectionCount = collectionCount_;
    stats.averageCollectionTime = getAverageCollectionTime();
    
    return stats;
}

void GarbageCollector::setGCThreshold(size_t threshold) {
    std::lock_guard<std::mutex> lock(gcMutex_);
    gcThreshold_ = threshold;
}

void GarbageCollector::setMaxHeapSize(size_t maxSize) {
    std::lock_guard<std::mutex> lock(gcMutex_);
    maxHeapSize_ = maxSize;
}

bool GarbageCollector::shouldTriggerGC() const {
    size_t currentHeapSize = getCurrentHeapSize();
    return currentHeapSize > gcThreshold_ || currentHeapSize > maxHeapSize_ * 0.8;
}

void GarbageCollector::triggerGC() {
    gcCondition_.notify_one();
}

void GarbageCollector::gcLoop() {
    while (isRunning_) {
        std::unique_lock<std::mutex> lock(gcMutex_);
        
        // 等待GC触发条件
        gcCondition_.wait(lock, [this] { 
            return !isRunning_ || shouldTriggerGC(); 
        });
        
        if (!isRunning_) break;
        
        // 执行垃圾回收
        performCollection();
        
        // 休眠一段时间
        lock.unlock();
        std::this_thread::sleep_for(std::chrono::milliseconds(100));
    }
}

void GarbageCollector::performCollection() {
    auto startTime = std::chrono::steady_clock::now();
    
    // 标记阶段
    markPhase();
    
    // 清除阶段
    sweepPhase();
    
    // 压缩阶段（可选）
    if (shouldCompact()) {
        compactPhase();
    }
    
    // 分代提升
    promoteObjects();
    
    auto endTime = std::chrono::steady_clock::now();
    auto duration = std::chrono::duration_cast<std::chrono::microseconds>(endTime - startTime);
    
    collectionTimes_.push_back(duration.count());
    collectionCount_++;
}

void GarbageCollector::performFullCollection() {
    auto startTime = std::chrono::steady_clock::now();
    
    // 对所有代执行完整的标记-清除
    for (auto& generation : generations_) {
        generation->clear();
    }
    
    // 重新分配所有活跃对象到年轻代
    std::unordered_set<void*> liveObjects;
    markLiveObjects(liveObjects);
    
    // 清理死对象
    auto it = objectMetadata_.begin();
    while (it != objectMetadata_.end()) {
        if (liveObjects.find(it->first) == liveObjects.end()) {
            // 死对象，释放内存
            void* ptr = it->first;
            size_t size = it->second.size;
            size_t generation = it->second.generation;
            
            generations_[generation]->deallocate(ptr, size);
            totalCollected_ += size;
            
            it = objectMetadata_.erase(it);
        } else {
            ++it;
        }
    }
    
    auto endTime = std::chrono::steady_clock::now();
    auto duration = std::chrono::duration_cast<std::chrono::microseconds>(endTime - startTime);
    
    collectionTimes_.push_back(duration.count());
    collectionCount_++;
}

void GarbageCollector::markPhase() {
    // 清除所有标记
    for (auto& pair : objectMetadata_) {
        pair.second.marked = false;
    }
    
    // 从根集合开始标记
    for (void* root : rootSet_) {
        markObject(root);
    }
}

void GarbageCollector::markObject(void* ptr) {
    if (!ptr) return;
    
    auto it = objectMetadata_.find(ptr);
    if (it == objectMetadata_.end() || it->second.marked) {
        return; // 对象不存在或已标记
    }
    
    // 标记对象
    it->second.marked = true;
    
    // 递归标记引用的对象
    markReferencedObjects(ptr);
}

void GarbageCollector::markReferencedObjects(void* ptr) {
    // 简化实现：假设对象包含指向其他对象的指针
    // 实际实现需要根据对象类型和布局来遍历引用
    
    auto it = objectMetadata_.find(ptr);
    if (it == objectMetadata_.end()) return;
    
    // 根据对象类型处理引用
    switch (it->second.type) {
        case ObjectType::ARRAY:
            markArrayReferences(ptr);
            break;
        case ObjectType::OBJECT:
            markObjectReferences(ptr);
            break;
        case ObjectType::STRING:
            // 字符串通常不包含引用
            break;
        default:
            break;
    }
}

void GarbageCollector::markArrayReferences(void* arrayPtr) {
    // 简化实现：遍历数组元素
    // 实际需要根据数组的具体结构来实现
}

void GarbageCollector::markObjectReferences(void* objectPtr) {
    // 简化实现：遍历对象字段
    // 实际需要根据对象的具体结构来实现
}

void GarbageCollector::sweepPhase() {
    auto it = objectMetadata_.begin();
    while (it != objectMetadata_.end()) {
        if (!it->second.marked) {
            // 未标记的对象，需要回收
            void* ptr = it->first;
            size_t size = it->second.size;
            size_t generation = it->second.generation;
            
            generations_[generation]->deallocate(ptr, size);
            totalCollected_ += size;
            
            it = objectMetadata_.erase(it);
        } else {
            ++it;
        }
    }
}

void GarbageCollector::compactPhase() {
    // 简化实现：压缩堆内存以减少碎片
    for (auto& generation : generations_) {
        generation->compact();
    }
}

void GarbageCollector::promoteObjects() {
    // 将长期存活的对象提升到老年代
    for (auto& pair : objectMetadata_) {
        ObjectMetadata& metadata = pair.second;
        
        if (metadata.generation < generationCount_ - 1) {
            // 检查对象年龄
            auto now = std::chrono::steady_clock::now();
            auto age = std::chrono::duration_cast<std::chrono::seconds>(now - metadata.allocTime);
            
            if (age.count() > 10) { // 10秒后提升到下一代
                promoteObject(pair.first, metadata.generation + 1);
            }
        }
    }
}

void GarbageCollector::promoteObject(void* ptr, size_t newGeneration) {
    auto it = objectMetadata_.find(ptr);
    if (it == objectMetadata_.end() || newGeneration >= generationCount_) {
        return;
    }
    
    ObjectMetadata& metadata = it->second;
    size_t oldGeneration = metadata.generation;
    size_t size = metadata.size;
    
    // 从旧代中移除
    generations_[oldGeneration]->deallocate(ptr, size);
    
    // 在新代中分配
    void* newPtr = generations_[newGeneration]->allocate(size, metadata.type);
    if (newPtr) {
        // 复制数据
        std::memcpy(newPtr, ptr, size);
        
        // 更新元数据
        metadata.generation = newGeneration;
        objectMetadata_[newPtr] = metadata;
        objectMetadata_.erase(it);
        
        // 更新根集合
        if (rootSet_.find(ptr) != rootSet_.end()) {
            rootSet_.erase(ptr);
            rootSet_.insert(newPtr);
        }
    }
}

bool GarbageCollector::shouldCompact() const {
    // 简化实现：当碎片率超过50%时进行压缩
    size_t totalSize = 0;
    size_t usedSize = 0;
    
    for (const auto& generation : generations_) {
        totalSize += generation->getTotalSize();
        usedSize += generation->getUsedSize();
    }
    
    if (totalSize == 0) return false;
    
    double fragmentationRatio = 1.0 - (double)usedSize / totalSize;
    return fragmentationRatio > 0.5;
}

void GarbageCollector::markLiveObjects(std::unordered_set<void*>& liveObjects) {
    // 从根集合开始深度优先搜索
    std::unordered_set<void*> visited;
    
    for (void* root : rootSet_) {
        markLiveObjectsDFS(root, liveObjects, visited);
    }
}

void GarbageCollector::markLiveObjectsDFS(void* ptr, std::unordered_set<void*>& liveObjects, 
                                         std::unordered_set<void*>& visited) {
    if (!ptr || visited.find(ptr) != visited.end()) {
        return;
    }
    
    visited.insert(ptr);
    liveObjects.insert(ptr);
    
    // 递归访问引用的对象
    markReferencedObjects(ptr);
}

size_t GarbageCollector::getCurrentHeapSize() const {
    size_t totalSize = 0;
    for (const auto& generation : generations_) {
        totalSize += generation->getUsedSize();
    }
    return totalSize;
}

double GarbageCollector::getAverageCollectionTime() const {
    if (collectionTimes_.empty()) return 0.0;
    
    double total = 0.0;
    for (double time : collectionTimes_) {
        total += time;
    }
    return total / collectionTimes_.size();
}

// Generation 实现
Generation::Generation(size_t id) 
    : id_(id), totalSize_(0), usedSize_(0), freeBlocks_() {}

Generation::~Generation() {
    clear();
}

void* Generation::allocate(size_t size, ObjectType type) {
    // 简化实现：使用系统malloc
    void* ptr = std::malloc(size);
    if (ptr) {
        allocatedBlocks_[ptr] = size;
        usedSize_ += size;
        totalSize_ = std::max(totalSize_, usedSize_);
    }
    return ptr;
}

void Generation::deallocate(void* ptr, size_t size) {
    if (!ptr) return;
    
    auto it = allocatedBlocks_.find(ptr);
    if (it != allocatedBlocks_.end()) {
        std::free(ptr);
        usedSize_ -= it->second;
        allocatedBlocks_.erase(it);
        
        // 添加到空闲块列表
        freeBlocks_.push_back({ptr, size});
    }
}

void Generation::clear() {
    for (auto& pair : allocatedBlocks_) {
        std::free(pair.first);
    }
    allocatedBlocks_.clear();
    freeBlocks_.clear();
    usedSize_ = 0;
    totalSize_ = 0;
}

void Generation::compact() {
    // 简化实现：重新整理内存布局以减少碎片
    // 实际实现需要移动对象并更新所有引用
    
    std::vector<std::pair<void*, size_t>> blocks;
    for (auto& pair : allocatedBlocks_) {
        blocks.push_back(pair);
    }
    
    // 清理当前分配
    for (auto& pair : allocatedBlocks_) {
        std::free(pair.first);
    }
    allocatedBlocks_.clear();
    
    // 重新分配连续内存
    for (auto& block : blocks) {
        void* newPtr = std::malloc(block.second);
        if (newPtr) {
            allocatedBlocks_[newPtr] = block.second;
        }
    }
    
    freeBlocks_.clear();
}

size_t Generation::getId() const {
    return id_;
}

size_t Generation::getTotalSize() const {
    return totalSize_;
}

size_t Generation::getUsedSize() const {
    return usedSize_;
}

size_t Generation::getFreeSize() const {
    return totalSize_ - usedSize_;
}

// 全局垃圾回收器实例
static std::unique_ptr<GarbageCollector> g_gc;
static std::once_flag g_gc_init_flag;

GarbageCollector& getGlobalGC() {
    std::call_once(g_gc_init_flag, []() {
        g_gc = std::make_unique<GarbageCollector>();
        g_gc->start();
    });
    return *g_gc;
}

void initializeGC() {
    getGlobalGC();
}

void shutdownGC() {
    if (g_gc) {
        g_gc->stop();
        g_gc.reset();
    }
}

// 便利函数
void* gcAlloc(size_t size, ObjectType type) {
    return getGlobalGC().allocate(size, type);
}

void gcFree(void* ptr) {
    getGlobalGC().deallocate(ptr);
}

void gcAddRoot(void* ptr) {
    getGlobalGC().addRoot(ptr);
}

void gcRemoveRoot(void* ptr) {
    getGlobalGC().removeRoot(ptr);
}

void gcCollect() {
    getGlobalGC().collect();
}

void gcForceFullCollection() {
    getGlobalGC().forceFullCollection();
}

GCStats gcGetStats() {
    return getGlobalGC().getStats();
}

} // namespace runtime
} // namespace starry
