#!/usr/bin/env bash
#
# Embedding Server startup script
# Usage: ./scripts/embedding-server.sh [start|stop|restart|rebuild|status|logs]
#
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
SERVER_DIR="$PROJECT_ROOT/embedding-server"
LOG_FILE="$PROJECT_ROOT/embedding-server.log"
PID_FILE="$PROJECT_ROOT/embedding-server.pid"
PORT_FILE="$PROJECT_ROOT/embedding-server.port"
PROCESS_NAME="server.py"
DEFAULT_PORT=11411

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

usage() {
    echo "Usage: $0 [start|stop|restart|rebuild|status|logs]"
    echo ""
    echo "Commands:"
    echo "  start     Start embedding server in background"
    echo "  stop      Stop embedding server"
    echo "  restart   Restart embedding server"
    echo "  rebuild   Reinstall dependencies and restart"
    echo "  status    Check server status"
    echo "  logs      Show server logs (default: last 50 lines)"
    echo ""
    echo "Options (for start):"
    echo "  --port PORT    Server port (default: $DEFAULT_PORT)"
    echo "  --model MODEL  Model ID (default: BAAI/bge-small-en-v1.5)"
    exit 1
}

get_port() {
    [ -f "$PORT_FILE" ] && cat "$PORT_FILE" || echo "$DEFAULT_PORT"
}

get_pid() {
    [ -f "$PID_FILE" ] && cat "$PID_FILE"
}

is_running() {
    local pid=$(get_pid)
    [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null && ps -p "$pid" -o args= 2>/dev/null | grep -q "$PROCESS_NAME"
}

health_check() {
    local port=$(get_port)
    curl -sf "http://localhost:$port/health" >/dev/null 2>&1
}

check_python() {
    if ! python --version 2>&1 | grep -qE "3\.(10|11|12|13|14)"; then
        echo -e "${YELLOW}Warning: Python 3.10+ recommended. Current: $(python --version 2>&1)${NC}"
    fi
}

install_deps() {
    echo -e "${GREEN}Installing dependencies...${NC}"
    cd "$SERVER_DIR"
    pip install -e . -q
    echo -e "${GREEN}Dependencies installed${NC}"
}

start_server() {
    if is_running; then
        echo -e "${YELLOW}Embedding server is already running (PID: $(get_pid))${NC}"
        return 0
    fi

    cd "$SERVER_DIR"
    check_python

    # Install dependencies if needed
    if ! python -c "import sentence_transformers" 2>/dev/null; then
        install_deps
    fi

    # Parse port from arguments
    local port=$DEFAULT_PORT
    local args=()
    while [[ $# -gt 0 ]]; do
        case $1 in
            --port)
                port="$2"
                args+=("$1" "$2")
                shift 2
                ;;
            *)
                args+=("$1")
                shift
                ;;
        esac
    done

    echo -e "${GREEN}Starting embedding server...${NC}"
    nohup python server.py "${args[@]}" > "$LOG_FILE" 2>&1 &
    local pid=$!
    echo $pid > "$PID_FILE"
    echo $port > "$PORT_FILE"

    sleep 3
    if is_running && health_check; then
        echo -e "${GREEN}Embedding server started (PID: $pid)${NC}"
        echo -e "Listening on: ${GREEN}http://localhost:$port${NC}"
        echo -e "Logs: $LOG_FILE"
    elif is_running; then
        echo -e "${YELLOW}Embedding server started but health check failed${NC}"
        echo -e "Server may still be loading model. Check logs: $LOG_FILE"
    else
        echo -e "${RED}Failed to start embedding server. Check logs: $LOG_FILE${NC}"
        rm -f "$PID_FILE" "$PORT_FILE"
        return 1
    fi
}

stop_server() {
    if ! is_running; then
        echo -e "${YELLOW}Embedding server is not running${NC}"
        rm -f "$PID_FILE"
        return 0
    fi

    local pid=$(get_pid)
    echo -e "${YELLOW}Stopping embedding server (PID: $pid)...${NC}"

    kill "$pid" 2>/dev/null

    # Wait for graceful shutdown
    local count=0
    while is_running && [ $count -lt 10 ]; do
        sleep 1
        count=$((count + 1))
    done

    # Force kill if still running
    if is_running; then
        echo -e "${RED}Force killing embedding server...${NC}"
        kill -9 "$pid" 2>/dev/null
    fi

    rm -f "$PID_FILE"
    echo -e "${GREEN}Embedding server stopped${NC}"
}

check_status() {
    local port=$(get_port)
    if is_running; then
        echo -e "${GREEN}Embedding server is running${NC}"
        echo "  PID: $(get_pid)"
        echo "  URL: http://localhost:$port"
        echo "  Log: $LOG_FILE"
        if health_check; then
            echo -e "  Health: ${GREEN}OK${NC}"
            # Try to get model info
            local model=$(curl -s "http://localhost:$port/health" 2>/dev/null | grep -o '"model":"[^"]*"' | cut -d'"' -f4)
            [ -n "$model" ] && echo "  Model: $model"
        else
            echo -e "  Health: ${YELLOW}Loading...${NC} (model may still be initializing)"
        fi
    else
        echo -e "${RED}Embedding server is not running${NC}"
        return 1
    fi
}

show_logs() {
    if [ ! -f "$LOG_FILE" ]; then
        echo -e "${RED}No log file found: $LOG_FILE${NC}"
        return 1
    fi

    local lines=${2:-50}
    if [ "$lines" = "-f" ]; then
        tail -f "$LOG_FILE"
    else
        tail -"$lines" "$LOG_FILE"
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
    restart)
        stop_server
        sleep 1
        shift || true
        start_server "$@"
        ;;
    rebuild)
        stop_server
        install_deps
        sleep 1
        shift || true
        start_server "$@"
        ;;
    status)
        check_status
        ;;
    logs)
        show_logs "$@"
        ;;
    -h|--help)
        usage
        ;;
    *)
        usage
        ;;
esac
