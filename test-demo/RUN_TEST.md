# MQTT Gateway 测试 Demo 运行说明

## 目录结构

```
test-demo/
├── package.json          # 项目配置文件
├── test-client.js       # 测试客户端代码
├── README.md           # 使用说明文档
└── node_modules/       # 依赖包（npm install 后生成）
```

## 运行步骤

### 1. 启动 MQTT Gateway 服务

确保 MQTT Gateway 服务正在运行：

```bash
# 在项目根目录运行
cd c:\Users\huangyicao\Downloads\xiaozhi-mqtt-gateway
npm start
```

### 2. 进入测试目录

```bash
cd test-demo
```

### 3. 安装依赖（如果还没有安装）

```bash
npm install
```

### 4. 运行测试客户端

```bash
node test-client.js
```

## 测试结果分析

### 成功的连接

测试客户端已经成功连接到 MQTT Gateway：

```
========================================
  MQTT Gateway 测试客户端
========================================

开始连接 MQTT Gateway...
✓ MQTT 连接成功
  Client ID: GID_test@@@00:11:22:33:44:55
  Username: test_user

收到消息: devices/p2p/00:11:22:33:44:55
  类型: mcp
  内容: {
  "type": "mcp",
  "payload": {
    "jsonrpc": "2.0",
    "method": "initialize",
    "id": 10000,
    "params": {
      "protocolVersion": "2024-11-05",
      "capabilities": {},
      "clientInfo": {
        "name": "xiaozhi-mqtt-client",
        "version": "1.0.0"
      }
    }
  }
}
```

### 连接状态

- ✓ MQTT 连接成功
- ✓ 订阅回复主题成功
- ✓ 收到网关的 MCP initialize 消息
- ✓ Hello 消息已发送

## 测试内容

测试客户端会执行以下测试：

1. **连接测试**：连接到 MQTT Gateway
2. **Hello 消息测试**：发送 Hello 消息并接收响应
3. **UDP 传输测试**：设置 UDP 音频传输
4. **JSON 消息测试**：发送测试 JSON 消息
5. **MCP 消息测试**：发送 MCP 工具列表请求
6. **音频数据测试**：发送模拟音频数据
7. **多条音频测试**：连续发送多条音频数据
8. **Goodbye 消息测试**：发送会话结束消息

## 配置说明

### 默认配置

```javascript
const config = {
    mqttHost: 'localhost',
    mqttPort: 1883,
    udpHost: 'localhost',
    udpPort: 1883,
    clientId: 'GID_test@@@00:11:22:33:44:55',
    username: 'test_user',
    password: 'test_password'
};
```

### 修改配置

如果需要修改配置，编辑 `test-client.js` 文件中的 `config` 对象：

```javascript
const config = {
    mqttHost: 'your-mqtt-host',      // 修改 MQTT 服务器地址
    mqttPort: 1883,                  // 修改 MQTT 端口
    udpHost: 'your-udp-host',        // 修改 UDP 服务器地址
    udpPort: 1883,                   // 修改 UDP 端口
    clientId: 'GID_test@@@00:11:22:33:44:55', // 修改客户端 ID
    username: 'your-username',        // 修改用户名
    password: 'your-password'         // 修改密码
};
```

## 故障排除

### 端口占用错误

如果看到 `EADDRINUSE` 错误：

```
Error: listen EADDRINUSE: address already in use :::1883
```

说明端口 1883 已被占用。检查是否有其他进程正在使用该端口：

```bash
netstat -ano | findstr :1883
```

### 连接超时

如果测试客户端在等待 Hello 响应时超时：

1. 检查 MQTT Gateway 服务是否正常运行
2. 检查网络连接是否正常
3. 检查防火墙设置
4. 查看网关日志，确认消息是否被正确处理

### 认证失败

如果认证失败，检查：

1. 客户端 ID 格式是否正确：`GID_test@@@00:11:22:33:44:55`
2. 用户名和密码是否正确
3. 网关的认证配置是否正确

## 自定义测试

你可以在 `test-client.js` 中添加自定义的测试：

```javascript
async runTests() {
    try {
        // 连接 MQTT
        await this.connect();
        
        // 发送 Hello 消息
        await this.sendHello();
        
        // 等待一段时间，确保 UDP 设置完成
        await this.sleep(1000);
        
        // 添加你的自定义测试
        console.log('\n=== 自定义测试 ===');
        this.sendJSONMessage('custom_test', {
            message: '自定义测试消息',
            data: { /* 你的数据 */ }
        });
        
        await this.sleep(2000);
        
    } catch (error) {
        console.error('测试失败:', error);
    } finally {
        this.disconnect();
    }
}
```

## 测试 API 参考

### MQTTGatewayTestClient 类

#### 主要方法

- `connect()` - 连接到 MQTT Gateway
- `sendHello()` - 发送 Hello 消息
- `sendJSONMessage(type, payload)` - 发送 JSON 消息
- `sendMCPMessage(method, params)` - 发送 MCP 消息
- `sendAudioData(audioData, timestamp)` - 发送音频数据
- `runTests()` - 运行所有测试
- `disconnect()` - 断开连接

#### 工具方法

- `sleep(ms)` - 延迟指定毫秒数

## 注意事项

1. 确保 MQTT Gateway 服务正在运行
2. 确保端口 1883 没有被占用
3. 确保网络连接正常
4. 测试客户端会自动处理连接的建立和关闭
5. 所有测试都会在完成后自动清理资源
6. 如果测试过程中出现错误，会自动断开连接
7. 音频数据是模拟数据，实际应用中需要使用真实的音频编码数据

## 下一步

1. 根据测试结果调整配置
2. 修改测试客户端以适应你的需求
3. 添加更多的测试用例
4. 集成到你的测试框架中
5. 监控网关的日志，分析消息处理情况
