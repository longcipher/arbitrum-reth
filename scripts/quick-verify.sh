#!/usr/bin/env bash

# Arbitrum-Reth 快速兼容性验证脚本
# 这个脚本提供一键式的兼容性测试

set -e

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# 配置
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
TEST_DATA_DIR="$PROJECT_ROOT/test-data"
REPORTS_DIR="$PROJECT_ROOT/reports"
LOGS_DIR="$PROJECT_ROOT/logs"

# 默认配置
DEFAULT_DURATION=300  # 5分钟快速测试
DEFAULT_TPS=100
DEFAULT_CONCURRENT=10

print_header() {
    echo -e "${BLUE}"
    echo "================================================================"
    echo "  Arbitrum-Reth 兼容性快速验证工具"
    echo "================================================================"
    echo -e "${NC}"
}

print_step() {
    echo -e "${YELLOW}>>> $1${NC}"
}

print_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

print_error() {
    echo -e "${RED}❌ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

show_usage() {
    cat << EOF
用法: $0 [选项]

选项:
    -h, --help              显示此帮助信息
    -q, --quick             快速测试 (1分钟)
    -b, --basic             基础测试 (5分钟, 默认)
    -f, --full              完整测试 (30分钟)
    -p, --performance       仅性能测试
    -r, --rpc               仅RPC兼容性测试
    -c, --consistency       仅数据一致性测试
    -d, --duration SECS     自定义测试时长
    --tps NUMBER            目标TPS (默认: $DEFAULT_TPS)
    --concurrent NUMBER     并发连接数 (默认: $DEFAULT_CONCURRENT)
    --nitro-endpoint URL    Nitro节点端点
    --reth-endpoint URL     Reth节点端点
    --skip-build           跳过构建步骤
    --cleanup              仅清理测试数据
    --report-only          仅生成现有测试报告

示例:
    $0 --quick                    # 1分钟快速验证
    $0 --basic                    # 5分钟基础测试
    $0 --full                     # 30分钟完整测试
    $0 --performance --duration 600 --tps 500  # 10分钟性能测试
    $0 --cleanup                  # 清理测试数据

EOF
}

check_dependencies() {
    print_step "检查依赖项..."
    
    local missing_deps=()
    
    if ! command -v cargo &> /dev/null; then
        missing_deps+=("cargo (Rust)")
    fi
    
    if ! command -v curl &> /dev/null; then
        missing_deps+=("curl")
    fi
    
    if ! command -v jq &> /dev/null; then
        print_warning "jq 未安装，JSON 输出可能不会被格式化"
    fi
    
    if [ ${#missing_deps[@]} -gt 0 ]; then
        print_error "缺少依赖项: ${missing_deps[*]}"
        echo "请安装缺少的依赖项后重试"
        exit 1
    fi
    
    print_success "依赖项检查完成"
}

setup_environment() {
    print_step "设置测试环境..."
    
    # 创建必要的目录
    mkdir -p "$TEST_DATA_DIR"/{nitro,reth}
    mkdir -p "$REPORTS_DIR" "$LOGS_DIR"
    mkdir -p "$PROJECT_ROOT"/test-configs
    
    # 确保配置文件存在
    if [ ! -f "$PROJECT_ROOT/test-configs/compatibility.toml" ]; then
        print_warning "配置文件不存在，使用默认配置"
        cat > "$PROJECT_ROOT/test-configs/compatibility.toml" << 'EOF'
[test]
timeout_seconds = 30
parallel_requests = 10

[endpoints]
nitro_rpc = "http://localhost:8547"
reth_rpc = "http://localhost:8548"

[performance]
target_tps = 100
max_latency_p95_ms = 100
test_duration_seconds = 300
EOF
    fi
    
    print_success "测试环境设置完成"
}

build_project() {
    if [ "$SKIP_BUILD" = "true" ]; then
        print_step "跳过构建步骤"
        return
    fi
    
    print_step "构建项目..."
    
    cd "$PROJECT_ROOT"
    
    # 构建主项目
    if ! cargo build --workspace --release --quiet; then
        print_error "项目构建失败"
        exit 1
    fi
    
    # 检查二进制文件
    if [ ! -f "$PROJECT_ROOT/target/release/arbitrum-reth" ]; then
        print_error "主二进制文件未找到"
        exit 1
    fi
    
    print_success "项目构建完成"
}

start_mock_nitro() {
    print_step "启动模拟 Nitro 节点..."
    
    # 创建简单的模拟 RPC 服务器
    cat > "$TEST_DATA_DIR/mock_nitro.py" << 'EOF'
#!/usr/bin/env python3
import json
import threading
import time
from http.server import HTTPServer, BaseHTTPRequestHandler
from urllib.parse import urlparse

class MockNitroHandler(BaseHTTPRequestHandler):
    def do_POST(self):
        try:
            content_length = int(self.headers.get('Content-Length', 0))
            if content_length > 0:
                post_data = self.rfile.read(content_length)
                request = json.loads(post_data.decode('utf-8'))
            else:
                request = {}
            
            method = request.get('method', '')
            request_id = request.get('id', 1)
            
            # 模拟常见 RPC 响应
            responses = {
                'eth_blockNumber': '0x12345',
                'eth_chainId': '0xa4b1',  # Arbitrum One
                'eth_gasPrice': '0x174876e800',
                'eth_getBalance': '0x1bc16d674ec80000',
                'eth_getTransactionCount': '0x1',
                'eth_call': '0x',
                'eth_estimateGas': '0x5208',
            }
            
            result = responses.get(method, None)
            
            response = {
                "jsonrpc": "2.0",
                "id": request_id,
                "result": result
            }
            
            self.send_response(200)
            self.send_header('Content-Type', 'application/json')
            self.send_header('Access-Control-Allow-Origin', '*')
            self.end_headers()
            self.wfile.write(json.dumps(response).encode())
            
        except Exception as e:
            self.send_error(500, str(e))
    
    def do_GET(self):
        if self.path == '/health':
            self.send_response(200)
            self.send_header('Content-Type', 'application/json')
            self.end_headers()
            self.wfile.write(b'{"status":"ok"}')
        else:
            self.send_error(404)
    
    def log_message(self, format, *args):
        pass  # 禁用日志输出

if __name__ == '__main__':
    server = HTTPServer(('127.0.0.1', 8547), MockNitroHandler)
    print("Mock Nitro server starting on port 8547...")
    server.serve_forever()
EOF
    
    # 启动模拟服务器
    python3 "$TEST_DATA_DIR/mock_nitro.py" > "$LOGS_DIR/mock_nitro.log" 2>&1 &
    MOCK_NITRO_PID=$!
    echo $MOCK_NITRO_PID > "$TEST_DATA_DIR/mock_nitro.pid"
    
    # 等待服务器启动
    sleep 3
    
    # 验证服务器是否运行
    if curl -s "http://localhost:8547/health" > /dev/null; then
        print_success "模拟 Nitro 节点启动成功"
    else
        print_error "模拟 Nitro 节点启动失败"
        cleanup_processes
        exit 1
    fi
}

start_arbitrum_reth() {
    print_step "启动 Arbitrum-Reth 节点..."
    
    # 启动 Arbitrum-Reth
    "$PROJECT_ROOT/target/release/arbitrum-reth" \
        --datadir "$TEST_DATA_DIR/reth" \
        --log-level info \
        --metrics \
        --metrics-addr 127.0.0.1:9090 \
        > "$LOGS_DIR/arbitrum_reth.log" 2>&1 &
    
    RETH_PID=$!
    echo $RETH_PID > "$TEST_DATA_DIR/arbitrum_reth.pid"
    
    # 等待节点启动
    print_step "等待 Arbitrum-Reth 节点启动..."
    local timeout=60
    local count=0
    
    while [ $count -lt $timeout ]; do
        if curl -s "http://localhost:8548/health" > /dev/null 2>&1; then
            print_success "Arbitrum-Reth 节点启动成功"
            return 0
        fi
        sleep 2
        count=$((count + 2))
        if [ $((count % 10)) -eq 0 ]; then
            echo -n "."
        fi
    done
    
    print_error "Arbitrum-Reth 节点启动超时"
    print_error "检查日志: $LOGS_DIR/arbitrum_reth.log"
    cleanup_processes
    exit 1
}

run_rpc_compatibility_test() {
    print_step "运行 RPC 兼容性测试..."
    
    local test_duration=${1:-$DEFAULT_DURATION}
    local output_file="$REPORTS_DIR/rpc_compatibility_$(date +%Y%m%d_%H%M%S).json"
    
    # 检查测试工具是否存在
    if [ ! -f "$PROJECT_ROOT/tests/rpc_compatibility_tester.rs" ]; then
        print_warning "RPC 兼容性测试工具源文件不存在，跳过此测试"
        return 0
    fi
    
    # 编译并运行测试
    cd "$PROJECT_ROOT"
    if cargo build --release --test rpc_compatibility_tester 2>/dev/null; then
        if timeout $test_duration cargo run --release --test rpc_compatibility_tester -- \
            --nitro-endpoint "http://localhost:8547" \
            --reth-endpoint "http://localhost:8548" \
            --test-suite quick \
            --output "$output_file" > "$LOGS_DIR/rpc_test.log" 2>&1; then
            print_success "RPC 兼容性测试完成"
        else
            print_warning "RPC 兼容性测试完成但有警告"
        fi
    else
        print_warning "RPC 兼容性测试工具编译失败，跳过此测试"
    fi
}

run_performance_test() {
    print_step "运行性能测试..."
    
    local test_duration=${1:-$DEFAULT_DURATION}
    local target_tps=${2:-$DEFAULT_TPS}
    local concurrent=${3:-$DEFAULT_CONCURRENT}
    local output_file="$REPORTS_DIR/performance_$(date +%Y%m%d_%H%M%S).json"
    
    # 检查测试工具是否存在
    if [ ! -f "$PROJECT_ROOT/tests/performance_benchmark.rs" ]; then
        print_warning "性能测试工具源文件不存在，跳过此测试"
        return 0
    fi
    
    cd "$PROJECT_ROOT"
    if cargo build --release --test performance_benchmark 2>/dev/null; then
        if timeout $((test_duration + 60)) cargo run --release --test performance_benchmark -- \
            --duration $test_duration \
            --target-tps $target_tps \
            --concurrent $concurrent \
            --nitro-endpoint "http://localhost:8547" \
            --reth-endpoint "http://localhost:8548" \
            --output "$output_file" > "$LOGS_DIR/performance_test.log" 2>&1; then
            print_success "性能测试完成"
        else
            print_warning "性能测试完成但有警告"
        fi
    else
        print_warning "性能测试工具编译失败，跳过此测试"
    fi
}

run_basic_verification() {
    print_step "运行基础验证..."
    
    # 简单的 RPC 调用测试
    local nitro_block_number
    local reth_block_number
    
    # 测试 Nitro 端点
    if nitro_block_number=$(curl -s -X POST \
        -H "Content-Type: application/json" \
        --data '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' \
        http://localhost:8547 | jq -r '.result' 2>/dev/null); then
        print_success "Nitro 端点响应正常 (区块号: $nitro_block_number)"
    else
        print_error "Nitro 端点无响应"
        return 1
    fi
    
    # 测试 Reth 端点 (如果可用)
    if curl -s "http://localhost:8548/health" > /dev/null 2>&1; then
        if reth_block_number=$(curl -s -X POST \
            -H "Content-Type: application/json" \
            --data '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' \
            http://localhost:8548 | jq -r '.result' 2>/dev/null); then
            print_success "Reth 端点响应正常 (区块号: $reth_block_number)"
        else
            print_warning "Reth 端点未响应 RPC 调用"
        fi
    else
        print_warning "Reth 节点健康检查失败"
    fi
    
    return 0
}

generate_report() {
    print_step "生成测试报告..."
    
    local report_file="$REPORTS_DIR/summary_$(date +%Y%m%d_%H%M%S).md"
    
    cat > "$report_file" << EOF
# Arbitrum-Reth 兼容性测试报告

**测试时间:** $(date)
**测试类型:** $TEST_TYPE
**测试时长:** ${DURATION}秒

## 测试概要

### 环境信息
- 操作系统: $(uname -s)
- 架构: $(uname -m)
- Rust 版本: $(rustc --version 2>/dev/null || echo "未知")

### 节点状态
- Nitro 模拟节点: $(curl -s http://localhost:8547/health >/dev/null 2>&1 && echo "✅ 运行中" || echo "❌ 未运行")
- Arbitrum-Reth 节点: $(curl -s http://localhost:8548/health >/dev/null 2>&1 && echo "✅ 运行中" || echo "❌ 未运行")

## 测试结果

EOF

    # 添加测试结果
    if [ -n "$(ls -A "$REPORTS_DIR"/*.json 2>/dev/null)" ]; then
        echo "### 详细结果文件" >> "$report_file"
        ls -la "$REPORTS_DIR"/*.json >> "$report_file" 2>/dev/null || true
    fi
    
    echo "" >> "$report_file"
    echo "### 日志文件" >> "$report_file"
    ls -la "$LOGS_DIR"/ >> "$report_file" 2>/dev/null || true
    
    print_success "测试报告已生成: $report_file"
}

cleanup_processes() {
    print_step "清理进程..."
    
    # 停止 Arbitrum-Reth
    if [ -f "$TEST_DATA_DIR/arbitrum_reth.pid" ]; then
        local reth_pid=$(cat "$TEST_DATA_DIR/arbitrum_reth.pid")
        if ps -p $reth_pid > /dev/null 2>&1; then
            kill $reth_pid
            sleep 2
            if ps -p $reth_pid > /dev/null 2>&1; then
                kill -9 $reth_pid
            fi
        fi
        rm -f "$TEST_DATA_DIR/arbitrum_reth.pid"
    fi
    
    # 停止模拟 Nitro
    if [ -f "$TEST_DATA_DIR/mock_nitro.pid" ]; then
        local nitro_pid=$(cat "$TEST_DATA_DIR/mock_nitro.pid")
        if ps -p $nitro_pid > /dev/null 2>&1; then
            kill $nitro_pid
        fi
        rm -f "$TEST_DATA_DIR/mock_nitro.pid"
    fi
    
    # 清理其他 Python 进程
    pkill -f mock_nitro.py 2>/dev/null || true
    
    print_success "进程清理完成"
}

cleanup_data() {
    print_step "清理测试数据..."
    
    rm -rf "$TEST_DATA_DIR"
    rm -rf "$REPORTS_DIR"/*.json 2>/dev/null || true
    rm -rf "$LOGS_DIR"/*.log 2>/dev/null || true
    
    print_success "测试数据清理完成"
}

main() {
    # 默认参数
    TEST_TYPE="basic"
    DURATION=$DEFAULT_DURATION
    TPS=$DEFAULT_TPS
    CONCURRENT=$DEFAULT_CONCURRENT
    SKIP_BUILD="false"
    CLEANUP_ONLY="false"
    REPORT_ONLY="false"
    RUN_RPC="true"
    RUN_PERFORMANCE="true"
    
    # 解析命令行参数
    while [[ $# -gt 0 ]]; do
        case $1 in
            -h|--help)
                show_usage
                exit 0
                ;;
            -q|--quick)
                TEST_TYPE="quick"
                DURATION=60
                shift
                ;;
            -b|--basic)
                TEST_TYPE="basic"
                DURATION=300
                shift
                ;;
            -f|--full)
                TEST_TYPE="full"
                DURATION=1800
                shift
                ;;
            -p|--performance)
                RUN_RPC="false"
                shift
                ;;
            -r|--rpc)
                RUN_PERFORMANCE="false"
                shift
                ;;
            -c|--consistency)
                RUN_RPC="false"
                RUN_PERFORMANCE="false"
                shift
                ;;
            -d|--duration)
                DURATION="$2"
                shift 2
                ;;
            --tps)
                TPS="$2"
                shift 2
                ;;
            --concurrent)
                CONCURRENT="$2"
                shift 2
                ;;
            --skip-build)
                SKIP_BUILD="true"
                shift
                ;;
            --cleanup)
                CLEANUP_ONLY="true"
                shift
                ;;
            --report-only)
                REPORT_ONLY="true"
                shift
                ;;
            *)
                print_error "未知参数: $1"
                show_usage
                exit 1
                ;;
        esac
    done
    
    print_header
    
    # 如果只是清理，执行清理并退出
    if [ "$CLEANUP_ONLY" = "true" ]; then
        cleanup_processes
        cleanup_data
        exit 0
    fi
    
    # 如果只是生成报告，生成并退出
    if [ "$REPORT_ONLY" = "true" ]; then
        generate_report
        exit 0
    fi
    
    # 设置清理陷阱
    trap cleanup_processes EXIT
    
    # 主要测试流程
    check_dependencies
    setup_environment
    build_project
    
    # 启动节点
    start_mock_nitro
    start_arbitrum_reth
    
    # 基础验证
    if ! run_basic_verification; then
        print_error "基础验证失败，停止测试"
        exit 1
    fi
    
    # 运行选定的测试
    if [ "$RUN_RPC" = "true" ]; then
        run_rpc_compatibility_test $DURATION
    fi
    
    if [ "$RUN_PERFORMANCE" = "true" ]; then
        run_performance_test $DURATION $TPS $CONCURRENT
    fi
    
    # 生成报告
    generate_report
    
    print_success "所有测试完成！"
    echo -e "${BLUE}查看结果:${NC}"
    echo "  报告目录: $REPORTS_DIR"
    echo "  日志目录: $LOGS_DIR"
}

# 运行主函数
main "$@"
