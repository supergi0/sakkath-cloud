#!/bin/bash

# Build script for sakkath-cloud project
# Usage: ./build.sh <version>

if [ "$#" -ne 1 ]; then
    echo "Usage: ./build.sh <version>"
    echo "Example: ./build.sh 1.0.0"
    exit 1
fi

VERSION=$1
RELEASE_DIR="release_v${VERSION}"

echo "Building release version ${VERSION}..."

# Clean up existing release directory if it exists
if [ -d "$RELEASE_DIR" ]; then
    echo "Removing existing release directory..."
    rm -rf "$RELEASE_DIR"
fi

# Create release directory structure
mkdir -p "$RELEASE_DIR/api"
mkdir -p "$RELEASE_DIR/ui"

echo "Building Rust API..."
cd api
cargo build --release

if [ $? -ne 0 ]; then
    echo "Error: API build failed"
    exit 1
fi

# Copy API binary
echo "Copying API binary..."
cp target/release/sakkath-api "../$RELEASE_DIR/api/"
chmod +x "../$RELEASE_DIR/api/sakkath-api"

# Copy and modify .env file
if [ -f ".env" ]; then
    echo "Copying and modifying .env file..."
    sed 's/release=false/release=true/g' .env > "../$RELEASE_DIR/api/.env"
else
    echo "Creating .env file with release=true..."
    echo "release=true" > "../$RELEASE_DIR/api/.env"
fi

cd ..

echo "Building Next.js UI..."
cd ui
npm run build

if [ $? -ne 0 ]; then
    echo "Error: UI build failed"
    exit 1
fi

# Copy UI build output
echo "Copying UI build files..."
cp -r out/* "../$RELEASE_DIR/ui/"

cd ..

# Copy database file if it exists
if [ -f "sakkath.db" ]; then
    echo "Copying database file..."
    cp sakkath.db "$RELEASE_DIR/"
else
    echo "Warning: sakkath.db not found, will be created on first run"
fi

echo "Build complete! Release files are in: $RELEASE_DIR"
echo "Structure:"
tree -L 2 "$RELEASE_DIR" 2>/dev/null || ls -R "$RELEASE_DIR"
