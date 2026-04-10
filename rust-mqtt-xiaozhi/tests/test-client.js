const mqtt = require('mqtt');

const config = {
    mqttHost: 'localhost',
    mqttPort: 1883,
    clientId: 'GID_test@@@00:11:22:33:44:55',
    // 注意：根据服务器配置，可能不需要用户名和密码
    username: '',
    password: ''
};

class RustMqttTestClient {
    constructor(config) {
        this.config = config;
        this.mqttClient = null;
    }

    async connect() {
        console.log('连接到 Rust MQTT Gateway...');
        
        return new Promise((resolve, reject) => {
            // 构建连接选项
            const options = {
                clientId: this.config.clientId,
                clean: true,
                connectTimeout: 10000
            };
            
            // 只有在提供了用户名和密码时才添加
            if (this.config.username && this.config.password) {
                options.username = this.config.username;
                options.password = this.config.password;
            }
            
            this.mqttClient = mqtt.connect(`mqtt://${this.config.mqttHost}:${this.config.mqttPort}`, options);

            this.mqttClient.on('connect', () => {
                console.log('✓ 连接成功');
                console.log('  Client ID:', this.config.clientId);
                if (this.config.username) {
                    console.log('  Username:', this.config.username);
                }
                
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
                console.error('✗ 连接错误:', err.message);
                reject(err);
            });

            this.mqttClient.on('message', (topic, message) => {
                this.handleMessage(topic, message);
            });
        });
    }

    sendHello() {
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
    }

    sendTestMessage() {
        console.log('\n发送测试消息...');
        
        const testMessage = {
            type: 'test',
            message: '这是一条测试消息',
            timestamp: Date.now()
        };

        const topic = 'devices/p2p/gateway';
        this.mqttClient.publish(topic, JSON.stringify(testMessage));
        console.log('✓ 测试消息已发送');
    }

    sendAudioMessage() {
        console.log('\n发送音频消息...');
        
        const audioMessage = {
            type: 'audio',
            data: Buffer.alloc(100).fill(0xAA).toString('hex'),
            timestamp: Date.now()
        };

        const topic = 'devices/p2p/gateway';
        this.mqttClient.publish(topic, JSON.stringify(audioMessage));
        console.log('✓ 音频消息已发送');
    }

    sendTextMessage() {
        console.log('\n发送文本消息...');
        
        const textMessage = {
            type: 'text',
            text: 'Hello from Rust MQTT Gateway!',
            timestamp: Date.now()
        };

        const topic = 'devices/p2p/gateway';
        this.mqttClient.publish(topic, JSON.stringify(textMessage));
        console.log('✓ 文本消息已发送');
    }

    sendMcpMessage() {
        console.log('\n发送 MCP 消息...');
        
        const mcpMessage = {
            type: 'mcp',
            payload: {
                jsonrpc: '2.0',
                method: 'tools/list',
                id: Date.now(),
                params: {}
            }
        };

        const topic = 'devices/p2p/gateway';
        this.mqttClient.publish(topic, JSON.stringify(mcpMessage));
        console.log('✓ MCP 消息已发送');
    }

    sendGoodbye() {
        console.log('\n发送 Goodbye 消息...');
        
        const goodbyeMessage = {
            type: 'goodbye'
        };

        const topic = 'devices/p2p/gateway';
        this.mqttClient.publish(topic, JSON.stringify(goodbyeMessage));
        console.log('✓ Goodbye 消息已发送');
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

    disconnect() {
        console.log('\n断开连接...');
        
        if (this.mqttClient) {
            this.mqttClient.end();
            console.log('✓ 连接已关闭');
        }
    }

    async runTests() {
        try {
            await this.connect();
            
            await this.sleep(1000);
            this.sendHello();
            
            await this.sleep(2000);
            this.sendTestMessage();
            
            await this.sleep(2000);
            this.sendAudioMessage();
            
            await this.sleep(2000);
            this.sendTextMessage();
            
            await this.sleep(2000);
            this.sendMcpMessage();
            
            await this.sleep(2000);
            this.sendGoodbye();
            
            await this.sleep(2000);
            
            console.log('\n=== 测试完成 ===');
            
        } catch (error) {
            console.error('测试失败:', error);
        } finally {
            this.disconnect();
        }
    }

    sleep(ms) {
        return new Promise(resolve => setTimeout(resolve, ms));
    }
}

async function main() {
    console.log('========================================');
    console.log('  Rust MQTT Gateway 测试客户端');
    console.log('========================================\n');
    
    console.log('配置信息:');
    console.log('  MQTT 服务器:', `${config.mqttHost}:${config.mqttPort}`);
    console.log('  Client ID:', config.clientId);
    if (config.username) {
        console.log('  Username:', config.username);
    }
    console.log('');
    
    const client = new RustMqttTestClient(config);
    await client.runTests();
    
    console.log('\n测试完成，程序退出');
    process.exit(0);
}

process.on('uncaughtException', (err) => {
    console.error('未捕获的异常:', err);
    process.exit(1);
});

process.on('unhandledRejection', (reason, promise) => {
    console.error('未处理的 Promise 拒绝:', reason);
    process.exit(1);
});

if (require.main === module) {
    main().catch(err => {
        console.error('主程序错误:', err);
        process.exit(1);
    });
}

module.exports = RustMqttTestClient;
