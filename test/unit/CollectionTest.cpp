#include <gtest/gtest.h>
#include "starry/stdlib/Collection.h"
#include <string>

using namespace starry::stdlib;

class CollectionTest : public ::testing::Test {
protected:
    void SetUp() override {}
    void TearDown() override {}
};

// Array测试
TEST_F(CollectionTest, ArrayBasicOperations) {
    Array<int> arr;
    
    EXPECT_TRUE(arr.empty());
    EXPECT_EQ(arr.size(), 0);
    
    arr.push(1);
    arr.push(2);
    arr.push(3);
    
    EXPECT_FALSE(arr.empty());
    EXPECT_EQ(arr.size(), 3);
    EXPECT_EQ(arr.get(0), 1);
    EXPECT_EQ(arr.get(1), 2);
    EXPECT_EQ(arr.get(2), 3);
}

TEST_F(CollectionTest, ArrayIndexOperator) {
    Array<int> arr;
    arr.push(10);
    arr.push(20);
    arr.push(30);
    
    EXPECT_EQ(arr[0], 10);
    EXPECT_EQ(arr[1], 20);
    EXPECT_EQ(arr[2], 30);
    
    arr[1] = 25;
    EXPECT_EQ(arr[1], 25);
}

TEST_F(CollectionTest, ArrayPopOperation) {
    Array<int> arr;
    arr.push(1);
    arr.push(2);
    arr.push(3);
    
    EXPECT_EQ(arr.pop(), 3);
    EXPECT_EQ(arr.size(), 2);
    EXPECT_EQ(arr.pop(), 2);
    EXPECT_EQ(arr.pop(), 1);
    EXPECT_TRUE(arr.empty());
}

TEST_F(CollectionTest, ArrayInsertOperation) {
    Array<int> arr;
    arr.push(1);
    arr.push(3);
    
    arr.insert(1, 2);
    
    EXPECT_EQ(arr.size(), 3);
    EXPECT_EQ(arr[0], 1);
    EXPECT_EQ(arr[1], 2);
    EXPECT_EQ(arr[2], 3);
}

TEST_F(CollectionTest, ArrayRemoveOperation) {
    Array<int> arr;
    arr.push(1);
    arr.push(2);
    arr.push(3);
    
    arr.remove(1);
    
    EXPECT_EQ(arr.size(), 2);
    EXPECT_EQ(arr[0], 1);
    EXPECT_EQ(arr[1], 3);
}

TEST_F(CollectionTest, ArrayOutOfBounds) {
    Array<int> arr;
    arr.push(1);
    
    EXPECT_THROW(arr.get(1), std::out_of_range);
    EXPECT_THROW(arr.set(1, 10), std::out_of_range);
    EXPECT_THROW(arr.remove(1), std::out_of_range);
}

TEST_F(CollectionTest, ArrayCopyConstructor) {
    Array<int> arr1;
    arr1.push(1);
    arr1.push(2);
    arr1.push(3);
    
    Array<int> arr2(arr1);
    
    EXPECT_EQ(arr2.size(), 3);
    EXPECT_EQ(arr2[0], 1);
    EXPECT_EQ(arr2[1], 2);
    EXPECT_EQ(arr2[2], 3);
    
    // 修改原数组不应影响副本
    arr1[0] = 10;
    EXPECT_EQ(arr2[0], 1);
}

// LinkedList测试
TEST_F(CollectionTest, LinkedListBasicOperations) {
    LinkedList<int> list;
    
    EXPECT_TRUE(list.empty());
    EXPECT_EQ(list.size(), 0);
    
    list.pushBack(1);
    list.pushBack(2);
    list.pushBack(3);
    
    EXPECT_FALSE(list.empty());
    EXPECT_EQ(list.size(), 3);
}

TEST_F(CollectionTest, LinkedListFrontOperations) {
    LinkedList<int> list;
    
    list.pushFront(1);
    list.pushFront(2);
    list.pushFront(3);
    
    EXPECT_EQ(list.size(), 3);
    EXPECT_EQ(list.popFront(), 3);
    EXPECT_EQ(list.popFront(), 2);
    EXPECT_EQ(list.popFront(), 1);
    EXPECT_TRUE(list.empty());
}

TEST_F(CollectionTest, LinkedListBackOperations) {
    LinkedList<int> list;
    
    list.pushBack(1);
    list.pushBack(2);
    list.pushBack(3);
    
    EXPECT_EQ(list.size(), 3);
    EXPECT_EQ(list.popBack(), 3);
    EXPECT_EQ(list.popBack(), 2);
    EXPECT_EQ(list.popBack(), 1);
    EXPECT_TRUE(list.empty());
}

TEST_F(CollectionTest, LinkedListMixedOperations) {
    LinkedList<int> list;
    
    list.pushBack(2);
    list.pushFront(1);
    list.pushBack(3);
    
    EXPECT_EQ(list.popFront(), 1);
    EXPECT_EQ(list.popBack(), 3);
    EXPECT_EQ(list.popFront(), 2);
    EXPECT_TRUE(list.empty());
}

