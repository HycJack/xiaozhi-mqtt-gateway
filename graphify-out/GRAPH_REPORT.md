# Graph Report - c:\Users\huangyicao\Downloads\xiaozhi-mqtt-gateway  (2026-04-09)

## Corpus Check
- Corpus is ~3,886 words - fits in a single context window. You may not need a graph.

## Summary
- 76 nodes · 116 edges · 10 communities detected
- Extraction: 60% EXTRACTED · 40% INFERRED · 0% AMBIGUOUS · INFERRED: 46 edges (avg confidence: 0.5)
- Token cost: 0 input · 0 output

## God Nodes (most connected - your core abstractions)
1. `MQTTConnection` - 19 edges
2. `MQTTProtocol` - 19 edges
3. `MQTTServer` - 11 edges
4. `WebSocketBridge` - 8 edges
5. `ConfigManager` - 7 edges
6. `generatePasswordSignature()` - 3 edges
7. `validateMqttCredentials()` - 2 edges
8. `generateMqttConfig()` - 2 edges

## Surprising Connections (you probably didn't know these)
- None detected - all connections are within the same source files.

## Communities

### Community 0 - "Community 0"
Cohesion: 0.2
Nodes (1): MQTTConnection

### Community 1 - "Community 1"
Cohesion: 0.24
Nodes (1): MQTTServer

### Community 2 - "Community 2"
Cohesion: 0.22
Nodes (1): MQTTProtocol

### Community 3 - "Community 3"
Cohesion: 0.22
Nodes (1): WebSocketBridge

### Community 4 - "Community 4"
Cohesion: 0.36
Nodes (1): ConfigManager

### Community 5 - "Community 5"
Cohesion: 0.38
Nodes (0): 

### Community 6 - "Community 6"
Cohesion: 0.83
Nodes (3): generateMqttConfig(), generatePasswordSignature(), validateMqttCredentials()

### Community 7 - "Community 7"
Cohesion: 0.67
Nodes (3): Config Manager, MQTT Gateway, MQTT Protocol

### Community 8 - "Community 8"
Cohesion: 1.0
Nodes (0): 

### Community 9 - "Community 9"
Cohesion: 1.0
Nodes (0): 

## Knowledge Gaps
- **Thin community `Community 8`** (2 nodes): `.parsePingReq()`, `.sendPingResp()`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 9`** (1 nodes): `ecosystem.config.js`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.