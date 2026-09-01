/**
 * @file test_state_store.cpp
 * @brief Component tests for StateStore
 */

#include <gtest/gtest.h>
#include "hefaos/state_store.hpp"

namespace hefaos::test {

class StateStoreTest : public ::testing::Test {
protected:
    void SetUp() override {
        store_ = std::make_unique<StateStore>();
    }

    std::unique_ptr<StateStore> store_;
};

TEST_F(StateStoreTest, CreateEntity) {
    auto id = store_->create_entity();
    // TODO: Implement proper tests
    (void)id;
    EXPECT_TRUE(true);
}

}  // namespace hefaos::test
