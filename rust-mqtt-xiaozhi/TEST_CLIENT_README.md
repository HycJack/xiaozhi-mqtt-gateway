# Rust MQTT Gateway 测试客户端

这是一个用于测试 Rust 版本的 XiaoZhi MQTT Gateway 的测试客户端。

## 安装依赖

```bash
npm install
```

## 配置说明

默认配置：
- MQTT 服务器：localhost:1883
- 客户端 ID：GID_test@@@00:11:22:33:44:55
- 用户名：test_user
- 密码：test_password

如果需要修改配置，编辑 `test-client.js` 文件中的 `config` 对象。

## 运行测试

### 1. 启动 Rust MQTT Gateway

首先确保 Rust MQTT Gateway 正在运行：

```bash
cargo run --release
```

### 2. 运行测试客户端

```bash
npm test
```

或者直接运行：

```bash
node test-client.js
```

## 测试内容

测试客户端会执行以下测试：

1. **连接测试**：连接到 Rust MQTT Gateway
2. **Hello 消息测试**：发送 Hello 消息进行会话初始化
3. **测试消息测试**：发送测试 JSON 消息
4. **音频消息测试**：发送模拟音频数据
5. **文本消息测试**：发送文本消息
6. **MCP 消息测试**：发送 MCP 协议消息
7. **Goodbye 消息测试**：发送会话结束消息

## 测试输出示例

```
========================================
  Rust MQTT Gateway 测试客户端
========================================

连接到 Rust MQTT Gateway...
✓ 连接成功
  Client ID: GID_test@@@00:11:22:33:44:55
  Username: test_user
✓ 订阅回复主题成功: devices/p2p/00:11:22:33:44:55

发送 Hello 消息...
✓ Hello 消息已发送

收到消息: devices/p2p/00:11:22:33:44:55
  类型: hello
  内容: {
  "type": "hello",
  "version": 3,
  "session_id": "uuid",
  "transport": "udp",
  "udp": {
    "server": "localhost",
    "port": 1883,
    "encryption": "aes-128-ctr",
    "key": "hex_key",
    "nonce": "hex_nonce"
  },
  "audio_params": {
    "codec": "opus",
    "sample_rate": 16000,
    "channels": 1,
    "bitrate": 24000
  }
}

发送测试消息...
✓ 测试消息已发送

发送音频消息...
✓ 音频消息已发送

发送文本消息...
✓ 文本消息已发送

发送 MCP 消息...
✓ MCP 消息已发送

发送 Goodbye 消息...
✓ Goodbye 消息已发送

=== 测试完成 ===

断开连接...
✓ 连接已关闭

测试完成，程序退出
```

## 自定义测试

你可以在 `test-client.js` 中添加自定义的测试：

```javascript
async runTests() {
    try {
        await this.connect();
        
        await this.sleep(1000);
        this.sendHello();
        
        // 添加你的自定义测试
        console.log('\n=== 自定义测试 ===');
        this.sendCustomMessage();
        
        await this.sleep(2000);
        
    } catch (error) {
        console.error('测试失败:', error);
    } finally {
        this.disconnect();
    }
}
```

## API 参考

### RustMqttTestClient 类

#### 主要方法

- `connect()` - 连接到 MQTT Gateway
- `sendHello()` - 发送 Hello 消息
- `sendTestMessage()` - 发送测试消息
- `sendAudioMessage()` - 发送音频消息
- `sendTextMessage()` - 发送文本消息
- `sendMcpMessage()` - 发送 MCP 消息
- `sendGoodbye()` - 发送 Goodbye 消息
- `runTests()` - 运行所有测试
- `disconnect()` - 断开连接

#### 工具方法

- `sleep(ms)` - 延迟指定毫秒数

## 故障排除

### 连接失败

如果连接失败，请检查：
1. Rust MQTT Gateway 服务是否正在运行
2. 端口配置是否正确
3. 防火墙是否阻止了连接

### 认证失败

如果认证失败，检查：
1. 客户端 ID 格式是否正确：`GID_test@@@00:11:22:33:44:55`
2. 用户名和密码是否正确
3. 网关的认证配置是否正确

### 消息发送失败

如果消息发送失败，检查：
1. 连接是否正常
2. 消息格式是否正确
3. 网关是否正确处理消息

## 注意事项

1. 确保 Rust MQTT Gateway 服务正在运行
2. 确保端口 1883 没有被占用
3. 确保网络连接正常
4. 测试客户端会自动处理连接的建立和关闭
5. 所有测试都会在完成后自动清理资源
6. 如果测试过程中出现错误，会自动断开连接

## 下一步

1. 根据测试结果调整配置
2. 修改测试客户端以适应你的需求
3. 添加更多的测试用例
4. 集成到你的测试框架中
5. 监控网关的日志，分析消息处理情况
