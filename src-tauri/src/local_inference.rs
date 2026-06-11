//! 本地模型推理 — 通过 Ollama HTTP API
//!
//! 提供:
//! - check_ollama() → 检测 Ollama 是否运行 + 列出可用模型
//! - chat_ollama_stream() → 向 Ollama 发消息, 流式推送事件给前端
//!
//! 发射的事件格式与 Claude CLI 兼容，前端无需大改。

use crate::events::emit_to_frontend;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::AppHandle;

const OLLAMA_BASE: &str = "http://localhost:11434";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OllamaModel {
    pub name: String,
    pub size: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OllamaStatus {
    pub running: bool,
    pub models: Vec<OllamaModel>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LocalChatParams {
    /// 前端传来的会话标识 (stdin_id 风格)
    pub stdin_id: String,
    /// Ollama 模型名 (如 qwen2.5-coder:7b)
    pub model: String,
    /// 对话历史
    pub messages: Vec<LocalMessage>,
    /// 系统提示词（可选）
    pub system: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LocalMessage {
    pub role: String,
    pub content: String,
}

/// 检测 Ollama 是否在运行并返回可用模型列表
pub async fn check_ollama() -> OllamaStatus {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .unwrap_or_default();

    match client
        .get(format!("{}/api/tags", OLLAMA_BASE))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            match resp.json::<Value>().await {
                Ok(json) => {
                    let models = json["models"]
                        .as_array()
                        .map(|arr| {
                            arr.iter()
                                .map(|m| OllamaModel {
                                    name: m["name"].as_str().unwrap_or("unknown").to_string(),
                                    size: m["size"].as_u64().unwrap_or(0),
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    OllamaStatus {
                        running: true,
                        models,
                        error: None,
                    }
                }
                Err(e) => OllamaStatus {
                    running: true,
                    models: vec![],
                    error: Some(format!("模型列表解析失败: {}", e)),
                },
            }
        }
        Ok(resp) => OllamaStatus {
            running: false,
            models: vec![],
            error: Some(format!("Ollama 返回 {}", resp.status())),
        },
        Err(e) => OllamaStatus {
            running: false,
            models: vec![],
            error: Some(format!("Ollama 未运行: {}", e)),
        },
    }
}

/// 向 Ollama 发送聊天请求，通过 Tauri 事件流式推送给前端。
///
/// 发射的事件:
///   local:stream:{stdin_id}  — 流式文本 (content_block_delta 等)
///   local:exit:{stdin_id}    — 会话结束 (退出码 0)
///
/// 事件 payload 格式与 Claude CLI 兼容:
///   {"type":"system","subtype":"init",...}
///   {"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"..."}}}
///   {"type":"result","subtype":"success","result":"...",...}
pub async fn chat_ollama_stream(
    app: AppHandle,
    params: LocalChatParams,
) -> Result<(), String> {
    let stream_channel = format!("local:stream:{}", params.stdin_id);
    let exit_channel = format!("local:exit:{}", params.stdin_id);

    // —— 构建 Ollama 请求 ——
    let mut ollama_msgs: Vec<Value> = Vec::new();

    if let Some(ref sys) = params.system {
        if !sys.is_empty() {
            ollama_msgs.push(serde_json::json!({
                "role": "system",
                "content": sys,
            }));
        }
    }

    for m in &params.messages {
        ollama_msgs.push(serde_json::json!({
            "role": m.role,
            "content": m.content,
        }));
    }

    let request_body = serde_json::json!({
        "model": params.model,
        "messages": ollama_msgs,
        "stream": true,
    });

    // —— 发请求 ——
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/api/chat", OLLAMA_BASE))
        .json(&request_body)
        .timeout(std::time::Duration::from_secs(300))
        .send()
        .await
        .map_err(|e| format!("连接 Ollama 失败: {}", e))?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Ollama 错误 {}: {}", status, body));
    }

    // —— 发射 system:init ——
    let init = serde_json::json!({
        "type": "system",
        "subtype": "init",
        "session_id": params.stdin_id,
        "model": params.model,
    });
    let _ = emit_to_frontend(&app, &stream_channel, init);

    // —— 发射 content_block_start ——
    let cbs = serde_json::json!({
        "type": "stream_event",
        "event": {
            "type": "content_block_start",
            "index": 0,
            "content_block": { "type": "text", "text": "" }
        }
    });
    let _ = emit_to_frontend(&app, &stream_channel, cbs);

    // —— 流式读取 Ollama 响应 ——
    let mut full_text = String::new();
    let mut input_tokens: u64 = 0;
    let mut output_tokens: u64 = 0;
    let mut buf = Vec::new();

    use futures_util::StreamExt;
    let mut byte_stream = resp.bytes_stream();

    while let Some(chunk_result) = byte_stream.next().await {
        match chunk_result {
            Ok(bytes) => {
                buf.extend_from_slice(&bytes);

                // 按换行切分, 处理完整的 JSON 行
                while let Some(pos) = buf.iter().position(|&b| b == b'\n') {
                    let line_bytes: Vec<u8> = buf.drain(..=pos).collect();
                    let line = String::from_utf8_lossy(&line_bytes[..line_bytes.len() - 1]);

                    if line.trim().is_empty() {
                        continue;
                    }

                    if let Ok(json) = serde_json::from_str::<Value>(&line) {
                        // 提取文本增量
                        if let Some(content) = json["message"]["content"].as_str() {
                            full_text.push_str(content);

                            let delta = serde_json::json!({
                                "type": "stream_event",
                                "event": {
                                    "type": "content_block_delta",
                                    "index": 0,
                                    "delta": {
                                        "type": "text_delta",
                                        "text": content
                                    }
                                }
                            });
                            let _ = emit_to_frontend(&app, &stream_channel, delta);
                        }

                        // 跟踪 token 计数 (Ollama 在最后一条返回)
                        if let Some(c) = json["prompt_eval_count"].as_u64() {
                            input_tokens = c;
                        }
                        if let Some(c) = json["eval_count"].as_u64() {
                            output_tokens = c;
                        }

                        // done=true → 结束
                        if json.get("done").and_then(|v| v.as_bool()).unwrap_or(false) {
                            break;
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("[NOVA] Ollama 流读取错误: {}", e);
                break;
            }
        }
    }

    // —— 发射 content_block_stop ——
    let cbe = serde_json::json!({
        "type": "stream_event",
        "event": { "type": "content_block_stop", "index": 0 }
    });
    let _ = emit_to_frontend(&app, &stream_channel, cbe);

    // —— 发射 message_delta + usage ——
    let msg_delta = serde_json::json!({
        "type": "stream_event",
        "event": {
            "type": "message_delta",
            "delta": { "stop_reason": "end_turn" },
            "usage": {
                "input_tokens": input_tokens,
                "output_tokens": output_tokens
            }
        }
    });
    let _ = emit_to_frontend(&app, &stream_channel, msg_delta);

    // —— 发射 message_stop ——
    let msg_stop = serde_json::json!({
        "type": "stream_event",
        "event": { "type": "message_stop" }
    });
    let _ = emit_to_frontend(&app, &stream_channel, msg_stop);

    // —— 发射 result ——
    let result = serde_json::json!({
        "type": "result",
        "subtype": "success",
        "result": full_text,
        "session_id": params.stdin_id,
    });
    let _ = emit_to_frontend(&app, &stream_channel, result);

    // —— 发射 process_exit ——
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let exit_payload = serde_json::json!(0);
    let _ = emit_to_frontend(&app, &exit_channel, exit_payload);

    Ok(())
}
