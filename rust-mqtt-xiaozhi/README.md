# XiaoZhi MQTT Gateway - Rust 版本

这是一个用 Rust 重写的 XiaoZhi MQTT Gateway，提供高性能、内存安全的 MQTT 网关服务。

## 项目结构

```
rust-mqtt-xiaozhi/
├── Cargo.toml              # 主项目配置
├── config.toml             # 配置文件
├── src/
│   ├── main.rs            # 主程序入口
│   └── connection.rs      # 连接管理模块
├── mqtt-protocol/          # MQTT 协议处理模块
├── websocket-bridge/      # WebSocket 桥接模块
├── udp-handler/           # UDP 音频处理模块
├── config-manager/       # 配置管理模块
└── auth-manager/         # 认证管理模块
```

## 功能特性

### 核心功能

- **MQTT 协议支持**：完整的 MQTT 3.1.1 协议实现
- **WebSocket 桥接**：支持与 WebSocket 服务器的双向通信
- **UDP 音频传输**：支持加密的 UDP 音频数据传输
- **多协议认证**：支持默认认证、OAuth2、JWT、Token 等多种认证方式
- **连接管理**：自动管理客户端连接，支持心跳检测
- **配置管理**：支持 TOML/JSON 格式的配置文件
- **高性能**：基于 Tokio 异步运行时，支持高并发

### 技术优势

- **内存安全**：Rust 的所有权系统确保内存安全
- **高性能**：零成本抽象和高效的异步 I/O
- **类型安全**：编译时类型检查，减少运行时错误
- **并发安全**：内置的并发原语和线程安全保证
- **跨平台**：支持 Windows、Linux、macOS 等平台

## 快速开始

### 环境要求

- Rust 1.70 或更高版本
- 现代操作系统（Windows、Linux、macOS）

### 安装

1. 克隆项目：
```bash
git clone <repository-url>
cd rust-mqtt-xiaozhi
```

2. 构建项目：
```bash
cargo build --release
```

3. 配置文件：
编辑 `config.toml` 文件，根据需要修改配置。

### 运行

```bash
cargo run --release
```

或者使用编译后的二进制文件：

```bash
./target/release/mqtt-gateway
```

## 配置说明

### 配置文件 (config.toml)

```toml
[mqtt]
port = 1883                    # MQTT 服务端口
max_connections = 1000        # 最大连接数
max_payload_size = 8192        # 最大消息大小
keep_alive_timeout = 60        # 心跳超时时间（秒）

[udp]
port = 1883                    # UDP 服务端口
max_packet_size = 8192         # 最大数据包大小
encryption = "aes-128-ctr"     # 加密算法

[websocket]
url = "ws://localhost:8080"    # WebSocket 服务器地址
reconnect_interval = 5000      # 重连间隔（毫秒）
max_reconnect_attempts = 10     # 最大重连次数

[auth]
enabled = true                 # 是否启用认证
protocols = ["default", "oauth2", "jwt", "token"]  # 支持的认证协议
default_username = "test_user" # 默认用户名
default_password = "test_password"  # 默认密码

[logging]
level = "info"                 # 日志级别
file = "mqtt-gateway.log"      # 日志文件路径
```

### 环境变量

也可以通过环境变量配置：

```bash
export MQTT_PORT=1883
export UDP_PORT=1883
export WS_URL="ws://localhost:8080"
export LOG_LEVEL="info"
```

## 认证方式

### 1. 默认认证

使用客户端 ID 格式：`GID_test@@@00:11:22:33:44:55`

```javascript
const client = mqtt.connect('mqtt://localhost:1883', {
    clientId: 'GID_test@@@00:11:22:33:44:55',
    username: 'test_user',
    password: 'test_password'
});
```

### 2. OAuth2 认证

```javascript
const client = mqtt.connect('mqtt://localhost:1883', {
    clientId: 'GID_test@@@00:11:22:33:44:55',
    username: 'oauth2:your_access_token',
    password: 'client_secret'
});
```

### 3. JWT 认证

```javascript
const client = mqtt.connect('mqtt://localhost:1883', {
    clientId: 'GID_test@@@00:11:22:33:44:55',
    username: 'jwt:your_jwt_token',
    password: 'jwt_secret'
});
```

