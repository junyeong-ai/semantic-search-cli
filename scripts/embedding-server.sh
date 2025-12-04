#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
SERVER_DIR="$PROJECT_ROOT/embedding-server"
LOG_FILE="$PROJECT_ROOT/embedding-server.log"
PID_FILE="$PROJECT_ROOT/embedding-server.pid"

usage() {
    echo "Usage: $0 [start|stop|status|logs]"
    echo ""
    echo "Commands:"
    echo "  start   Start embedding server in background"
    echo "  stop    Stop embedding server"
    echo "  status  Check server status"
    echo "  logs    Tail server logs"
    echo ""
    echo "Options (for start):"
    echo "  --port PORT    Server port (default: 11411)"
    echo "  --model MODEL  Model ID (default: BAAI/bge-small-en-v1.5)"
    exit 1
}

start_server() {
    if [ -f "$PID_FILE" ]; then
        pid=$(cat "$PID_FILE")
        if kill -0 "$pid" 2>/dev/null; then
            echo "Server already running (PID: $pid)"
            exit 1
        fi
        rm -f "$PID_FILE"
    fi

    cd "$SERVER_DIR"

    # Check Python version
    if ! python --version 2>&1 | grep -qE "3\.(10|11|12|13|14)"; then
        echo "Warning: Python 3.10+ recommended. Current: $(python --version)"
    fi

    # Install dependencies if needed
    if ! python -c "import sentence_transformers" 2>/dev/null; then
        echo "Installing dependencies..."
        pip install -e .
    fi

    echo "Starting embedding server..."
    nohup python server.py "$@" > "$LOG_FILE" 2>&1 &
    echo $! > "$PID_FILE"

    sleep 2
    if kill -0 "$(cat "$PID_FILE")" 2>/dev/null; then
        echo "Server started (PID: $(cat "$PID_FILE"))"
        echo "Logs: $LOG_FILE"
    else
        echo "Failed to start server. Check logs: $LOG_FILE"
        rm -f "$PID_FILE"
        exit 1
    fi
}

stop_server() {
    if [ ! -f "$PID_FILE" ]; then
        echo "Server not running (no PID file)"
        exit 0
    fi

    pid=$(cat "$PID_FILE")
    if kill -0 "$pid" 2>/dev/null; then
        echo "Stopping server (PID: $pid)..."
        kill "$pid"
        rm -f "$PID_FILE"
        echo "Server stopped"
    else
        echo "Server not running (stale PID file)"
        rm -f "$PID_FILE"
    fi
}

check_status() {
    if [ -f "$PID_FILE" ]; then
        pid=$(cat "$PID_FILE")
        if kill -0 "$pid" 2>/dev/null; then
            echo "Server running (PID: $pid)"
            health=$(curl -s http://localhost:11411/health 2>/dev/null)
            if [ -n "$health" ]; then
                echo "Health: $health"
            else
                echo "Health check failed (server may still be starting)"
            fi
            exit 0
        fi
    fi
    echo "Server not running"
    exit 1
}

show_logs() {
    if [ -f "$LOG_FILE" ]; then
        tail -f "$LOG_FILE"
    else
        echo "No log file found: $LOG_FILE"
        exit 1
    fi
}

# Parse command
case "${1:-start}" in
    start)
        shift || true
        start_server "$@"
        ;;
    stop)
        stop_server
        ;;
    status)
        check_status
        ;;
    logs)
        show_logs
        ;;
    -h|--help)
        usage
        ;;
    *)
        usage
        ;;
esac