TEST_F(CollectionTest, LinkedListEmptyOperations) {
    LinkedList<int> list;
    
    EXPECT_THROW(list.popFront(), std::runtime_error);
    EXPECT_THROW(list.popBack(), std::runtime_error);
}

// HashMap测试
TEST_F(CollectionTest, HashMapBasicOperations) {
    HashMap<std::string, int> map;
    
    EXPECT_TRUE(map.empty());
    EXPECT_EQ(map.size(), 0);
    
    map.put("one", 1);
    map.put("two", 2);
    map.put("three", 3);
    
    EXPECT_FALSE(map.empty());
    EXPECT_EQ(map.size(), 3);
    
    EXPECT_EQ(map.get("one"), 1);
    EXPECT_EQ(map.get("two"), 2);
    EXPECT_EQ(map.get("three"), 3);
}

TEST_F(CollectionTest, HashMapContains) {
    HashMap<std::string, int> map;
    
    map.put("key1", 100);
    map.put("key2", 200);
    
    EXPECT_TRUE(map.contains("key1"));
    EXPECT_TRUE(map.contains("key2"));
    EXPECT_FALSE(map.contains("key3"));
}

TEST_F(CollectionTest, HashMapUpdate) {
    HashMap<std::string, int> map;
    
    map.put("key", 100);
    EXPECT_EQ(map.get("key"), 100);
    
    map.put("key", 200);
    EXPECT_EQ(map.get("key"), 200);
    EXPECT_EQ(map.size(), 1); // 大小不应该改变
}

TEST_F(CollectionTest, HashMapRemove) {
    HashMap<std::string, int> map;
    
    map.put("key1", 100);
    map.put("key2", 200);
    
    EXPECT_EQ(map.size(), 2);
    
    map.remove("key1");
    EXPECT_EQ(map.size(), 1);
    EXPECT_FALSE(map.contains("key1"));
    EXPECT_TRUE(map.contains("key2"));
}

TEST_F(CollectionTest, HashMapNonExistentKey) {
    HashMap<std::string, int> map;
    
    EXPECT_THROW(map.get("nonexistent"), std::runtime_error);
    EXPECT_THROW(map.remove("nonexistent"), std::runtime_error);
}

// Set测试
TEST_F(CollectionTest, SetBasicOperations) {
    Set<int> set;
    
    EXPECT_TRUE(set.empty());
    EXPECT_EQ(set.size(), 0);
    
    set.add(1);
    set.add(2);
    set.add(3);
    
    EXPECT_FALSE(set.empty());
    EXPECT_EQ(set.size(), 3);
    
    EXPECT_TRUE(set.contains(1));
    EXPECT_TRUE(set.contains(2));
    EXPECT_TRUE(set.contains(3));
    EXPECT_FALSE(set.contains(4));
}

TEST_F(CollectionTest, SetDuplicateElements) {
    Set<int> set;
    
    set.add(1);
    set.add(1);
    set.add(1);
    
    EXPECT_EQ(set.size(), 1);
    EXPECT_TRUE(set.contains(1));
}

TEST_F(CollectionTest, SetRemove) {
    Set<int> set;
    
    set.add(1);
    set.add(2);
    set.add(3);
    
    set.remove(2);
    
    EXPECT_EQ(set.size(), 2);
    EXPECT_TRUE(set.contains(1));
    EXPECT_FALSE(set.contains(2));
    EXPECT_TRUE(set.contains(3));
}

TEST_F(CollectionTest, SetToVector) {
    Set<int> set;
    
    set.add(3);
    set.add(1);
    set.add(2);
    
    auto vec = set.toVector();
    
    EXPECT_EQ(vec.size(), 3);
    // Set中的元素是有序的
    EXPECT_EQ(vec[0], 1);
    EXPECT_EQ(vec[1], 2);
    EXPECT_EQ(vec[2], 3);
}

TEST_F(CollectionTest, SetClear) {
    Set<int> set;
    
    set.add(1);
    set.add(2);
    set.add(3);
    
    EXPECT_EQ(set.size(), 3);
    
    set.clear();
    
    EXPECT_TRUE(set.empty());
    EXPECT_EQ(set.size(), 0);
}

// 字符串集合测试
TEST_F(CollectionTest, StringCollections) {
    Array<std::string> strArray;
    strArray.push("hello");
    strArray.push("world");
    
    EXPECT_EQ(strArray[0], "hello");
    EXPECT_EQ(strArray[1], "world");
    
    HashMap<std::string, std::string> strMap;
    strMap.put("greeting", "hello");
    strMap.put("target", "world");
    
    EXPECT_EQ(strMap.get("greeting"), "hello");
    EXPECT_EQ(strMap.get("target"), "world");
    
    Set<std::string> strSet;
    strSet.add("unique");
    strSet.add("values");
    strSet.add("unique"); // 重复值
    
    EXPECT_EQ(strSet.size(), 2);
    EXPECT_TRUE(strSet.contains("unique"));
    EXPECT_TRUE(strSet.contains("values"));
}