# MQTT Gateway 测试客户端

这是一个用于测试 XiaoZhi MQTT Gateway 的测试客户端。

## 安装依赖

首先需要安装 MQTT 客户端库：

```bash
npm install mqtt
```

## 配置说明

在运行测试之前，请确保 MQTT Gateway 服务正在运行。

默认配置：
- MQTT 服务器：localhost:1883
- UDP 服务器：localhost:1883
- 客户端 ID：GID_test@@@00:11:22:33:44:55
- 用户名：test_user
- 密码：test_password

如果需要修改配置，可以编辑 `test-client.js` 文件中的 `config` 对象。

## 运行测试

```bash
node test-client.js
```

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

## 测试输出示例

```
========================================
  MQTT Gateway 测试客户端
========================================

开始连接 MQTT Gateway...
✓ MQTT 连接成功
  Client ID: GID_test@@@00:11:22:33:44:55
  Username: test_user
✓ 订阅回复主题成功: devices/p2p/00:11:22:33:44:55

发送 Hello 消息...
✓ Hello 消息已发送

✓ 收到 Hello 响应:
  Session ID: session_123456
  Transport: udp
  UDP 服务器: localhost:1883
  加密方式: aes-128-ctr

设置 UDP 传输...
✓ UDP 监听中: 0.0.0.0:12345

=== 测试 1: 发送 JSON 消息 ===
✓ test 消息已发送

=== 测试 2: 发送 MCP 消息 ===
✓ MCP tools/list 消息已发送

=== 测试 3: 发送音频数据 ===
✓ 音频数据已发送 (100 字节)

=== 测试 4: 发送多条音频数据 ===
✓ 音频数据已发送 (50 字节)
✓ 音频数据已发送 (60 字节)
✓ 音频数据已发送 (70 字节)
✓ 音频数据已发送 (80 字节)
✓ 音频数据已发送 (90 字节)

=== 测试 5: 发送 Goodbye 消息 ===
✓ goodbye 消息已发送

=== 所有测试完成 ===

断开连接...
✓ UDP 连接已关闭
✓ MQTT 连接已关闭

测试完成，程序退出
```

## 自定义测试

你可以在代码中添加自定义的测试：

```javascript
// 在 runTests 方法中添加你的测试
async runTests() {
    try {
        // 连接 MQTT
        await this.connect();
        
        // 发送 Hello 消息
        await this.sendHello();
        
        // 等待一段时间，确保 UDP 设置完成
        await this.sleep(1000);
        
        // 添加你的自定义测试
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

## API 参考

### MQTTGatewayTestClient 类

#### 方法

- `connect()` - 连接到 MQTT Gateway
- `sendHello()` - 发送 Hello 消息
- `sendJSONMessage(type, payload)` - 发送 JSON 消息
- `sendMCPMessage(method, params)` - 发送 MCP 消息
- `sendAudioData(audioData, timestamp)` - 发送音频数据
- `runTests()` - 运行所有测试
- `disconnect()` - 断开连接

#### 事件

- `connect` - MQTT 连接成功
- `message` - 收到消息
- `error` - 连接错误
- `close` - 连接关闭

## 故障排除

### 连接失败

如果连接失败，请检查：
1. MQTT Gateway 服务是否正在运行
2. 端口配置是否正确
3. 防火墙是否阻止了连接

### UDP 传输失败

如果 UDP 传输失败，请检查：
1. UDP 端口是否正确
2. 网络是否允许 UDP 通信
3. 防火墙是否阻止了 UDP 端口

### 认证失败

如果认证失败，请检查：
1. 客户端 ID 格式是否正确
2. 用户名和密码是否正确
3. 网关的认证配置是否正确

## 注意事项

1. 测试客户端会自动处理连接的建立和关闭
2. 所有测试都会在完成后自动清理资源
3. 如果测试过程中出现错误，会自动断开连接
4. 音频数据是模拟数据，实际应用中需要使用真实的音频编码数据
