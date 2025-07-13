#!/bin/bash

set -euo pipefail

# === Config ===
REPO_URL="git@github.com:hieulc0/healthy_bot.git"
PROJECT_NAME="healthy_bot"
TARGET_DIR="/home/hieulc/projects/$PROJECT_NAME"
SSH_SERVER="hieulc@192.168.2.156"
IMAGE="$PROJECT_NAME:arm64"
ENV_FILE=".env"

echo "🚀 Starting deployment to $SSH_SERVER"

# === Step 1: Ensure target directory exists
ssh "$SSH_SERVER" "mkdir -p '$TARGET_DIR'"

# === Step 2: Copy .env
echo "📤 Uploading .env..."
scp "$ENV_FILE" "$SSH_SERVER:$TARGET_DIR/.env"

# === Step 3: Remote logic
ssh "$SSH_SERVER" ash <<EOF
    set -euo pipefail

    # Clone repo if not present
    if [ ! -d "$TARGET_DIR/.git" ]; then
        echo "📥 Cloning repository..."
        rm -rf "$TARGET_DIR"  # cleanup any conflicting folder
        git clone "$REPO_URL" "$TARGET_DIR"
    else
        echo "✅ Repository already exists."
    fi

    cd "$TARGET_DIR"

    # Git checkout & pull
    echo "🔄 Updating repository..."
    git checkout develop
    git pull origin develop

    # Check Dockerfile/Containerfile exists
    if [ ! -f Dockerfile ] && [ ! -f Containerfile ]; then
        echo "❌ No Dockerfile or Containerfile found in $TARGET_DIR"
        exit 1
    fi

    # Build image
    echo "🐳 Building image..."
    podman build -t $IMAGE .

    # Stop and remove any running containers from this image
    echo "🛑 Stopping existing containers (if any)..."
    existing=\$(podman ps -q --filter ancestor=$IMAGE)
    if [ -n "\$existing" ]; then
        podman stop \$existing
        podman rm \$existing
    fi

    # Run new container
    echo "🚀 Starting new container..."
    podman run -d --env-file .env \
      --network=host \
      -v /sys/class/power_supply:/host/sys/class/power_supply:ro \
      -v /sys/class/net:/host/sys/class/net:ro \
      -v /proc/net/fib_trie:/host/proc/net/fib_trie:ro \
      -v /proc/net/if_inet6:/host/proc/net/if_inet6:ro \
      --privileged \
      $IMAGE

    # Optional cleanup
    echo "🧼 Cleaning up unused images..."
    podman image prune -f

    echo "✅ Remote deployment finished."
EOF

echo "✅ Deployment completed successfully!"
