const mqtt = require('mqtt');
const dgram = require('dgram');
const crypto = require('crypto');

// 配置参数
const config = {
    mqttHost: 'localhost',
    mqttPort: 1885,
    udpHost: 'localhost',
    udpPort: 1886,
    clientId: 'GID_test@@@00:11:22:33:44:55',
    username: 'test_user',
    password: 'test_password'
};

class MQTTGatewayTestClient {
    constructor(config) {
        this.config = config;
        this.mqttClient = null;
        this.udpClient = null;
        this.udpSocket = null;
        this.sessionId = null;
        this.udpConfig = null;
    }

    async connect() {
        console.log('开始连接 MQTT Gateway...');
        
        // 连接 MQTT 服务器
        this.mqttClient = mqtt.connect(`mqtt://${this.config.mqttHost}:${this.config.mqttPort}`, {
            clientId: this.config.clientId,
            username: this.config.username,
            password: this.config.password,
            clean: true,
            connectTimeout: 10000
        });

        return new Promise((resolve, reject) => {
            this.mqttClient.on('connect', () => {
                console.log('✓ MQTT 连接成功');
                console.log('  Client ID:', this.config.clientId);
                console.log('  Username:', this.config.username);
                
                // 订阅回复主题
                const replyTopic = `devices/p2p/${this.config.clientId.split('@@@')[1]}`;
                this.mqttClient.subscribe(replyTopic, (err) => {
                    if (err) {
                        console.error('订阅主题失败:', err);
                        reject(err);
                    } else {
                        console.log('✓ 订阅回复主题成功:', replyTopic);
                        resolve();
                    }
                });
            });

            this.mqttClient.on('error', (err) => {
                console.error('✗ MQTT 连接错误:', err.message);
                reject(err);
            });

            this.mqttClient.on('close', () => {
                console.log('MQTT 连接已关闭');
            });

            // 监听消息
            this.mqttClient.on('message', (topic, message) => {
                this.handleMessage(topic, message);
            });
        });
    }

    async sendHello() {
        console.log('\n发送 Hello 消息...');
        
        const helloMessage = {
            type: 'hello',
            version: 3,
            audio_params: {
                codec: 'opus',
                sample_rate: 16000,
                channels: 1,
                bitrate: 24000
            },
            features: ['voice', 'text']
        };

        const topic = 'devices/p2p/gateway';
        this.mqttClient.publish(topic, JSON.stringify(helloMessage));
        console.log('✓ Hello 消息已发送');
        
        // 等待 Hello 响应
        await this.waitForHelloResponse();
    }

    async waitForHelloResponse() {
        return new Promise((resolve, reject) => {
            const timeout = setTimeout(() => {
                reject(new Error('等待 Hello 响应超时'));
            }, 10000);

            this.mqttClient.once('message', (topic, message) => {
                clearTimeout(timeout);
                try {
                    const response = JSON.parse(message.toString());
                    if (response.type === 'hello') {
                        console.log('\n✓ 收到 Hello 响应:');
                        console.log('  Session ID:', response.session_id);
                        console.log('  Transport:', response.transport);
                        
                        if (response.udp) {
                            this.udpConfig = {
                                server: response.udp.server,
                                port: response.udp.port,
                                encryption: response.udp.encryption,
                                key: Buffer.from(response.udp.key, 'hex'),
                                nonce: Buffer.from(response.udp.nonce, 'hex')
                            };
                            console.log('  UDP 服务器:', `${this.udpConfig.server}:${this.udpConfig.port}`);
                            console.log('  加密方式:', this.udpConfig.encryption);
                            
                            this.sessionId = response.session_id;
                            this.setupUDP();
                        }
                        resolve(response);
                    }
                } catch (error) {
                    reject(error);
                }
            });
        });
    }

    setupUDP() {
        console.log('\n设置 UDP 传输...');
        
        this.udpSocket = dgram.createSocket('udp4');
        
        this.udpSocket.on('message', (msg, rinfo) => {
            this.handleUDPMessage(msg, rinfo);
        });

        this.udpSocket.on('error', (err) => {
            console.error('UDP 错误:', err);
        });

        this.udpSocket.on('listening', () => {
            const address = this.udpSocket.address();
            console.log('✓ UDP 监听中:', `${address.address}:${address.port}`);
        });

        // 绑定到随机端口
        this.udpSocket.bind(0);
    }

    sendJSONMessage(type, payload = {}) {
        const message = {
            type: type,
            session_id: this.sessionId,
            ...payload
        };

        const topic = 'devices/p2p/gateway';
        this.mqttClient.publish(topic, JSON.stringify(message));
        console.log(`✓ ${type} 消息已发送`);
    }

    sendMCPMessage(method, params = {}) {
        const message = {
            type: 'mcp',
            session_id: this.sessionId,
            payload: {
                jsonrpc: '2.0',
                method: method,
                id: Date.now(),
                params: params
            }
        };

        const topic = 'devices/p2p/gateway';
        this.mqttClient.publish(topic, JSON.stringify(message));
        console.log(`✓ MCP ${method} 消息已发送`);
    }

