#!/bin/bash

REPO_URL="git@github.com:hieulc0/healthy_bot.git"
TARGET_DIR="/home/hieulc/projects/healthy_bot"
SSH_SERVER="hieulc@192.168.2.156"
IMAGE="healthy_bot:arm64"

# Ensure target directory exists
ssh "$SSH_SERVER" "mkdir -p $TARGET_DIR"

# Send .env file
scp .env "$SSH_SERVER:$TARGET_DIR/.env"

# Run remote deployment
ssh "$SSH_SERVER" ash <<EOF
set -e  # Exit on any error

if [ -d "$TARGET_DIR/.git" ]; then
    echo "✅ Repo already exists at $TARGET_DIR"
else
    echo "📥 Cloning repo..."
    rm -rf "$TARGET_DIR"  # ensure clean if non-git folder exists
    git clone "$REPO_URL" "$TARGET_DIR"
fi

cd "$TARGET_DIR"

# Only run git commands if we're inside a valid repo
if [ -d ".git" ]; then
    echo "🔄 Updating repo..."
    git checkout develop
    git pull origin develop
else
    echo "❌ Not a git repository: $TARGET_DIR"
    exit 1
fi

# Check Dockerfile existence
if [ ! -f Dockerfile ] && [ ! -f Containerfile ]; then
    echo "❌ No Dockerfile or Containerfile in $TARGET_DIR"
    exit 1
fi

# Build the image
echo "🐳 Building image..."
podman build -t $IMAGE .

# Stop and remove running container from same image
CONTAINER_ID=\$(podman ps -q --filter ancestor=$IMAGE)
if [ -n "\$CONTAINER_ID" ]; then
    echo "⛔ Stopping existing container..."
    podman stop \$CONTAINER_ID
    podman rm \$CONTAINER_ID
fi

# Run new container
echo "🚀 Starting new container..."
podman run -d --env-file .env $IMAGE
EOF
echo "Deployment completed successfully!"