### 4. Token 认证

```javascript
const client = mqtt.connect('mqtt://localhost:1883', {
    clientId: 'GID_test@@@00:11:22:33:44:55',
    username: 'token:your_token',
    password: 'token_secret'
});
```

## API 文档

### MQTT 消息格式

#### Hello 消息

```json
{
  "type": "hello",
  "version": 3,
  "audio_params": {
    "codec": "opus",
    "sample_rate": 16000,
    "channels": 1,
    "bitrate": 24000
  },
  "features": ["voice", "text"]
}
```

#### 音频消息

```json
{
  "type": "audio",
  "data": "hex_encoded_audio_data",
  "timestamp": 1234567890
}
```

#### 文本消息

```json
{
  "type": "text",
  "text": "Hello, World!",
  "timestamp": 1234567890
}
```

#### MCP 消息

```json
{
  "type": "mcp",
  "payload": {
    "jsonrpc": "2.0",
    "method": "tools/list",
    "id": 1,
    "params": {}
  }
}
```

#### Goodbye 消息

```json
{
  "type": "goodbye"
}
```

## 性能优化

### 编译优化

使用 release 模式编译以获得最佳性能：

```bash
cargo build --release
```

### 运行时优化

- 使用 `tokio` 异步运行时
- 启用 LTO (Link Time Optimization)
- 设置合适的线程池大小

### 配置优化

- 调整 `max_connections` 根据服务器负载
- 增加 `max_payload_size` 以支持更大的消息
- 优化 `keep_alive_timeout` 以平衡性能和可靠性

## 监控和日志

### 日志级别

- `error`: 错误信息
- `warn`: 警告信息
- `info`: 一般信息
- `debug`: 调试信息
- `trace`: 详细跟踪信息

### 日志输出

日志可以输出到控制台和文件：

```toml
[logging]
level = "info"
file = "mqtt-gateway.log"
```

### 监控指标

- 当前连接数
- 消息吞吐量
- 错误率
- 响应时间

## 故障排除

### 常见问题

1. **端口占用**
   ```
   Error: bind failed: address already in use
   ```
   解决方案：检查端口是否被占用，修改配置文件中的端口号。

2. **认证失败**
   ```
   Error: authentication failed
   ```
   解决方案：检查客户端 ID 格式和认证凭据。

3. **连接超时**
   ```
   Error: connection timeout
   ```
   解决方案：检查网络连接，增加 `keep_alive_timeout`。

### 调试模式

启用调试日志：

```bash
RUST_LOG=debug cargo run --release
```

## 开发指南

### 添加新功能

1. 在相应的模块中添加代码
2. 更新配置文件
3. 添加测试用例
4. 更新文档

### 测试

运行测试：

```bash
cargo test
```

### 代码风格

遵循 Rust 官方代码风格指南：

```bash
cargo fmt
```

检查代码质量：

```bash
cargo clippy
```

## 部署

### Docker 部署

```dockerfile
FROM rust:1.70 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bullseye-slim
COPY --from=builder /app/target/release/mqtt-gateway /usr/local/bin/
COPY config.toml /etc/mqtt-gateway/
EXPOSE 1883
CMD ["mqtt-gateway"]
```

### 系统服务

创建 systemd 服务文件：

```ini
[Unit]
Description=XiaoZhi MQTT Gateway
After=network.target

[Service]
Type=simple
User=mqtt
ExecStart=/usr/local/bin/mqtt-gateway
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
```

## 贡献指南

欢迎贡献代码！请遵循以下步骤：

1. Fork 项目
2. 创建功能分支
3. 提交更改
4. 推送到分支
5. 创建 Pull Request

## 许可证

MIT License

## 联系方式

- 项目主页：[GitHub Repository]
- 问题反馈：[Issues]
- 邮件：[Email]

## 致谢

感谢所有贡献者和用户的支持！

## 更新日志

### v0.1.0 (2024-04-09)

- 初始版本发布
- 实现基本的 MQTT 协议支持
- 添加 WebSocket 桥接功能
- 实现 UDP 音频传输
- 支持多种认证方式
- 完整的配置管理系统