    sendAudioData(audioData, timestamp = Date.now()) {
        if (!this.udpConfig) {
            console.error('UDP 配置未初始化，无法发送音频数据');
            return;
        }

        // 生成 UDP 头部
        const header = Buffer.alloc(16);
        header.writeUInt8(1, 0); // 类型
        header.writeUInt8(0, 1); // 标志
        header.writeUInt16BE(audioData.length, 2); // 长度
        header.writeUInt32BE(0, 4); // 连接ID (暂时使用0)
        header.writeUInt32BE(timestamp, 8); // 时间戳
        header.writeUInt32BE(0, 12); // 序列号 (暂时使用0)

        // 加密音频数据
        const cipher = crypto.createCipheriv(this.udpConfig.encryption, this.udpConfig.key, header);
        const encryptedAudio = Buffer.concat([
            cipher.update(audioData),
            cipher.final()
        ]);

        // 发送 UDP 消息
        const message = Buffer.concat([header, encryptedAudio]);
        this.udpSocket.send(message, this.udpConfig.port, this.udpConfig.host);
        console.log(`✓ 音频数据已发送 (${audioData.length} 字节)`);
    }

    handleMessage(topic, message) {
        try {
            const data = JSON.parse(message.toString());
            console.log('\n收到消息:', topic);
            console.log('  类型:', data.type);
            console.log('  内容:', JSON.stringify(data, null, 2));
        } catch (error) {
            console.error('解析消息失败:', error);
        }
    }

    handleUDPMessage(msg, rinfo) {
        console.log('\n收到 UDP 消息:', rinfo);
        
        if (msg.length < 16) {
            console.error('UDP 消息太短');
            return;
        }

        try {
            // 解析 UDP 头部
            const type = msg.readUInt8(0);
            const payloadLength = msg.readUInt16BE(2);
            const timestamp = msg.readUInt32BE(8);
            const sequence = msg.readUInt32BE(12);

            console.log('  类型:', type);
            console.log('  长度:', payloadLength);
            console.log('  时间戳:', timestamp);
            console.log('  序列号:', sequence);

            // 解密音频数据
            const header = msg.slice(0, 16);
            const encryptedPayload = msg.slice(16, 16 + payloadLength);
            const decipher = crypto.createDecipheriv(
                this.udpConfig.encryption,
                this.udpConfig.key,
                header
            );
            const audioData = Buffer.concat([
                decipher.update(encryptedPayload),
                decipher.final()
            ]);

            console.log('  音频数据长度:', audioData.length);
        } catch (error) {
            console.error('处理 UDP 消息失败:', error);
        }
    }

    async runTests() {
        try {
            // 连接 MQTT
            await this.connect();
            
            // 发送 Hello 消息
            await this.sendHello();
            
            // 等待一段时间，确保 UDP 设置完成
            await this.sleep(1000);
            
            // 测试 1: 发送 JSON 消息
            console.log('\n=== 测试 1: 发送 JSON 消息 ===');
            this.sendJSONMessage('test', {
                message: '这是一条测试消息',
                timestamp: Date.now()
            });
            
            await this.sleep(2000);
            
            // 测试 2: 发送 MCP 消息
            console.log('\n=== 测试 2: 发送 MCP 消息 ===');
            this.sendMCPMessage('tools/list', {});
            
            await this.sleep(2000);
            
            // 测试 3: 发送音频数据
            console.log('\n=== 测试 3: 发送音频数据 ===');
            const testAudioData = Buffer.alloc(100); // 模拟音频数据
            testAudioData.fill(0xAA);
            this.sendAudioData(testAudioData);
            
            await this.sleep(2000);
            
            // 测试 4: 发送多条音频数据
            console.log('\n=== 测试 4: 发送多条音频数据 ===');
            for (let i = 0; i < 5; i++) {
                const audioData = Buffer.alloc(50 + i * 10);
                audioData.fill(0xBB + i);
                this.sendAudioData(audioData);
                await this.sleep(500);
            }
            
            await this.sleep(2000);
            
            // 测试 5: 发送 Goodbye 消息
            console.log('\n=== 测试 5: 发送 Goodbye 消息 ===');
            this.sendJSONMessage('goodbye', {
                session_id: this.sessionId
            });
            
            await this.sleep(2000);
            
            console.log('\n=== 所有测试完成 ===');
            
        } catch (error) {
            console.error('测试失败:', error);
        } finally {
            this.disconnect();
        }
    }

    disconnect() {
        console.log('\n断开连接...');
        
        if (this.udpSocket) {
            this.udpSocket.close();
            console.log('✓ UDP 连接已关闭');
        }
        
        if (this.mqttClient) {
            this.mqttClient.end();
            console.log('✓ MQTT 连接已关闭');
        }
    }

    sleep(ms) {
        return new Promise(resolve => setTimeout(resolve, ms));
    }
}

// 运行测试
async function main() {
    console.log('========================================');
    console.log('  MQTT Gateway 测试客户端');
    console.log('========================================\n');
    
    const client = new MQTTGatewayTestClient(config);
    await client.runTests();
    
    console.log('\n测试完成，程序退出');
    process.exit(0);
}

// 错误处理
process.on('uncaughtException', (err) => {
    console.error('未捕获的异常:', err);
    process.exit(1);
});

process.on('unhandledRejection', (reason, promise) => {
    console.error('未处理的 Promise 拒绝:', reason);
    process.exit(1);
});

// 启动测试
if (require.main === module) {
    main().catch(err => {
        console.error('主程序错误:', err);
        process.exit(1);
    });
}

module.exports = MQTTGatewayTestClient;
