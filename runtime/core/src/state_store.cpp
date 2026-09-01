/**
 * @file state_store.cpp
 * @brief StateStore implementation
 */

#include "hefaos/state_store.hpp"

namespace hefaos {

struct StateStore::Impl {
    StateStoreConfig config;
    // TODO: Implement component storage
};

StateStore::StateStore(StateStoreConfig config)
    : impl_(std::make_unique<Impl>()) {
    impl_->config = config;
}

StateStore::~StateStore() = default;

EntityId StateStore::create_entity() {
    // TODO: Implement entity creation
    return EntityId{0, 0};
}

void StateStore::destroy_entity(EntityId /*id*/) {
    // TODO: Implement entity destruction
}

bool StateStore::is_alive(EntityId /*id*/) const {
    // TODO: Implement alive check
    return false;
}

}  // namespace hefaos
