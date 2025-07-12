#!/bin/bash

REPO_URL="git@github.com:hieulc0/healthy_bot.git"
TARGET_DIR="/home/hieulc/projects/healthy_bot"
SSH_SERVER="hieulc@192.168.2.156"
IMAGE="healthy_bot:arm64"

# 📦 Send .env file first (ensure target dir exists)
ssh "$SSH_SERVER" "mkdir -p $TARGET_DIR"
scp .env "$SSH_SERVER:$TARGET_DIR/.env"

# 🚀 SSH into the server and deploy
ssh "$SSH_SERVER" ash <<EOF
if [ -d "$TARGET_DIR/.git" ]; then
    echo "✅ Repo already exists at $TARGET_DIR"
else
    echo "📥 Cloning repo..."
    git clone "$REPO_URL" "$TARGET_DIR"
fi

cd "$TARGET_DIR"
git checkout develop
git pull origin develop

# 🐳 Rebuild image
podman build -t $IMAGE .

# 🔁 Restart container
CONTAINER_ID=\$(podman ps -q --filter ancestor=$IMAGE)
if [ -n "\$CONTAINER_ID" ]; then
    echo "⛔ Stopping existing container..."
    podman stop \$CONTAINER_ID
    podman rm \$CONTAINER_ID
fi

echo "🚀 Starting new container..."
podman run -d --env-file .env $IMAGE
EOF
