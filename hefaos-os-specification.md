# HefaOS: AI-Native Robotics Operating System

## Technical Specification Document

**Version:** 1.0.0  
**Status:** Draft  
**Last Updated:** January 2026  
**Authors:** HefaOS Team

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Design Principles](#2-design-principles)
3. [System Architecture](#3-system-architecture)
4. [Core Components](#4-core-components)
   - 4.1 [State Store](#41-state-store)
   - 4.2 [Task Graph Scheduler](#42-task-graph-scheduler)
   - 4.3 [Executor Pool](#43-executor-pool)
   - 4.4 [IPC Layer](#44-ipc-layer)
   - 4.5 [AI Runtime](#45-ai-runtime)
   - 4.6 [Hardware Abstraction Layer](#46-hardware-abstraction-layer)
5. [Data Architecture](#5-data-architecture)
6. [Timing and Scheduling](#6-timing-and-scheduling)
7. [Memory Management](#7-memory-management)
8. [Safety Architecture](#8-safety-architecture)
9. [API Specification](#9-api-specification)
10. [Configuration](#10-configuration)
11. [Build System](#11-build-system)
12. [Deployment](#12-deployment)
13. [Testing Strategy](#13-testing-strategy)
14. [Migration from ROS2](#14-migration-from-ros2)
15. [Appendices](#15-appendices)

---

## 1. Executive Summary

### 1.1 Purpose

HefaOS is an AI-native robotics operating system designed for the era of intelligent machines. It provides a hybrid architecture that combines the cache efficiency of Entity-Component-System (ECS) data layouts with explicit task graph scheduling, enabling robots to run ML models as first-class components while maintaining real-time guarantees.

### 1.2 Key Differentiators

| Feature | ROS2 | HefaOS |
|---------|------|----------|
| Latency | 10-100ms (DDS) | <1ms (zero-copy IPC) |
| Scalability | ~200 nodes | 10,000+ entities |
| ML Integration | Add-on | Native, hierarchical |
| Memory Safety | Runtime errors | Compile-time verification |
| Data Layout | Object-oriented | Columnar (Arrow-backed) |
| Scheduling | Implicit (DDS) | Explicit DAG with priorities |

### 1.3 Target Applications

- Humanoid robots
- Mobile manipulation platforms
- Autonomous vehicles
- Industrial automation
- Multi-robot fleets
- Research and education

### 1.4 Design Goals

1. **Portability:** Same binary runs on any ARM Linux board
2. **Developer Experience:** Familiar pipeline-based mental model
3. **Real-Time Capability:** Layered timing guarantees (soft RT to hard RT)
4. **AI-Native:** ML models are first-class scheduled components
5. **Performance:** 10-100x improvement over ROS2 for typical workloads

---

## 2. Design Principles

### 2.1 Hybrid Architecture Philosophy

HefaOS rejects the false dichotomy between ECS and traditional pipelines. Instead, it combines:

- **From ECS:** Cache-efficient columnar data storage, component composition
- **From Task Graphs:** Explicit dependencies, deterministic scheduling
- **From Actor Model:** Process isolation, fault tolerance
- **From Reactive Systems:** Backpressure, streaming data handling

### 2.2 Core Principles

#### 2.2.1 Explicit Over Implicit

All data flow and execution order must be explicitly declared. No hidden dependencies.

```
GOOD: task_a.depends_on(task_b)  // Clear dependency
BAD:  task_a reads global state   // Hidden coupling
```

#### 2.2.2 Data-Oriented Design

Organize data for how it's processed, not how it's conceptualized.

```
GOOD: struct Positions { float x[1000]; float y[1000]; }  // Cache-friendly
BAD:  struct Entity { float x; float y; } entities[1000]; // Cache-hostile
```

#### 2.2.3 Zero-Copy by Default

Data should flow through the system without serialization or copying.

```
GOOD: share_buffer(ptr, size)     // Zero-copy handoff
BAD:  serialize(data) -> send()   // Unnecessary copy
```

#### 2.2.4 Fail-Safe Degradation

System must remain safe even when components fail or miss deadlines.

```
IF perception_timeout THEN
  use_last_known_state()
  reduce_velocity()
  alert_operator()
```

#### 2.2.5 Separation of Concerns

Timing requirements, data layout, and business logic are independent concerns.

```
@task(period=1ms, deadline=800us, priority=0)
def control_loop(state: RobotState) -> MotorCommand:
    # Business logic only, timing handled by framework
    return compute_control(state)
```

---

## 3. System Architecture

### 3.1 High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                         USER SPACE                                   │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  ┌────────────────────────────────────────────────────────────────┐ │
│  │                    Application Layer                            │ │
│  │                                                                 │ │
│  │   ┌──────────────┐  ┌──────────────┐  ┌──────────────┐         │ │
│  │   │   Behavior   │  │     LLM      │  │    User      │         │ │
│  │   │    Trees     │  │    Agents    │  │   Scripts    │         │ │
│  │   └──────────────┘  └──────────────┘  └──────────────┘         │ │
│  │                                                                 │ │
│  └────────────────────────────────────────────────────────────────┘ │
│                              │                                       │
│                         iceoryx IPC                                  │
│                              │                                       │
│  ┌────────────────────────────────────────────────────────────────┐ │
│  │                      AI Runtime Layer                           │ │
│  │                                                                 │ │
│  │   ┌──────────────┐  ┌──────────────┐  ┌──────────────┐         │ │
│  │   │ ONNX Runtime │  │   TFLite     │  │  llama.cpp   │         │ │
│  │   │ (Perception) │  │  (Control)   │  │ (Reasoning)  │         │ │
│  │   └──────────────┘  └──────────────┘  └──────────────┘         │ │
│  │                                                                 │ │
│  └────────────────────────────────────────────────────────────────┘ │
│                              │                                       │
│  ┌────────────────────────────────────────────────────────────────┐ │
│  │                   Task Graph Scheduler                          │ │
│  │                                                                 │ │
│  │   ┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐  │ │
│  │   │  Control Loop   │ │ Perception Loop │ │  Planning Loop  │  │ │
│  │   │     1 kHz       │ │     30 Hz       │ │     10 Hz       │  │ │
│  │   │   Priority 0    │ │   Priority 1    │ │   Priority 2    │  │ │
│  │   └─────────────────┘ └─────────────────┘ └─────────────────┘  │ │
│  │                                                                 │ │
│  └────────────────────────────────────────────────────────────────┘ │
│                              │                                       │
│  ┌────────────────────────────────────────────────────────────────┐ │
│  │                       State Store                               │ │
│  │                                                                 │ │
│  │   ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐      │ │
│  │   │  IMU   │ │ Joints │ │ Camera │ │Estimate│ │  Plan  │      │ │
│  │   │ States │ │ States │ │ Frames │ │ States │ │ States │      │ │
│  │   └────────┘ └────────┘ └────────┘ └────────┘ └────────┘      │ │
│  │                                                                 │ │
│  │              Arrow-backed columnar storage                      │ │
│  └────────────────────────────────────────────────────────────────┘ │
│                              │                                       │
│  ┌────────────────────────────────────────────────────────────────┐ │
│  │                Hardware Abstraction Layer                       │ │
│  │                                                                 │ │
│  │   ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐      │ │
│  │   │ GPIO   │ │ Serial │ │  CAN   │ │Camera  │ │ LiDAR  │      │ │
│  │   │(libgpiod)│ │(termios)│ │(SocketCAN)│ │ (V4L2) │ │(custom)│      │ │
│  │   └────────┘ └────────┘ └────────┘ └────────┘ └────────┘      │ │
│  │                                                                 │ │
│  └────────────────────────────────────────────────────────────────┘ │
│                                                                      │
├─────────────────────────────────────────────────────────────────────┤
│                         KERNEL SPACE                                 │
│                                                                      │
│                  Linux 6.x + PREEMPT_RT (mainline)                   │
│                                                                      │
│   CPU Isolation: isolcpus=2,3 nohz_full=2,3 rcu_nocbs=2,3          │
│                                                                      │
├─────────────────────────────────────────────────────────────────────┤
│                          HARDWARE                                    │
│                                                                      │
│   ┌─────────────────────────────┐    ┌─────────────────────────┐   │
│   │     ARM Application         │    │    Safety MCU           │   │
│   │     Processor (SoC)         │────│    (STM32/RP2040)       │   │
│   │                             │    │                         │   │
│   │  Core 0-1: System/AI        │    │  • Motor PWM (10kHz)    │   │
│   │  Core 2-3: RT Tasks         │    │  • E-Stop (<10μs)       │   │
│   │                             │    │  • Watchdog             │   │
│   └─────────────────────────────┘    └─────────────────────────┘   │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

### 3.2 Process Architecture

HefaOS uses a multi-process architecture for fault isolation:

```
┌─────────────────────────────────────────────────────────────────────┐
│                        Process Layout                                │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│   ┌─────────────────┐     Shared Memory (hugetlbfs)                 │
│   │   hefaos-core    │◄───────────────────────────────────┐          │
│   │                 │                                     │          │
│   │ • State Store   │     ┌─────────────────┐            │          │
│   │ • Scheduler     │     │  hefaos-control  │◄───────────┤          │
│   │ • IPC Router    │     │                 │            │          │
│   └─────────────────┘     │ • RT Tasks      │            │          │
│           │               │ • Motor Control │            │          │
│           │               │ • Safety Monitor│            │          │
│      iceoryx              └─────────────────┘            │          │
│           │                                              │          │
│           │               ┌─────────────────┐            │          │
│           └──────────────►│ hefaos-perception│◄───────────┤          │
│           │               │                 │            │          │
│           │               │ • Camera Pipeline│           │          │
│           │               │ • Object Detection│          │          │
│           │               │ • SLAM          │            │          │
│           │               └─────────────────┘            │          │
│           │                                              │          │
│           │               ┌─────────────────┐            │          │
│           └──────────────►│ hefaos-planning  │◄───────────┘          │
│                           │                 │                        │
│                           │ • Path Planning │                        │
│                           │ • Task Planning │                        │
│                           │ • LLM Agents    │                        │
│                           └─────────────────┘                        │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

### 3.3 Threading Model

Each process follows a consistent threading model:

| Thread Type | Purpose | Scheduling | CPU Affinity |
|-------------|---------|------------|--------------|
| Main Thread | Initialization, shutdown | SCHED_OTHER | System cores |
| RT Worker | Real-time task execution | SCHED_FIFO | Isolated cores |
| AI Worker | ML inference | SCHED_OTHER | System cores (GPU) |
| IO Thread | Sensor/actuator I/O | SCHED_FIFO | System cores |
| IPC Thread | Message routing | SCHED_OTHER | System cores |

---

## 4. Core Components

### 4.1 State Store

The State Store provides typed, cache-efficient storage for all robot state data. It uses ECS-inspired data layout without ECS execution semantics.

#### 4.1.1 Design Goals

- Zero-copy access to component data
- Cache-friendly memory layout (Structure-of-Arrays)
- Type-safe queries with compile-time verification
- Efficient batch operations for multi-robot scenarios
- Integration with Apache Arrow for interoperability

#### 4.1.2 Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                          State Store                                 │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│   ┌─────────────────────────────────────────────────────────────┐   │
│   │                     Schema Registry                          │   │
│   │                                                              │   │
│   │   ComponentType -> Arrow Schema                              │   │
│   │   • IMUReading: {accel: vec3, gyro: vec3, timestamp: u64}   │   │
│   │   • JointState: {position: f64, velocity: f64, effort: f64} │   │
│   │   • CameraFrame: {data: binary, width: u32, height: u32}    │   │
│   └─────────────────────────────────────────────────────────────┘   │
│                              │                                       │
│   ┌─────────────────────────────────────────────────────────────┐   │
│   │                    Component Tables                          │   │
│   │                                                              │   │
│   │   ┌─────────────┐ ┌─────────────┐ ┌─────────────┐          │   │
│   │   │ IMUReading  │ │ JointState  │ │ CameraFrame │          │   │
│   │   │   Table     │ │   Table     │ │   Table     │          │   │
│   │   │             │ │             │ │             │          │   │
│   │   │ entity_id[] │ │ entity_id[] │ │ entity_id[] │          │   │
│   │   │ accel_x[]   │ │ position[]  │ │ data[]      │          │   │
│   │   │ accel_y[]   │ │ velocity[]  │ │ width[]     │          │   │
│   │   │ accel_z[]   │ │ effort[]    │ │ height[]    │          │   │
│   │   │ gyro_x[]    │ │ timestamp[] │ │ timestamp[] │          │   │
│   │   │ gyro_y[]    │ │             │ │             │          │   │
│   │   │ gyro_z[]    │ │             │ │             │          │   │
│   │   │ timestamp[] │ │             │ │             │          │   │
│   │   └─────────────┘ └─────────────┘ └─────────────┘          │   │
│   │                                                              │   │
│   │              Arrow RecordBatch (columnar)                    │   │
│   └─────────────────────────────────────────────────────────────┘   │
│                              │                                       │
│   ┌─────────────────────────────────────────────────────────────┐   │
│   │                     Entity Index                             │   │
│   │                                                              │   │
│   │   EntityId -> { table_index, row_index } per component      │   │
│   │                                                              │   │
│   │   robot_0: IMU@row[0], Joints@row[0..6], Camera@row[0]      │   │
│   │   robot_1: IMU@row[1], Joints@row[6..12], Camera@row[1]     │   │
│   └─────────────────────────────────────────────────────────────┘   │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

#### 4.1.3 Core Types

```cpp
// Entity identifier
struct EntityId {
    uint32_t index;      // Dense index for array access
    uint32_t generation; // Version to detect stale references
};

// Component metadata
struct ComponentInfo {
    std::type_index type;
    std::shared_ptr<arrow::Schema> schema;
    size_t size_bytes;
    size_t alignment;
};

// State store interface
class StateStore {
public:
    // Entity management
    EntityId create_entity();
    void destroy_entity(EntityId id);
    bool is_alive(EntityId id) const;
    
    // Component access (single entity)
    template<typename T>
    T* get(EntityId id);
    
    template<typename T>
    const T* get(EntityId id) const;
    
    template<typename T>
    void set(EntityId id, const T& value);
    
    template<typename T>
    bool has(EntityId id) const;
    
    template<typename T>
    void remove(EntityId id);
    
    // Batch access (all entities with component)
    template<typename T>
    ComponentView<T> get_all();
    
    template<typename T>
    arrow::ChunkedArray* get_column(const std::string& field);
    
    // Multi-component queries
    template<typename... Ts>
    QueryResult<Ts...> query();
    
    // Serialization
    std::shared_ptr<arrow::RecordBatch> to_arrow() const;
    void from_arrow(const arrow::RecordBatch& batch);
};
```

#### 4.1.4 Built-in Component Types

```cpp
// Spatial components
struct Position { double x, y, z; };
struct Orientation { double w, x, y, z; };  // Quaternion
struct Pose { Position position; Orientation orientation; };
struct Twist { Vector3 linear; Vector3 angular; };
struct Wrench { Vector3 force; Vector3 torque; };

// Sensor components
struct IMUReading {
    Vector3 linear_acceleration;
    Vector3 angular_velocity;
    uint64_t timestamp_ns;
    uint32_t sequence;
};

struct JointState {
    double position;
    double velocity;
    double effort;
    uint64_t timestamp_ns;
};

struct CameraFrame {
    std::span<const uint8_t> data;  // Zero-copy view
    uint32_t width;
    uint32_t height;
    uint32_t encoding;  // RGB8, BGR8, MONO8, etc.
    uint64_t timestamp_ns;
};

struct PointCloud {
    std::span<const float> points;  // x,y,z interleaved
    std::span<const uint8_t> colors; // Optional RGB
    uint32_t num_points;
    uint64_t timestamp_ns;
};

// Control components
struct MotorCommand {
    double position_target;
    double velocity_target;
    double effort_limit;
    uint8_t control_mode;  // Position, Velocity, Effort
};

struct TrajectoryPoint {
    std::array<double, 7> positions;
    std::array<double, 7> velocities;
    std::array<double, 7> accelerations;
    uint64_t time_from_start_ns;
};

// State estimation components
struct StateEstimate {
    Pose pose;
    Twist twist;
    std::array<double, 36> covariance;  // 6x6 covariance matrix
    uint64_t timestamp_ns;
};

// Perception components
struct Detection {
    uint32_t class_id;
    float confidence;
    BoundingBox bbox;
    std::optional<Pose> pose_3d;
};

struct OccupancyGrid {
    std::span<const int8_t> data;  // -1: unknown, 0-100: probability
    double resolution;
    uint32_t width;
    uint32_t height;
    Pose origin;
};
```

#### 4.1.5 Thread Safety

The State Store uses a multi-version concurrency control (MVCC) approach:

```cpp
// Read operations are lock-free
const IMUReading* imu = store.get<IMUReading>(robot_id);  // No lock

// Write operations use per-component locks
store.set<IMUReading>(robot_id, new_reading);  // Acquires IMU table lock

// Batch operations use copy-on-write
auto all_joints = store.get_all<JointState>();  // Returns immutable snapshot
```

---

### 4.2 Task Graph Scheduler

The Task Graph Scheduler manages execution of all robot tasks with explicit dependencies and timing constraints.

#### 4.2.1 Design Goals

- Explicit dependency declaration (no hidden coupling)
- Priority-based scheduling with deadline awareness
- Automatic parallelization where safe
- Support for heterogeneous rates (1kHz control, 30Hz perception)
- Graceful deadline miss handling

#### 4.2.2 Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                      Task Graph Scheduler                            │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│   ┌─────────────────────────────────────────────────────────────┐   │
│   │                      Task Registry                           │   │
│   │                                                              │   │
│   │   TaskId -> TaskDefinition                                   │   │
│   │   • name, execute_fn, period, deadline, priority            │   │
│   │   • dependencies[], resource_requirements                    │   │
│   └─────────────────────────────────────────────────────────────┘   │
│                              │                                       │
│   ┌─────────────────────────────────────────────────────────────┐   │
│   │                    Dependency Graph                          │   │
│   │                                                              │   │
│   │         ┌─────────┐         ┌─────────┐                     │   │
│   │         │IMU_Read │         │Enc_Read │                     │   │
│   │         └────┬────┘         └────┬────┘                     │   │
│   │              │                   │                           │   │
│   │              └─────────┬─────────┘                           │   │
│   │                        │                                     │   │
│   │                        ▼                                     │   │
│   │                 ┌─────────────┐                              │   │
│   │                 │State_Estimate│                             │   │
│   │                 └──────┬──────┘                              │   │
│   │                        │                                     │   │
│   │              ┌─────────┴─────────┐                           │   │
│   │              │                   │                           │   │
│   │              ▼                   ▼                           │   │
│   │       ┌───────────┐      ┌─────────────┐                    │   │
│   │       │  Control  │      │Safety_Check │                    │   │
│   │       └─────┬─────┘      └─────────────┘                    │   │
│   │             │                                                │   │
│   │             ▼                                                │   │
│   │       ┌───────────┐                                         │   │
│   │       │Motor_Send │                                         │   │
│   │       └───────────┘                                         │   │
│   │                                                              │   │
│   └─────────────────────────────────────────────────────────────┘   │
│                              │                                       │
│   ┌─────────────────────────────────────────────────────────────┐   │
│   │                  Rate Group Manager                          │   │
│   │                                                              │   │
│   │   ┌─────────────┐ ┌─────────────┐ ┌─────────────┐          │   │
│   │   │ 1kHz Group  │ │ 100Hz Group │ │ 30Hz Group  │          │   │
│   │   │             │ │             │ │             │          │   │
│   │   │ IMU_Read    │ │ Encoder_Read│ │ Camera_Read │          │   │
│   │   │ State_Est   │ │ Force_Read  │ │ Detection   │          │   │
│   │   │ Control     │ │             │ │ Planning    │          │   │
│   │   │ Motor_Send  │ │             │ │             │          │   │
│   │   └─────────────┘ └─────────────┘ └─────────────┘          │   │
│   │                                                              │   │
│   └─────────────────────────────────────────────────────────────┘   │
│                              │                                       │
│   ┌─────────────────────────────────────────────────────────────┐   │
│   │                   Execution Planner                          │   │
│   │                                                              │   │
│   │   • Topological sort of dependency graph                    │   │
│   │   • Identify parallel execution opportunities               │   │
│   │   • Assign tasks to executor threads                        │   │
│   │   • Generate execution timeline                             │   │
│   └─────────────────────────────────────────────────────────────┘   │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

#### 4.2.3 Core Types

```cpp
// Task identifier
using TaskId = uint32_t;

// Priority levels (lower = higher priority)
enum class Priority : uint8_t {
    SAFETY = 0,      // Safety-critical (e-stop, collision avoidance)
    CONTROL = 1,     // Real-time control loops
    PERCEPTION = 2,  // Sensor processing
    PLANNING = 3,    // Motion/task planning
    BACKGROUND = 4   // Logging, diagnostics
};

// Resource requirements
struct ResourceRequirements {
    bool requires_gpu = false;
    bool requires_network = false;
    bool realtime_critical = false;
    size_t memory_estimate_bytes = 0;
};

// Task timing constraints
struct TimingConstraints {
    std::chrono::nanoseconds period;        // Execution rate
    std::chrono::nanoseconds deadline;      // Maximum latency
    std::chrono::nanoseconds wcet_estimate; // Worst-case execution time
};

// Task definition
struct TaskDefinition {
    std::string name;
    std::function<void()> execute;
    TimingConstraints timing;
    Priority priority;
    ResourceRequirements resources;
    std::vector<TaskId> dependencies;
    
    // Optional callbacks
    std::function<void()> on_deadline_miss;
    std::function<void(std::exception&)> on_error;
};

// Task runtime statistics
struct TaskStats {
    uint64_t execution_count;
    std::chrono::nanoseconds total_time;
    std::chrono::nanoseconds min_time;
    std::chrono::nanoseconds max_time;
    std::chrono::nanoseconds avg_time;
    uint64_t deadline_misses;
    uint64_t error_count;
};

// Task graph interface
class TaskGraph {
public:
    // Task management
    TaskId add_task(TaskDefinition def);
    void remove_task(TaskId id);
    void update_task(TaskId id, TaskDefinition def);
    
    // Dependency management
    void add_dependency(TaskId from, TaskId to);
    void remove_dependency(TaskId from, TaskId to);
    
    // Graph operations
    ExecutionPlan compile();
    bool validate() const;  // Check for cycles, missing deps
    
    // Introspection
    TaskStats get_stats(TaskId id) const;
    std::string to_dot() const;  // GraphViz format
};
```

#### 4.2.4 Execution Plan

The compiled execution plan optimizes task placement:

```cpp
struct ExecutionPlan {
    // Tasks grouped by rate
    struct RateGroup {
        std::chrono::nanoseconds period;
        std::vector<TaskId> tasks;  // Topologically sorted
        std::vector<std::pair<TaskId, TaskId>> parallel_pairs;
    };
    std::vector<RateGroup> rate_groups;
    
    // Thread assignment
    struct ThreadAssignment {
        TaskId task;
        uint32_t thread_id;
        bool pinned_to_rt_core;
    };
    std::vector<ThreadAssignment> assignments;
    
    // Timing budget
    struct TimingBudget {
        TaskId task;
        std::chrono::nanoseconds start_offset;
        std::chrono::nanoseconds allocated_time;
    };
    std::vector<TimingBudget> budgets;
};
```

#### 4.2.5 Example Usage

```cpp
TaskGraph graph;

// Define tasks
auto imu_task = graph.add_task({
    .name = "IMU_Read",
    .execute = [&]() {
        auto reading = imu_driver.read();
        state_store.set<IMUReading>(robot_id, reading);
    },
    .timing = {
        .period = 1ms,
        .deadline = 500us,
        .wcet_estimate = 100us
    },
    .priority = Priority::CONTROL,
    .resources = { .realtime_critical = true }
});

auto encoder_task = graph.add_task({
    .name = "Encoder_Read",
    .execute = [&]() {
        auto joints = encoder_driver.read();
        state_store.set<JointStates>(robot_id, joints);
    },
    .timing = {
        .period = 1ms,
        .deadline = 500us,
        .wcet_estimate = 50us
    },
    .priority = Priority::CONTROL,
    .resources = { .realtime_critical = true }
});

auto estimate_task = graph.add_task({
    .name = "State_Estimate",
    .execute = [&]() {
        auto imu = state_store.get<IMUReading>(robot_id);
        auto joints = state_store.get<JointStates>(robot_id);
        auto estimate = estimator.update(*imu, *joints);
        state_store.set<StateEstimate>(robot_id, estimate);
    },
    .timing = {
        .period = 1ms,
        .deadline = 700us,
        .wcet_estimate = 150us
    },
    .priority = Priority::CONTROL,
    .resources = { .realtime_critical = true }
});

auto control_task = graph.add_task({
    .name = "Control",
    .execute = [&]() {
        auto estimate = state_store.get<StateEstimate>(robot_id);
        auto command = controller.compute(*estimate);
        motor_driver.send(command);
    },
    .timing = {
        .period = 1ms,
        .deadline = 900us,
        .wcet_estimate = 100us
    },
    .priority = Priority::CONTROL,
    .resources = { .realtime_critical = true }
});

// Define dependencies
graph.add_dependency(imu_task, estimate_task);
graph.add_dependency(encoder_task, estimate_task);
graph.add_dependency(estimate_task, control_task);

// Compile and validate
auto plan = graph.compile();
assert(graph.validate());

// Execute
executor.run(plan);
```

---

### 4.3 Executor Pool

The Executor Pool manages thread allocation and task execution.

#### 4.3.1 Design Goals

- CPU core isolation for real-time tasks
- Priority-based thread scheduling
- Efficient work stealing for load balancing
- Deadline-aware task preemption

#### 4.3.2 Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                         Executor Pool                                │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│   ┌─────────────────────────────────────────────────────────────┐   │
│   │                    Thread Manager                            │   │
│   │                                                              │   │
│   │   ┌───────────────────┐    ┌───────────────────┐           │   │
│   │   │   RT Thread Pool  │    │   Worker Thread   │           │   │
│   │   │                   │    │       Pool        │           │   │
│   │   │  Thread 0: Core 2 │    │                   │           │   │
│   │   │  Thread 1: Core 3 │    │  Thread 0: Any    │           │   │
│   │   │                   │    │  Thread 1: Any    │           │   │
│   │   │  SCHED_FIFO (99)  │    │  Thread 2: Any    │           │   │
│   │   │  mlockall()       │    │  Thread 3: Any    │           │   │
│   │   │                   │    │                   │           │   │
│   │   │  [RT Task Queue]  │    │  SCHED_OTHER      │           │   │
│   │   └───────────────────┘    │  [Work Queue]     │           │   │
│   │                            └───────────────────┘           │   │
│   └─────────────────────────────────────────────────────────────┘   │
│                              │                                       │
│   ┌─────────────────────────────────────────────────────────────┐   │
│   │                     Rate Timers                              │   │
│   │                                                              │   │
│   │   ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐          │   │
│   │   │ 1kHz    │ │ 500Hz   │ │ 100Hz   │ │ 30Hz    │          │   │
│   │   │ Timer   │ │ Timer   │ │ Timer   │ │ Timer   │          │   │
│   │   └─────────┘ └─────────┘ └─────────┘ └─────────┘          │   │
│   │                                                              │   │
│   │   timerfd + CLOCK_MONOTONIC for precision                   │   │
│   └─────────────────────────────────────────────────────────────┘   │
│                              │                                       │
│   ┌─────────────────────────────────────────────────────────────┐   │
│   │                  Deadline Monitor                            │   │
│   │                                                              │   │
│   │   • Tracks task start/completion times                      │   │
│   │   • Triggers on_deadline_miss callbacks                     │   │
│   │   • Logs timing violations                                  │   │
│   │   • Adaptive rate reduction on persistent misses            │   │
│   └─────────────────────────────────────────────────────────────┘   │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

#### 4.3.3 Core Types

```cpp
// Thread configuration
struct ThreadConfig {
    std::string name;
    int cpu_affinity = -1;          // -1 = no affinity
    int scheduling_policy = SCHED_OTHER;
    int scheduling_priority = 0;    // 0-99 for SCHED_FIFO
    bool lock_memory = false;       // mlockall()
    size_t stack_size = 0;          // 0 = default
};

// Executor configuration
struct ExecutorConfig {
    uint32_t rt_thread_count = 2;
    uint32_t worker_thread_count = 4;
    std::vector<int> rt_cpu_cores = {2, 3};
    std::vector<int> worker_cpu_cores = {};  // Empty = any
    bool enable_work_stealing = true;
};

// Executor interface
class Executor {
public:
    explicit Executor(ExecutorConfig config);
    ~Executor();
    
    // Lifecycle
    void start();
    void stop();
    bool is_running() const;
    
    // Execution
    void run(ExecutionPlan& plan);
    void run_once(ExecutionPlan& plan);  // Single iteration
    
    // Async task submission (non-scheduled)
    std::future<void> submit(std::function<void()> task);
    
    // Introspection
    std::vector<ThreadStats> get_thread_stats() const;
    ExecutorMetrics get_metrics() const;
};

// Executor metrics
struct ExecutorMetrics {
    uint64_t tasks_executed;
    uint64_t deadline_misses;
    double cpu_utilization;
    std::chrono::nanoseconds avg_loop_time;
    std::chrono::nanoseconds max_loop_time;
};
```

#### 4.3.4 Real-Time Thread Setup

```cpp
void setup_rt_thread(const ThreadConfig& config) {
    // Set thread name
    pthread_setname_np(pthread_self(), config.name.c_str());
    
    // Set CPU affinity
    if (config.cpu_affinity >= 0) {
        cpu_set_t cpuset;
        CPU_ZERO(&cpuset);
        CPU_SET(config.cpu_affinity, &cpuset);
        pthread_setaffinity_np(pthread_self(), sizeof(cpuset), &cpuset);
    }
    
    // Set scheduling policy and priority
    if (config.scheduling_policy == SCHED_FIFO || 
        config.scheduling_policy == SCHED_RR) {
        struct sched_param param;
        param.sched_priority = config.scheduling_priority;
        pthread_setschedparam(pthread_self(), config.scheduling_policy, &param);
    }
    
    // Lock memory (prevent page faults)
    if (config.lock_memory) {
        mlockall(MCL_CURRENT | MCL_FUTURE);
    }
    
    // Pre-fault stack
    volatile char stack_touch[config.stack_size];
    memset((void*)stack_touch, 0, sizeof(stack_touch));
}
```

---

### 4.4 IPC Layer

The IPC Layer provides zero-copy inter-process communication using iceoryx.

#### 4.4.1 Design Goals

- True zero-copy message passing
- Type-safe pub/sub with schema validation
- Sub-microsecond latency for local IPC
- Process isolation for fault tolerance

#### 4.4.2 Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                           IPC Layer                                  │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│   ┌─────────────────────────────────────────────────────────────┐   │
│   │                    iceoryx RouDi                             │   │
│   │                   (Central Daemon)                           │   │
│   │                                                              │   │
│   │   • Manages shared memory segments                          │   │
│   │   • Routes publisher/subscriber discovery                   │   │
│   │   • Monitors process health                                 │   │
│   └─────────────────────────────────────────────────────────────┘   │
│                              │                                       │
│   ┌─────────────────────────────────────────────────────────────┐   │
│   │                  Shared Memory Pool                          │   │
│   │                                                              │   │
│   │   /dev/shm/hefaos (hugetlbfs mounted)                        │   │
│   │                                                              │   │
│   │   ┌─────────────────────────────────────────────────────┐   │   │
│   │   │              Memory Segments                         │   │   │
│   │   │                                                      │   │   │
│   │   │   Segment 1: Control Messages (64KB chunks)         │   │   │
│   │   │   Segment 2: Sensor Data (1MB chunks)               │   │   │
│   │   │   Segment 3: Image Data (16MB chunks)               │   │   │
│   │   │   Segment 4: Point Clouds (64MB chunks)             │   │   │
│   │   └─────────────────────────────────────────────────────┘   │   │
│   │                                                              │   │
│   └─────────────────────────────────────────────────────────────┘   │
│                              │                                       │
│   ┌─────────────────────────────────────────────────────────────┐   │
│   │                   Topic Registry                             │   │
│   │                                                              │   │
│   │   Topic                   Type            Publisher/Subscriber│   │
│   │   ─────────────────────────────────────────────────────────│   │
│   │   /robot/imu             IMUReading       control → *       │   │
│   │   /robot/joints          JointStates      control → *       │   │
│   │   /robot/estimate        StateEstimate    control → planning│   │
│   │   /robot/camera/rgb      CameraFrame      perception → *    │   │
│   │   /robot/detections      Detections       perception → plan │   │
│   │   /robot/command         MotorCommand     planning → control│   │
│   └─────────────────────────────────────────────────────────────┘   │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

#### 4.4.3 Core Types

```cpp
// Topic definition
template<typename T>
struct Topic {
    static constexpr const char* name();
    static constexpr const char* service();
    static constexpr const char* instance();
    using MessageType = T;
};

// Publisher interface
template<typename T>
class Publisher {
public:
    explicit Publisher(const Topic<T>& topic);
    
    // Loan memory for zero-copy publish
    std::optional<T*> loan();
    void publish(T* sample);
    
    // Copy publish (when loan not needed)
    void publish(const T& data);
    
    // Statistics
    uint64_t published_count() const;
    bool has_subscribers() const;
};

// Subscriber interface
template<typename T>
class Subscriber {
public:
    explicit Subscriber(const Topic<T>& topic);
    
    // Non-blocking receive
    std::optional<const T*> take();
    
    // Blocking receive with timeout
    std::optional<const T*> take(std::chrono::milliseconds timeout);
    
    // Callback-based (integrates with executor)
    void set_callback(std::function<void(const T&)> callback);
    
    // Statistics
    uint64_t received_count() const;
    uint64_t dropped_count() const;
};

// Service (request-response) interface
template<typename Request, typename Response>
class Service {
public:
    explicit Service(const std::string& name);
    
    void set_handler(std::function<Response(const Request&)> handler);
};

template<typename Request, typename Response>
class Client {
public:
    explicit Client(const std::string& name);
    
    std::future<Response> call(const Request& request);
    std::optional<Response> call(const Request& request, 
                                  std::chrono::milliseconds timeout);
};
```

#### 4.4.4 Message Definitions (FlatBuffers)

```flatbuffers
// schemas/imu_reading.fbs
namespace hefaos.msgs;

struct Vector3 {
    x: float64;
    y: float64;
    z: float64;
}

table IMUReading {
    linear_acceleration: Vector3;
    angular_velocity: Vector3;
    timestamp_ns: uint64;
    sequence: uint32;
}

root_type IMUReading;
```

```flatbuffers
// schemas/joint_state.fbs
namespace hefaos.msgs;

table JointState {
    position: float64;
    velocity: float64;
    effort: float64;
    timestamp_ns: uint64;
}

table JointStates {
    joints: [JointState];
    names: [string];
}

root_type JointStates;
```

#### 4.4.5 iceoryx Configuration

```toml
# /etc/hefaos/roudi_config.toml

[general]
version = 1

[log]
level = "warn"

# Memory pool configuration
[[segment]]
reader_group = "hefaos"
writer_group = "hefaos"

# Small messages (IMU, commands)
[[segment.mempool]]
size = 128
count = 1000

# Medium messages (joint states)
[[segment.mempool]]
size = 1024
count = 500

# Large messages (detections, plans)
[[segment.mempool]]
size = 65536
count = 100

# Images
[[segment.mempool]]
size = 1048576
count = 30

# Point clouds
[[segment.mempool]]
size = 16777216
count = 10
```

---

### 4.5 AI Runtime

The AI Runtime provides hierarchical scheduling of ML models across multiple inference engines.

#### 4.5.1 Design Goals

- Unified interface across inference backends
- Priority-based model scheduling
- Deadline-aware inference (skip if too late)
- Efficient batching where applicable
- Hot model swapping without restart

#### 4.5.2 Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                          AI Runtime                                  │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│   ┌─────────────────────────────────────────────────────────────┐   │
│   │                    Model Registry                            │   │
│   │                                                              │   │
│   │   ModelId -> ModelInfo                                       │   │
│   │   • path, backend, input_shapes, output_shapes              │   │
│   │   • priority, deadline, warmup_count                        │   │
│   └─────────────────────────────────────────────────────────────┘   │
│                              │                                       │
│   ┌─────────────────────────────────────────────────────────────┐   │
│   │                  Inference Scheduler                         │   │
│   │                                                              │   │
│   │   Priority Queue (by deadline)                              │   │
│   │   ┌─────────────────────────────────────────────────────┐   │   │
│   │   │  P0: SafetyNet (1kHz, must complete)                │   │   │
│   │   │  P1: ControlPolicy (1kHz, soft deadline)            │   │   │
│   │   │  P2: ObjectDetection (30Hz)                         │   │   │
│   │   │  P3: Segmentation (10Hz)                            │   │   │
│   │   │  P4: LLMAgent (on-demand)                           │   │   │
│   │   └─────────────────────────────────────────────────────┘   │   │
│   │                                                              │   │
│   │   Batching: Combine similar requests                        │   │
│   │   Preemption: Yield GPU for higher priority                 │   │
│   └─────────────────────────────────────────────────────────────┘   │
│                              │                                       │
│   ┌─────────────────────────────────────────────────────────────┐   │
│   │                  Backend Adapters                            │   │
│   │                                                              │   │
│   │   ┌─────────────┐ ┌─────────────┐ ┌─────────────┐          │   │
│   │   │ONNX Runtime │ │  TFLite     │ │ llama.cpp   │          │   │
│   │   │             │ │             │ │             │          │   │
│   │   │ CPU/GPU/NPU │ │ CPU/XNNPACK │ │ CPU (GGML)  │          │   │
│   │   │             │ │             │ │             │          │   │
│   │   │ Perception  │ │ Control     │ │ Reasoning   │          │   │
│   │   │ Models      │ │ Models      │ │ Models      │          │   │
│   │   └─────────────┘ └─────────────┘ └─────────────┘          │   │
│   │                                                              │   │
│   │   ┌─────────────┐ ┌─────────────┐                          │   │
│   │   │ TensorRT    │ │  Custom     │                          │   │
│   │   │ (Jetson)    │ │ (C/ASM)     │                          │   │
│   │   │             │ │             │                          │   │
│   │   │ GPU Accel   │ │ Safety      │                          │   │
│   │   │ Vision      │ │ Critical    │                          │   │
│   │   └─────────────┘ └─────────────┘                          │   │
│   │                                                              │   │
│   └─────────────────────────────────────────────────────────────┘   │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

#### 4.5.3 Priority Bands

| Band | Name | Frequency | Examples | Backend | Constraints |
|------|------|-----------|----------|---------|-------------|
| P0 | Safety | 1-10 kHz | Collision check, joint limits | Custom C | No allocation, <100μs |
| P1 | Control | 100-1000 Hz | Impedance control, visual servoing | TFLite | Pre-allocated, <1ms |
| P2 | Perception | 10-100 Hz | Object detection, pose estimation | ONNX/TensorRT | Can use GPU, <50ms |
| P3 | Planning | 1-10 Hz | Motion planning, task planning | ONNX | Can batch, <200ms |
| P4 | Reasoning | <1 Hz | LLM agents, high-level decisions | llama.cpp | Best effort |

#### 4.5.4 Core Types

```cpp
// Model identifier
using ModelId = uint32_t;

// Inference priority
enum class InferencePriority : uint8_t {
    SAFETY = 0,
    CONTROL = 1,
    PERCEPTION = 2,
    PLANNING = 3,
    REASONING = 4
};

// Backend type
enum class Backend : uint8_t {
    ONNX_RUNTIME,
    TFLITE,
    TENSORRT,
    LLAMA_CPP,
    CUSTOM
};

// Tensor descriptor
struct TensorDesc {
    std::string name;
    std::vector<int64_t> shape;
    DataType dtype;  // FLOAT32, FLOAT16, INT8, etc.
};

// Model configuration
struct ModelConfig {
    std::string name;
    std::string path;
    Backend backend;
    InferencePriority priority;
    std::chrono::microseconds deadline;
    
    std::vector<TensorDesc> inputs;
    std::vector<TensorDesc> outputs;
    
    // Optimization hints
    bool enable_fp16 = false;
    bool enable_int8 = false;
    int num_threads = 1;
    int warmup_iterations = 10;
};

// Inference request
struct InferenceRequest {
    ModelId model;
    std::vector<Tensor> inputs;
    std::chrono::steady_clock::time_point deadline;
    std::function<void(std::vector<Tensor>)> callback;
};

// AI Runtime interface
class AIRuntime {
public:
    // Model management
    ModelId load_model(const ModelConfig& config);
    void unload_model(ModelId id);
    void reload_model(ModelId id, const std::string& new_path);
    
    // Synchronous inference
    std::vector<Tensor> infer(ModelId model, 
                               const std::vector<Tensor>& inputs);
    
    // Async inference with deadline
    std::future<std::vector<Tensor>> infer_async(
        ModelId model,
        const std::vector<Tensor>& inputs,
        std::chrono::microseconds deadline);
    
    // Batch inference
    std::vector<std::vector<Tensor>> infer_batch(
        ModelId model,
        const std::vector<std::vector<Tensor>>& batch_inputs);
    
    // Statistics
    ModelStats get_stats(ModelId id) const;
};

// Model statistics
struct ModelStats {
    uint64_t inference_count;
    uint64_t deadline_misses;
    std::chrono::microseconds avg_latency;
    std::chrono::microseconds p99_latency;
    std::chrono::microseconds max_latency;
    size_t memory_usage_bytes;
};
```

#### 4.5.5 Example Usage

```cpp
AIRuntime runtime;

// Load perception model
auto detection_model = runtime.load_model({
    .name = "yolov8s",
    .path = "/models/yolov8s.onnx",
    .backend = Backend::ONNX_RUNTIME,
    .priority = InferencePriority::PERCEPTION,
    .deadline = 30ms,
    .inputs = {{ "images", {1, 3, 640, 640}, DataType::FLOAT32 }},
    .outputs = {{ "output0", {1, 84, 8400}, DataType::FLOAT32 }},
    .enable_fp16 = true
});

// Load control model
auto control_model = runtime.load_model({
    .name = "impedance_policy",
    .path = "/models/impedance.tflite",
    .backend = Backend::TFLITE,
    .priority = InferencePriority::CONTROL,
    .deadline = 800us,
    .inputs = {{ "state", {1, 14}, DataType::FLOAT32 }},
    .outputs = {{ "torque", {1, 7}, DataType::FLOAT32 }},
    .num_threads = 1  // Single thread for predictability
});

// Load LLM for reasoning
auto llm_model = runtime.load_model({
    .name = "llama3-8b",
    .path = "/models/llama3-8b-q4.gguf",
    .backend = Backend::LLAMA_CPP,
    .priority = InferencePriority::REASONING,
    .deadline = 5000ms  // Best effort
});

// Use in task graph
graph.add_task({
    .name = "ObjectDetection",
    .execute = [&]() {
        auto frame = state_store.get<CameraFrame>(robot_id);
        auto input = preprocess_image(frame);
        auto output = runtime.infer(detection_model, {input});
        auto detections = postprocess_detections(output);
        state_store.set<Detections>(robot_id, detections);
    },
    .timing = { .period = 33ms, .deadline = 30ms },
    .priority = Priority::PERCEPTION,
    .resources = { .requires_gpu = true }
});
```

---

### 4.6 Hardware Abstraction Layer

The HAL provides unified access to sensors and actuators across different hardware platforms.

#### 4.6.1 Design Goals

- Consistent API across hardware variants
- Zero-copy sensor data where possible
- Support for common robotics interfaces
- Hot-pluggable device support

#### 4.6.2 Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                    Hardware Abstraction Layer                        │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│   ┌─────────────────────────────────────────────────────────────┐   │
│   │                    Device Manager                            │   │
│   │                                                              │   │
│   │   • Device discovery (udev rules)                           │   │
│   │   • Hot-plug handling                                       │   │
│   │   • Resource allocation                                     │   │
│   └─────────────────────────────────────────────────────────────┘   │
│                              │                                       │
│   ┌─────────────────────────────────────────────────────────────┐   │
│   │                    Driver Registry                           │   │
│   │                                                              │   │
│   │   DeviceType -> DriverFactory                               │   │
│   │   • IMU: BMI088Driver, ICM42688Driver, MPU6050Driver        │   │
│   │   • Camera: V4L2Driver, RealSenseDriver, ZEDDriver          │   │
│   │   • Motor: ODriveDriver, CyberGearDriver, DynamixelDriver   │   │
│   │   • LiDAR: VelodyneDriver, OusterDriver, LivoxDriver        │   │
│   └─────────────────────────────────────────────────────────────┘   │
│                              │                                       │
│   ┌─────────────────────────────────────────────────────────────┐   │
│   │                    Interface Layers                          │   │
│   │                                                              │   │
│   │   ┌─────────────┐ ┌─────────────┐ ┌─────────────┐          │   │
│   │   │    GPIO     │ │   Serial    │ │     I2C     │          │   │
│   │   │  (libgpiod) │ │  (termios)  │ │  (i2c-dev)  │          │   │
│   │   └─────────────┘ └─────────────┘ └─────────────┘          │   │
│   │                                                              │   │
│   │   ┌─────────────┐ ┌─────────────┐ ┌─────────────┐          │   │
│   │   │     SPI     │ │     CAN     │ │   Ethernet  │          │   │
│   │   │  (spidev)   │ │ (SocketCAN) │ │  (sockets)  │          │   │
│   │   └─────────────┘ └─────────────┘ └─────────────┘          │   │
│   │                                                              │   │
│   │   ┌─────────────┐ ┌─────────────┐ ┌─────────────┐          │   │
│   │   │    USB      │ │    V4L2     │ │   EtherCAT  │          │   │
│   │   │  (libusb)   │ │  (cameras)  │ │   (SOEM)    │          │   │
│   │   └─────────────┘ └─────────────┘ └─────────────┘          │   │
│   │                                                              │   │
│   └─────────────────────────────────────────────────────────────┘   │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

#### 4.6.3 Core Interfaces

```cpp
// Base device interface
class Device {
public:
    virtual ~Device() = default;
    
    virtual std::string name() const = 0;
    virtual DeviceType type() const = 0;
    virtual DeviceStatus status() const = 0;
    
    virtual bool open() = 0;
    virtual void close() = 0;
    virtual bool is_open() const = 0;
};

// IMU interface
class IMU : public Device {
public:
    DeviceType type() const override { return DeviceType::IMU; }
    
    virtual IMUReading read() = 0;
    virtual void set_sample_rate(uint32_t hz) = 0;
    virtual void set_accel_range(AccelRange range) = 0;
    virtual void set_gyro_range(GyroRange range) = 0;
    virtual void calibrate() = 0;
};

// Camera interface
class Camera : public Device {
public:
    DeviceType type() const override { return DeviceType::CAMERA; }
    
    virtual CameraFrame capture() = 0;
    virtual void set_resolution(uint32_t width, uint32_t height) = 0;
    virtual void set_framerate(uint32_t fps) = 0;
    virtual void set_exposure(float ms) = 0;
    virtual void set_gain(float db) = 0;
    
    // Zero-copy buffer management
    virtual void set_buffer_count(uint32_t count) = 0;
    virtual CameraFrame* dequeue_buffer() = 0;
    virtual void queue_buffer(CameraFrame* frame) = 0;
};

// Motor controller interface
class MotorController : public Device {
public:
    DeviceType type() const override { return DeviceType::MOTOR; }
    
    virtual void set_position(double radians) = 0;
    virtual void set_velocity(double rad_per_sec) = 0;
    virtual void set_torque(double nm) = 0;
    
    virtual double get_position() = 0;
    virtual double get_velocity() = 0;
    virtual double get_torque() = 0;
    
    virtual void set_control_mode(ControlMode mode) = 0;
    virtual void set_gains(double kp, double kd, double ki) = 0;
    virtual void emergency_stop() = 0;
};

// LiDAR interface
class LiDAR : public Device {
public:
    DeviceType type() const override { return DeviceType::LIDAR; }
    
    virtual PointCloud scan() = 0;
    virtual void set_spin_rate(uint32_t rpm) = 0;
    virtual void set_return_mode(ReturnMode mode) = 0;
};

// Force/torque sensor interface
class ForceTorqueSensor : public Device {
public:
    DeviceType type() const override { return DeviceType::FORCE_TORQUE; }
    
    virtual Wrench read() = 0;
    virtual void tare() = 0;
    virtual void set_filter(FilterType type, float cutoff_hz) = 0;
};
```

#### 4.6.4 Safety MCU Interface

For hard real-time requirements, HefaOS delegates to an external safety MCU:

```cpp
// Safety MCU communication protocol
struct SafetyMCUCommand {
    uint8_t sync = 0xAA;
    uint8_t command_type;
    uint8_t motor_id;
    float position_target;
    float velocity_limit;
    float torque_limit;
    uint16_t crc;
};

struct SafetyMCUFeedback {
    uint8_t sync = 0x55;
    uint8_t status;
    uint8_t motor_id;
    float position;
    float velocity;
    float torque;
    uint8_t error_flags;
    uint16_t crc;
};

// Safety MCU interface
class SafetyMCU {
public:
    explicit SafetyMCU(const std::string& serial_port, uint32_t baud_rate);
    
    // Commands
    void send_command(const SafetyMCUCommand& cmd);
    SafetyMCUFeedback read_feedback();
    
    // Safety functions
    void emergency_stop();
    void reset_estop();
    void set_velocity_limit(float max_rad_per_sec);
    void set_torque_limit(float max_nm);
    
    // Health
    bool is_alive() const;  // Watchdog status
    uint32_t get_loop_rate() const;
    std::vector<MotorError> get_errors() const;
};
```

---

## 5. Data Architecture

### 5.1 Message Format Strategy

HefaOS uses a hybrid message format approach:

| Message Type | Format | Use Case |
|--------------|--------|----------|
| High-frequency control | FlatBuffers | IMU, joints, commands (>100Hz) |
| Bulk sensor data | Arrow IPC | Point clouds, images, maps |
| Configuration | YAML/JSON | Parameters, calibration |
| Logs | Arrow + Parquet | Recording, replay |

### 5.2 FlatBuffers Schemas

```flatbuffers
// schemas/robot_state.fbs
namespace hefaos.msgs;

enum ControlMode : byte {
    POSITION = 0,
    VELOCITY = 1,
    TORQUE = 2,
    IMPEDANCE = 3
}

struct Quaternion {
    w: float64;
    x: float64;
    y: float64;
    z: float64;
}

struct Pose {
    position: Vector3;
    orientation: Quaternion;
}

struct Twist {
    linear: Vector3;
    angular: Vector3;
}

table StateEstimate {
    pose: Pose;
    twist: Twist;
    covariance: [float64:36];
    timestamp_ns: uint64;
    frame_id: string;
}

table MotorCommand {
    positions: [float64];
    velocities: [float64];
    efforts: [float64];
    control_mode: ControlMode;
    timestamp_ns: uint64;
}
```

### 5.3 Arrow Schemas

```cpp
// Arrow schema for point clouds
auto point_cloud_schema = arrow::schema({
    arrow::field("x", arrow::float32()),
    arrow::field("y", arrow::float32()),
    arrow::field("z", arrow::float32()),
    arrow::field("intensity", arrow::uint8()),
    arrow::field("ring", arrow::uint8()),
    arrow::field("timestamp", arrow::uint32())
});

// Arrow schema for camera frames
auto camera_frame_schema = arrow::schema({
    arrow::field("data", arrow::large_binary()),
    arrow::field("width", arrow::uint32()),
    arrow::field("height", arrow::uint32()),
    arrow::field("encoding", arrow::utf8()),
    arrow::field("timestamp_ns", arrow::uint64())
});
```

---

## 6. Timing and Scheduling

### 6.1 Timing Budget

For a typical 1kHz control loop:

```
┌─────────────────────────────────────────────────────────────────────┐
│                    1ms Control Loop Budget                           │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│   0μs        200μs       400μs       600μs       800μs      1000μs  │
│   │           │           │           │           │           │     │
│   ├───────────┼───────────┼───────────┼───────────┼───────────┤     │
│   │  IMU Read │  Encoder  │   State   │  Control  │   Motor   │     │
│   │   100μs   │  Read     │  Estimate │  Compute  │   Send    │     │
│   │           │   50μs    │   150μs   │   100μs   │   100μs   │     │
│   │           │           │           │           │           │     │
│   │<─────────────── Total: 500μs ───────────────>│           │     │
│   │                                               │           │     │
│   │                                               │<─ Slack ─>│     │
│   │                                               │   500μs   │     │
│   │                                                           │     │
└─────────────────────────────────────────────────────────────────────┘
```

### 6.2 Deadline Miss Handling

```cpp
// Deadline miss policies
enum class DeadlineMissPolicy {
    LOG_AND_CONTINUE,   // Log warning, execute anyway
    SKIP_ITERATION,     // Skip this iteration
    USE_LAST_OUTPUT,    // Use previous computation result
    TRIGGER_SAFE_MODE,  // Reduce performance, increase safety
    EMERGENCY_STOP      // Halt robot
};

struct DeadlineMissHandler {
    DeadlineMissPolicy policy;
    uint32_t consecutive_miss_threshold;  // Trigger escalation after N misses
    std::function<void(TaskId, uint32_t miss_count)> callback;
};
```

### 6.3 Rate Synchronization

```cpp
// Synchronize multiple rate groups
class RateSynchronizer {
public:
    // Define rate relationships
    void set_base_rate(std::chrono::nanoseconds period);  // Fastest rate
    void add_derived_rate(std::string name, uint32_t divisor);
    
    // Example: 1kHz base, 100Hz perception (divisor=10), 10Hz planning (divisor=100)
    
    // Synchronization points
    void wait_for_tick();  // Block until next base tick
    bool should_run(const std::string& rate_name);  // Check if derived rate due
};
```

---

## 7. Memory Management

### 7.1 Memory Pools

```cpp
// Pre-allocated memory pools for RT-safe operation
class MemoryPool {
public:
    explicit MemoryPool(size_t block_size, size_t block_count);
    
    void* allocate();      // O(1), never fails if pool not exhausted
    void deallocate(void* ptr);  // O(1)
    
    size_t available() const;
    size_t capacity() const;
};

// Global pool configuration
struct MemoryConfig {
    // Control message pool
    size_t control_block_size = 256;
    size_t control_block_count = 1000;
    
    // Sensor data pool
    size_t sensor_block_size = 4096;
    size_t sensor_block_count = 500;
    
    // Image pool
    size_t image_block_size = 1024 * 1024;  // 1MB
    size_t image_block_count = 30;
    
    // Point cloud pool
    size_t pointcloud_block_size = 16 * 1024 * 1024;  // 16MB
    size_t pointcloud_block_count = 10;
};
```

### 7.2 Shared Memory Layout

```
┌─────────────────────────────────────────────────────────────────────┐
│                    Shared Memory Layout                              │
│                    /dev/shm/hefaos                                    │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│   Offset      Size        Purpose                                   │
│   ─────────────────────────────────────────────────────────────     │
│   0x00000000  4KB         Header (magic, version, process table)    │
│   0x00001000  64KB        iceoryx management data                   │
│   0x00011000  1MB         Control message pool                      │
│   0x00111000  4MB         Sensor data pool                          │
│   0x00511000  32MB        Image pool                                │
│   0x02511000  160MB       Point cloud pool                          │
│   0x0C511000  64MB        State store (Arrow tables)                │
│   0x10511000  ...         Extensible region                         │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 8. Safety Architecture

### 8.1 Safety Hierarchy

```
┌─────────────────────────────────────────────────────────────────────┐
│                      Safety Hierarchy                                │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│   Level 0: Hardware E-Stop                                          │
│   ─────────────────────                                              │
│   • Physical button, direct motor cutoff                            │
│   • Latency: <1μs                                                   │
│   • Always available, independent of software                        │
│                                                                      │
│   Level 1: Safety MCU                                                │
│   ────────────────────                                               │
│   • Watchdog timeout (5ms)                                          │
│   • Joint limit monitoring                                          │
│   • Current limiting                                                │
│   • Latency: <100μs                                                 │
│                                                                      │
│   Level 2: RT Safety Monitor                                         │
│   ──────────────────────────                                         │
│   • Velocity limiting                                               │
│   • Collision prediction                                            │
│   • Self-collision avoidance                                        │
│   • Latency: <1ms                                                   │
│                                                                      │
│   Level 3: Planning Safety                                           │
│   ────────────────────────                                           │
│   • Trajectory validation                                           │
│   • Workspace monitoring                                            │
│   • Human detection                                                 │
│   • Latency: <100ms                                                 │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

### 8.2 Watchdog System

```cpp
// Multi-level watchdog
class WatchdogSystem {
public:
    // Register watchdog for a component
    WatchdogId register_watchdog(
        const std::string& name,
        std::chrono::milliseconds timeout,
        std::function<void()> on_timeout
    );
    
    // Pet the watchdog (call regularly)
    void pet(WatchdogId id);
    
    // Global safety state
    SafetyState get_safety_state() const;
    void request_safe_stop();
    void request_emergency_stop();
};

enum class SafetyState {
    NOMINAL,          // All systems operational
    DEGRADED,         // Some non-critical watchdog timeouts
    SAFE_STOP,        // Controlled deceleration to stop
    EMERGENCY_STOP,   // Immediate motor cutoff
    FAULT             // Unrecoverable error
};
```

---

## 9. API Specification

### 9.1 C++ API

```cpp
// Main entry point
namespace hefaos {

// Initialize the runtime
void init(const RuntimeConfig& config);
void shutdown();

// Access global instances
StateStore& state_store();
TaskGraph& task_graph();
Executor& executor();
AIRuntime& ai_runtime();
IPCContext& ipc();

// Convenience macros
#define KAIJU_TASK(name, period_ms, priority) \
    hefaos::task_graph().add_task({ \
        .name = #name, \
        .execute = [&]() { name(); }, \
        .timing = { .period = std::chrono::milliseconds(period_ms) }, \
        .priority = priority \
    })

}  // namespace hefaos
```

### 9.2 Python Bindings

```python
import hefaos

# Initialize
hefaos.init(config_path="/etc/hefaos/robot.yaml")

# Define tasks using decorators
@hefaos.task(period_ms=1, priority=hefaos.Priority.CONTROL)
def control_loop():
    state = hefaos.state_store.get(robot_id, hefaos.StateEstimate)
    command = controller.compute(state)
    hefaos.state_store.set(robot_id, command)

@hefaos.task(period_ms=33, priority=hefaos.Priority.PERCEPTION)
def perception_loop():
    frame = hefaos.state_store.get(robot_id, hefaos.CameraFrame)
    detections = detector.run(frame)
    hefaos.state_store.set(robot_id, detections)

# Run
hefaos.run()
```

### 9.3 CLI Tools

```bash
# System management
hefaos-ctl start          # Start all processes
hefaos-ctl stop           # Graceful shutdown
hefaos-ctl status         # Show system status
hefaos-ctl restart        # Restart all processes

# Introspection
hefaos-topic list         # List all topics
hefaos-topic echo /robot/imu  # Print topic messages
hefaos-topic hz /robot/imu    # Measure publish rate

# Task monitoring
hefaos-task list          # List all tasks
hefaos-task stats         # Show timing statistics
hefaos-task graph         # Output DOT graph

# State inspection
hefaos-state dump         # Dump state store
hefaos-state get robot_0 IMUReading  # Get specific component

# Recording and playback
hefaos-record start -o recording.arrow  # Start recording
hefaos-record stop                       # Stop recording
hefaos-play recording.arrow              # Playback
```

---

## 10. Configuration

### 10.1 System Configuration

```yaml
# /etc/hefaos/system.yaml

runtime:
  name: "hefaos-robot"
  log_level: "info"
  log_path: "/var/log/hefaos"

executor:
  rt_threads: 2
  rt_cores: [2, 3]
  worker_threads: 4
  enable_work_stealing: true

memory:
  shared_memory_size: 256MB
  use_hugepages: true
  
ipc:
  roudi_config: "/etc/hefaos/roudi_config.toml"
```

### 10.2 Robot Configuration

```yaml
# /etc/hefaos/robot.yaml

robot:
  name: "manipulator_01"
  urdf: "/etc/hefaos/robot.urdf"

devices:
  imu:
    type: "bmi088"
    port: "/dev/spidev0.0"
    sample_rate: 1000
    
  encoders:
    type: "as5048a"
    port: "/dev/spidev0.1"
    joints: 7
    
  cameras:
    - name: "wrist_camera"
      type: "v4l2"
      device: "/dev/video0"
      width: 640
      height: 480
      fps: 30
      
  motors:
    type: "safety_mcu"
    port: "/dev/ttyUSB0"
    baud_rate: 1000000
    
tasks:
  control_loop:
    period_ms: 1
    deadline_ms: 0.8
    priority: CONTROL
    
  perception_loop:
    period_ms: 33
    deadline_ms: 30
    priority: PERCEPTION
    
models:
  object_detection:
    path: "/models/yolov8s.onnx"
    backend: onnx
    priority: PERCEPTION
    
  control_policy:
    path: "/models/policy.tflite"
    backend: tflite
    priority: CONTROL
```

---

## 11. Build System

### 11.1 CMake Structure

```cmake
# CMakeLists.txt
cmake_minimum_required(VERSION 3.20)
project(hefaos VERSION 1.0.0 LANGUAGES CXX C)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# Options
option(KAIJU_BUILD_TESTS "Build tests" ON)
option(KAIJU_BUILD_PYTHON "Build Python bindings" ON)
option(KAIJU_ENABLE_TENSORRT "Enable TensorRT backend" OFF)

# Dependencies
find_package(Threads REQUIRED)
find_package(Arrow REQUIRED)
find_package(FlatBuffers REQUIRED)
find_package(iceoryx_posh REQUIRED)
find_package(onnxruntime REQUIRED)

# Core library
add_library(hefaos_core
    src/state_store.cpp
    src/task_graph.cpp
    src/executor.cpp
    src/ipc.cpp
    src/ai_runtime.cpp
)

target_link_libraries(hefaos_core
    PUBLIC
        Arrow::arrow
        iceoryx_posh::iceoryx_posh
    PRIVATE
        Threads::Threads
        onnxruntime::onnxruntime
)

# HAL library
add_library(hefaos_hal
    src/hal/gpio.cpp
    src/hal/serial.cpp
    src/hal/can.cpp
    src/hal/camera.cpp
    src/hal/imu.cpp
)

target_link_libraries(hefaos_hal
    PRIVATE
        hefaos_core
        gpiod
        v4l2
)

# Executables
add_executable(hefaos-core src/main.cpp)
target_link_libraries(hefaos-core hefaos_core hefaos_hal)

# Python bindings
if(KAIJU_BUILD_PYTHON)
    find_package(pybind11 REQUIRED)
    pybind11_add_module(pyhefaos src/python/bindings.cpp)
    target_link_libraries(pyhefaos PRIVATE hefaos_core)
endif()
```

### 11.2 Cross-Compilation

```bash
# Cross-compile for ARM64
cmake -B build-arm64 \
    -DCMAKE_TOOLCHAIN_FILE=cmake/toolchain-aarch64.cmake \
    -DCMAKE_BUILD_TYPE=Release

cmake --build build-arm64 -j$(nproc)

# Create deployment package
cpack -G DEB -B build-arm64
```

### 11.3 Docker Development

```dockerfile
# Dockerfile.dev
FROM ubuntu:24.04

# Install dependencies
RUN apt-get update && apt-get install -y \
    build-essential cmake ninja-build \
    libarrow-dev libflatbuffers-dev \
    libgpiod-dev libv4l-dev \
    python3-dev python3-pip pybind11-dev

# Install iceoryx
RUN git clone https://github.com/eclipse-iceoryx/iceoryx.git && \
    cd iceoryx && cmake -B build -G Ninja && \
    cmake --build build && cmake --install build

# Install ONNX Runtime
RUN pip3 install onnxruntime

WORKDIR /hefaos
```

---

## 12. Deployment

### 12.1 System Service

```ini
# /etc/systemd/system/hefaos.service
[Unit]
Description=HefaOS Robotics
After=network.target

[Service]
Type=simple
User=hefaos
Group=hefaos
ExecStartPre=/usr/bin/hefaos-setup-rt
ExecStart=/usr/bin/hefaos-core --config /etc/hefaos/robot.yaml
Restart=on-failure
RestartSec=5
LimitRTPRIO=99
LimitMEMLOCK=infinity

[Install]
WantedBy=multi-user.target
```

### 12.2 RT Setup Script

```bash
#!/bin/bash
# /usr/bin/hefaos-setup-rt

# Set up CPU isolation
echo "Setting up RT environment..."

# Disable CPU frequency scaling on RT cores
for cpu in 2 3; do
    echo performance > /sys/devices/system/cpu/cpu$cpu/cpufreq/scaling_governor
done

# Set up hugepages
echo 256 > /proc/sys/vm/nr_hugepages

# Create shared memory mount
mount -t hugetlbfs none /dev/hugepages

# Set up iceoryx shared memory
mkdir -p /dev/shm/hefaos
chmod 777 /dev/shm/hefaos

echo "RT environment ready"
```

### 12.3 Board Support

```yaml
# Board-specific configuration auto-detected
# /etc/hefaos/boards/jetson_orin.yaml

board:
  name: "jetson_orin"
  
cpu:
  rt_cores: [4, 5, 6, 7]  # Performance cores
  system_cores: [0, 1, 2, 3]
  
gpu:
  enabled: true
  backend: tensorrt
  
memory:
  total: 32GB
  shared_memory: 4GB
  hugepages: 512
```

---

## 13. Testing Strategy

### 13.1 Unit Tests

```cpp
// test/test_state_store.cpp
#include <gtest/gtest.h>
#include <hefaos/state_store.hpp>

TEST(StateStore, CreateEntity) {
    StateStore store;
    auto id = store.create_entity();
    EXPECT_TRUE(store.is_alive(id));
}

TEST(StateStore, SetGetComponent) {
    StateStore store;
    auto id = store.create_entity();
    
    IMUReading reading{
        .linear_acceleration = {0, 0, 9.81},
        .angular_velocity = {0, 0, 0},
        .timestamp_ns = 12345
    };
    
    store.set<IMUReading>(id, reading);
    
    auto* retrieved = store.get<IMUReading>(id);
    ASSERT_NE(retrieved, nullptr);
    EXPECT_DOUBLE_EQ(retrieved->linear_acceleration.z, 9.81);
}
```

### 13.2 Integration Tests

```cpp
// test/test_control_loop.cpp
TEST(Integration, ControlLoopTiming) {
    hefaos::init(test_config);
    
    std::vector<std::chrono::nanoseconds> loop_times;
    
    hefaos::task_graph().add_task({
        .name = "timing_test",
        .execute = [&]() {
            static auto last = std::chrono::steady_clock::now();
            auto now = std::chrono::steady_clock::now();
            loop_times.push_back(now - last);
            last = now;
        },
        .timing = { .period = 1ms }
    });
    
    hefaos::executor().run_for(std::chrono::seconds(1));
    
    // Verify timing
    auto avg = std::accumulate(loop_times.begin(), loop_times.end(), 
                               std::chrono::nanoseconds(0)) / loop_times.size();
    EXPECT_NEAR(avg.count(), 1000000, 50000);  // 1ms ± 50μs
}
```

### 13.3 Simulation Testing

```python
# test/test_simulation.py
import hefaos
import mujoco

def test_manipulation_task():
    # Load simulation
    sim = mujoco.MjSim("robot.xml")
    
    # Initialize HefaOS with simulation HAL
    hefaos.init(hal_backend="mujoco", sim=sim)
    
    # Run task
    success = run_pick_and_place_task(target_position=[0.5, 0.0, 0.1])
    
    assert success
    assert sim.data.qpos[0] < 0.01  # Verify final position
```

---

## 14. Migration from ROS2

### 14.1 Compatibility Layer

```cpp
// ROS2 message bridge
#include <hefaos/ros2_bridge.hpp>

// Convert ROS2 message to HefaOS
sensor_msgs::msg::Imu ros_imu = ...;
hefaos::IMUReading hefaos_imu = hefaos::from_ros2(ros_imu);

// Convert HefaOS message to ROS2
hefaos::JointStates hefaos_joints = ...;
sensor_msgs::msg::JointState ros_joints = hefaos::to_ros2(hefaos_joints);
```

### 14.2 Migration Steps

1. **Phase 1: Bridge Mode**
   - Run HefaOS alongside ROS2
   - Use message bridge for interop
   - Migrate one node at a time

2. **Phase 2: Replace DDS**
   - Replace ROS2 DDS with iceoryx (rmw_iceoryx)
   - HefaOS and ROS2 share iceoryx transport
   - Gradual migration of nodes

3. **Phase 3: Native HefaOS**
   - All nodes migrated to native HefaOS
   - Remove ROS2 dependencies
   - Full performance benefits

---

## 15. Appendices

### 15.1 Glossary

| Term | Definition |
|------|------------|
| ECS | Entity-Component-System, a data-oriented design pattern |
| DAG | Directed Acyclic Graph |
| RT | Real-Time |
| IPC | Inter-Process Communication |
| WCET | Worst-Case Execution Time |
| HAL | Hardware Abstraction Layer |
| DDS | Data Distribution Service |
| FlatBuffers | Efficient serialization library by Google |
| Arrow | Apache Arrow, columnar memory format |
| iceoryx | Zero-copy IPC middleware |

### 15.2 References

1. Entity-Component-System Pattern
   - https://github.com/SanderMertens/flecs
   
2. Apache Arrow
   - https://arrow.apache.org/docs/
   
3. iceoryx
   - https://iceoryx.io/latest/
   
4. PREEMPT_RT
   - https://wiki.linuxfoundation.org/realtime/
   
5. Taskflow
   - https://taskflow.github.io/

### 15.3 Version History

| Version | Date | Changes |
|---------|------|---------|
| 1.0.0 | Jan 2026 | Initial specification |

---

## Document Control

**Classification:** Internal  
**Distribution:** Engineering Team  
**Review Cycle:** Quarterly  

**Approval:**

| Role | Name | Date |
|------|------|------|
| Author | - | - |
| Technical Lead | - | - |
| Architecture Review | - | - |

---

*End of Specification*
