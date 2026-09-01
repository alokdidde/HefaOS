#!/bin/bash
# ─────────────────────────────────────────────────────────────────────────────
# Hefaos Model Download Script
# ─────────────────────────────────────────────────────────────────────────────
#
# Downloads pre-trained AI models for Hefaos robotics applications.
# Models are downloaded from Hugging Face.
#
# Usage:
#   ./tools/scripts/download-models.sh           # Download base models
#   ./tools/scripts/download-models.sh --all     # Download all models including LLM
#   ./tools/scripts/download-models.sh --llm     # Download LLM model only
#
# ─────────────────────────────────────────────────────────────────────────────

set -e

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MODELS_DIR="${SCRIPT_DIR}/../../models"
BASE_URL="https://huggingface.co/hefaos-robotics/models/resolve/main"

# Create models directory
mkdir -p "$MODELS_DIR"

# ─────────────────────────────────────────────────────────────────────────────
# Helper functions
# ─────────────────────────────────────────────────────────────────────────────

download_model() {
    local url="$1"
    local output="$2"
    local description="$3"

    if [ -f "$output" ]; then
        echo "  ⏭ $description already exists, skipping"
        return 0
    fi

    echo "  ⬇ Downloading $description..."
    if curl -L --progress-bar "$url" -o "$output.tmp"; then
        mv "$output.tmp" "$output"
        echo "  ✓ $description downloaded"
    else
        echo "  ✗ Failed to download $description"
        rm -f "$output.tmp"
        return 1
    fi
}

print_usage() {
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Options:"
    echo "  --all     Download all models including large LLM"
    echo "  --llm     Download LLM model only"
    echo "  --help    Show this help message"
    echo ""
    echo "Without options, downloads base perception and control models."
}

# ─────────────────────────────────────────────────────────────────────────────
# Parse arguments
# ─────────────────────────────────────────────────────────────────────────────

DOWNLOAD_BASE=true
DOWNLOAD_LLM=false

while [[ $# -gt 0 ]]; do
    case $1 in
        --all)
            DOWNLOAD_LLM=true
            shift
            ;;
        --llm)
            DOWNLOAD_BASE=false
            DOWNLOAD_LLM=true
            shift
            ;;
        --help)
            print_usage
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            print_usage
            exit 1
            ;;
    esac
done

# ─────────────────────────────────────────────────────────────────────────────
# Download models
# ─────────────────────────────────────────────────────────────────────────────

echo "╔═══════════════════════════════════════════════════════════╗"
echo "║           Hefaos Model Download                            ║"
echo "╚═══════════════════════════════════════════════════════════╝"
echo ""
echo "Models directory: $MODELS_DIR"
echo ""

if [ "$DOWNLOAD_BASE" = true ]; then
    echo "Downloading base models..."
    echo ""

    # Perception models
    download_model \
        "$BASE_URL/yolov8s.onnx" \
        "$MODELS_DIR/yolov8s.onnx" \
        "YOLOv8s object detection (22MB)"

    download_model \
        "$BASE_URL/depth-anything-small.onnx" \
        "$MODELS_DIR/depth-anything-small.onnx" \
        "Depth Anything small (50MB)"

    download_model \
        "$BASE_URL/sam-vit-b.onnx" \
        "$MODELS_DIR/sam-vit-b.onnx" \
        "SAM ViT-B segmentation (375MB)"

    # Control models
    download_model \
        "$BASE_URL/grasp-policy.tflite" \
        "$MODELS_DIR/grasp-policy.tflite" \
        "Grasp policy network (5MB)"

    download_model \
        "$BASE_URL/motion-planner.onnx" \
        "$MODELS_DIR/motion-planner.onnx" \
        "Motion planner (15MB)"

    echo ""
fi

if [ "$DOWNLOAD_LLM" = true ]; then
    echo "Downloading LLM model (this may take a while)..."
    echo ""

    download_model \
        "$BASE_URL/llama3-8b-q4.gguf" \
        "$MODELS_DIR/llama3-8b-q4.gguf" \
        "LLaMA 3 8B Q4 (4.7GB)"

    echo ""
fi

# ─────────────────────────────────────────────────────────────────────────────
# Summary
# ─────────────────────────────────────────────────────────────────────────────

echo "╔═══════════════════════════════════════════════════════════╗"
echo "║  ✓ Model download complete!                                ║"
echo "╠═══════════════════════════════════════════════════════════╣"
echo "║  Downloaded models:                                        ║"

for model in "$MODELS_DIR"/*.{onnx,tflite,gguf} 2>/dev/null; do
    if [ -f "$model" ]; then
        size=$(du -h "$model" | cut -f1)
        name=$(basename "$model")
        printf "║  • %-40s %8s    ║\n" "$name" "$size"
    fi
done

echo "╚═══════════════════════════════════════════════════════════╝"
