#!/bin/bash

CONTAINER_NAME="bifrost"
DOCKER_IMAGE="bifrost:latest"
PORT=8000

echo "========================================="
echo "Deploying: $CONTAINER_NAME"
echo "Port: $PORT"
echo "========================================="

if ! docker build -t $DOCKER_IMAGE .; then
    echo "❌ Docker build failed! Exiting..."
    exit 1
fi

echo ">>> Stop the docker container: $CONTAINER_NAME"
docker stop $CONTAINER_NAME 2>/dev/null && echo "   Container stopped." || echo "   Container is not running."
docker rm $CONTAINER_NAME 2>/dev/null && echo "   Container removed" || echo "   Container not exists"

echo ">>> Prune the container"
docker container prune -f

docker run -d \
    --name $CONTAINER_NAME \
    --restart always \
    -p $PORT:$PORT \
    -v /etc/localtime:/etc/localtime:ro \
    $DOCKER_IMAGE

echo ">>> Cleaning up..."
docker image prune -f
echo "🎉 Deployment Successful! 🎊✨"
