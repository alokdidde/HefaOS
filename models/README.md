# Hefaos AI Models

This directory contains pre-trained AI models for Hefaos robotics applications.

## Important

Model files are **not committed to git** due to their large size. Use the download script to fetch them.

## Downloading Models

```bash
# Download base perception and control models (~500MB)
./tools/scripts/download-models.sh

# Download all models including LLM (~5GB)
./tools/scripts/download-models.sh --all

# Download LLM only
./tools/scripts/download-models.sh --llm
```

## Available Models

### Perception Models

| Model | Size | Purpose |
|-------|------|---------|
| `yolov8s.onnx` | 22MB | Object detection |
| `depth-anything-small.onnx` | 50MB | Monocular depth estimation |
| `sam-vit-b.onnx` | 375MB | Segment Anything Model |

### Control Models

| Model | Size | Purpose |
|-------|------|---------|
| `grasp-policy.tflite` | 5MB | Grasp pose prediction |
| `motion-planner.onnx` | 15MB | Neural motion planning |

### Language Models (Optional)

| Model | Size | Purpose |
|-------|------|---------|
| `llama3-8b-q4.gguf` | 4.7GB | Natural language understanding |

## Using Models

```cpp
#include <hefaos/ai/runtime.hpp>

// Load ONNX model
auto model = hefaos::ai::load_model("models/yolov8s.onnx");

// Run inference
auto results = model->infer(input_tensor);
```

```typescript
import { loadModel } from '@hefaos/sdk';

// Load model in simulation
const detector = await loadModel('models/yolov8s.onnx');
const detections = await detector.predict(cameraFrame);
```

## Custom Models

To add your own models:

1. Place the model file in this directory
2. Register it in your robot definition
3. The model will be deployed with your robot package

## Model Formats

Hefaos supports:

- **ONNX** (`.onnx`) - Universal format via ONNX Runtime
- **TensorFlow Lite** (`.tflite`) - Optimized for edge devices
- **GGUF** (`.gguf`) - LLM format via llama.cpp
