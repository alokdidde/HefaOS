#pragma once
/**
 * @file state_store.hpp
 * @brief ECS-based state management for Hefaos
 */

#include <cstdint>
#include <memory>
#include <optional>
#include <span>
#include <type_traits>
#include <vector>

namespace hefaos {

/**
 * Entity identifier with generation for safe reuse
 */
struct EntityId {
    uint32_t index{0};
    uint32_t generation{0};

    [[nodiscard]] bool operator==(const EntityId&) const = default;
};

/**
 * Configuration for StateStore
 */
struct StateStoreConfig {
    size_t max_entities{10000};
    bool enable_versioning{false};
};

/**
 * Central state management using Entity-Component-System pattern
 *
 * Thread-safe for concurrent reads, single writer.
 * Supports component versioning for temporal queries.
 */
class StateStore {
public:
    explicit StateStore(StateStoreConfig config = {});
    ~StateStore();

    // Non-copyable, non-movable
    StateStore(const StateStore&) = delete;
    StateStore& operator=(const StateStore&) = delete;

    // Entity management
    [[nodiscard]] EntityId create_entity();
    void destroy_entity(EntityId id);
    [[nodiscard]] bool is_alive(EntityId id) const;

    // Component access
    template <typename T>
    void set(EntityId id, const T& component);

    template <typename T>
    [[nodiscard]] T* get(EntityId id);

    template <typename T>
    [[nodiscard]] const T* get(EntityId id) const;

    template <typename T>
    void remove(EntityId id);

    template <typename T>
    [[nodiscard]] bool has(EntityId id) const;

    // Queries
    template <typename... Ts>
    [[nodiscard]] std::vector<EntityId> query() const;

    template <typename T, typename F>
    void for_each(F&& callback);

    // Versioning (if enabled)
    template <typename T>
    [[nodiscard]] const T* get_at_version(EntityId id, uint64_t version) const;

private:
    struct Impl;
    std::unique_ptr<Impl> impl_;
};

}  // namespace hefaos
