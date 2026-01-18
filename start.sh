#!/bin/bash

# Start script for sakkath-cloud project
# Usage: ./start.sh <ui|api>

if [ "$#" -ne 1 ]; then
    echo "Usage: ./start.sh <ui|api>"
    exit 1
fi

case "$1" in
    ui)
        echo "Starting UI on port 3000..."
        cd ui
        npm run dev -- -p 3000
        ;;
    api)
        echo "Starting API on port 9000..."
        cd api
        cargo watch --clear -x run -w src/
        ;;
    *)
        echo "Invalid argument. Use 'ui' or 'api'"
        exit 1
        ;;
esac
