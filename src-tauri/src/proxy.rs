//! HTTP 代理：把 CLI 的 Anthropic 格式请求转成 OpenAI 格式，让 Agnes AI 正常工作。
//!
//! Claude Code CLI 用 Anthropic Messages API（/v1/messages + Anthropic tools 格式）。
//! Agnes AI 后端是 vLLM，它的 Anthropic 兼容层处理 tools 时有 bug——要求 OpenAI
//! 风格的 `{"type":"function","function":{...}}` 嵌套。
//!
//! 这个代理在本地起一个 HTTP 服务器，接收 CLI 的 Anthropic 请求，转换成 OpenAI
//! 格式发给 Agnes AI，再把 OpenAI 响应转回 Anthropic 格式还给 CLI。

use bytes::Bytes;
use http_body_util::{combinators::BoxBody, BodyExt, Full};
use hyper::{
    body::Incoming,
    header::{HeaderValue, CONTENT_TYPE},
    server::conn::http1,
    service::service_fn,
    Method, Request, Response, StatusCode,
};
use hyper_util::rt::TokioIo;
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio_stream::StreamExt;

/// 统一的响应 Body 类型，兼容流式和非流式
type ProxyBody = BoxBody<Bytes, hyper::Error>;

/// 写日志到文件（因为 Tauri 可能不输出 stderr）
macro_rules! proxy_log {
    ($($arg:tt)*) => {{
        let msg = format!($($arg)*);
        let full_msg = format!("[AgnesProxy] {}", msg);
        eprintln!("{}", full_msg);
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("/tmp/nova_proxy.log")
        {
            use std::io::Write;
            let _ = writeln!(file, "[{}] {}", chrono_now(), full_msg);
        }
    }};
}

fn chrono_now() -> String {
    // 简单的秒级时间戳，避免引入 chrono 依赖
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| format!("{}", d.as_secs()))
        .unwrap_or_else(|_| "?".to_string())
}

/// 创建非流式响应的 body
fn full_body(data: impl Into<Bytes>) -> ProxyBody {
    Full::new(data.into())
        .map_err(|never: std::convert::Infallible| match never {})
        .boxed()
}

// ── 公共接口 ────────────────────────────────────────────────────────────

pub struct AgnesProxy {
    pub port: u16,
}

/// 启动代理。target_url 是 Agnes AI 的 base URL（如 https://apihub.agnes-ai.com/v1），
/// api_key 是用户的 API Key。
/// 返回 AgnesProxy 实例，其中 port 是实际绑定的本地端口。
pub async fn start_proxy(
    target_url: String,
    api_key: String,
) -> Result<AgnesProxy, Box<dyn std::error::Error + Send + Sync>> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();

    proxy_log!(" 代理启动在 127.0.0.1:{port} → {target_url}");

    tokio::spawn(async move {
        loop {
            let (stream, peer) = match listener.accept().await {
                Ok(conn) => conn,
                Err(e) => {
                    proxy_log!(" accept 错误: {e}");
                    return;
                }
            };

            let target = target_url.clone();
            let key = api_key.clone();

            tokio::spawn(async move {
                let io = TokioIo::new(stream);
                let svc = service_fn(move |req| handle(req, target.clone(), key.clone()));
                if let Err(e) = http1::Builder::new()
                    .keep_alive(true)
                    .serve_connection(io, svc)
                    .await
                {
                    proxy_log!(" 连接错误 ({}): {e}", peer);
                }
            });
        }
    });

    Ok(AgnesProxy { port })
}

// ── 请求处理 ────────────────────────────────────────────────────────────

async fn handle(
    req: Request<Incoming>,
    target_url: String,
    api_key: String,
) -> Result<Response<ProxyBody>, hyper::Error> {
    // 处理 POST /v1/messages 和 /messages（CLI 的 ANTHROPIC_BASE_URL 可能带或不带 /v1）
    let path = req.uri().path();
    proxy_log!(" 收到请求: {} {}", req.method(), path);
    if req.method() != Method::POST || (path != "/v1/messages" && path != "/messages") {
        proxy_log!(" 拒绝: 不是 POST /v1/messages 或 /messages");
        let mut resp = Response::new(full_body(
            "only POST /v1/messages or /messages is supported",
        ));
        *resp.status_mut() = StatusCode::NOT_FOUND;
        return Ok(resp);
    }

    // 读请求体
    let body_bytes = match req.collect().await {
        Ok(collected) => collected.to_bytes().to_vec(),
        Err(e) => {
            proxy_log!(" 读请求体失败: {e}");
            let mut resp = Response::new(full_body(
                "bad request",
            ));
            *resp.status_mut() = StatusCode::BAD_REQUEST;
            return Ok(resp);
        }
    };

    // 解析 Anthropic 请求
    let anthropic_req: Value = match serde_json::from_slice(&body_bytes) {
        Ok(v) => v,
        Err(e) => {
            proxy_log!(" JSON 解析失败: {e}");
            let mut resp = Response::new(full_body(
                "invalid json",
            ));
            *resp.status_mut() = StatusCode::BAD_REQUEST;
            return Ok(resp);
        }
    };

    let is_stream = anthropic_req
        .get("stream")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    proxy_log!(
        "请求解析成功: model={}, stream={}, messages={}, tools={}",
        anthropic_req.get("model").and_then(|v| v.as_str()).unwrap_or("?"),
        is_stream,
        anthropic_req.get("messages").and_then(|v| v.as_array()).map_or(0, |a| a.len()),
        anthropic_req.get("tools").and_then(|v| v.as_array()).map_or(0, |a| a.len()),
    );

    // Anthropic → OpenAI 格式转换
    let openai_req = anthropic_to_openai(&anthropic_req);
    let openai_body = serde_json::to_vec(&openai_req).unwrap_or_default();

    proxy_log!(" 转发到上游: body_len={}", openai_body.len());

    let client = reqwest::Client::new();
    let url = format!("{}/chat/completions", target_url.trim_end_matches('/'));

    if is_stream {
        handle_streaming(client, url, api_key, openai_body).await
    } else {
        handle_non_streaming(client, url, api_key, openai_body).await
    }
}

// ── 非流式处理 ──────────────────────────────────────────────────────────

async fn handle_non_streaming(
    client: reqwest::Client,
    url: String,
    api_key: String,
    body: Vec<u8>,
) -> Result<Response<ProxyBody>, hyper::Error> {
    proxy_log!(" 非流式请求: url={}", url);
    let resp = match client
        .post(&url)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", api_key))
        .body(body)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            proxy_log!(" 上游请求失败: {e}");
            let mut resp = Response::new(full_body("upstream error"));
            *resp.status_mut() = StatusCode::BAD_GATEWAY;
            return Ok(resp);
        }
    };

    let status = resp.status();
    proxy_log!(" 上游响应: status={}", status);
    let resp_body = resp.bytes().await.unwrap_or_default();

    if !status.is_success() {
        let body_str = String::from_utf8_lossy(&resp_body);
        proxy_log!(" 上游错误: {} {}", status, body_str);
        // 透传错误
        let mut r = Response::new(full_body(resp_body.to_vec()));
        *r.status_mut() = StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        return Ok(r);
    }

    let openai_resp: Value = match serde_json::from_slice(&resp_body) {
        Ok(v) => v,
        Err(e) => {
            proxy_log!(" 上游响应 JSON 解析失败: {}", e);
            let mut r = Response::new(full_body(resp_body.to_vec()));
            r.headers_mut().insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
            return Ok(r);
        }
    };

    let anthropic_resp = openai_to_anthropic(&openai_resp);
    let body = serde_json::to_vec(&anthropic_resp).unwrap_or_default();
    proxy_log!(" 非流式转换完成: resp_len={}", body.len());

    let mut r = Response::new(full_body(body));
    r.headers_mut().insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    Ok(r)
}

// ── 流式（SSE）处理 ──────────────────────────────────────────────────────

async fn handle_streaming(
    client: reqwest::Client,
    url: String,
    api_key: String,
    body: Vec<u8>,
) -> Result<Response<ProxyBody>, hyper::Error> {
    proxy_log!(" 流式请求: url={}", url);
    let resp = match client
        .post(&url)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", api_key))
        .body(body)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            proxy_log!(" 上游流式请求失败: {e}");
            let mut r = Response::new(full_body("upstream error"));
            *r.status_mut() = StatusCode::BAD_GATEWAY;
            return Ok(r);
        }
    };

    let status = resp.status();
    proxy_log!(" 上游流式响应: status={}", status);
    if !status.is_success() {
        let err_body = resp.bytes().await.unwrap_or_default();
        let mut r = Response::new(full_body(err_body.to_vec()));
        *r.status_mut() = StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        return Ok(r);
    }

    // 用 mpsc channel 构建流式响应
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Bytes, hyper::Error>>(32);

    tokio::spawn(async move {
        let mut stream = resp.bytes_stream();
        let mut msg_id = String::new();
        let mut model = String::new();
        let mut text_started = false;
        let mut buffer = String::new();
        let mut done = false;

        while let Some(chunk) = futures_util::StreamExt::next(&mut stream).await {
            let chunk = match chunk {
                Ok(c) => c,
                Err(e) => {
                    proxy_log!(" 流读取错误: {e}");
                    break;
                }
            };

            buffer.push_str(&String::from_utf8_lossy(&chunk));

            // 逐行处理 SSE
            while let Some(line_end) = buffer.find('\n') {
                let line = buffer[..line_end].trim().to_string();
                buffer = buffer[line_end + 1..].to_string();

                if line.is_empty() {
                    continue;
                }
                if line == "data: [DONE]" {
                    done = true;
                    continue;
                }
                if !line.starts_with("data: ") {
                    continue;
                }

                let json_str = &line[6..]; // 跳过 "data: "
                let chunk: Value = match serde_json::from_str(json_str) {
                    Ok(v) => v,
                    Err(_) => continue,
                };

                // 提取元数据
                if msg_id.is_empty() {
                    msg_id = chunk["id"].as_str().unwrap_or("chatcmpl-").to_string();
                }
                if model.is_empty() {
                    model = chunk["model"].as_str().unwrap_or("").to_string();
                }

                // 提取 delta
                let choices = match chunk["choices"].as_array() {
                    Some(c) => c,
                    None => continue,
                };

                for choice in choices {
                    let delta = &choice["delta"];
                    let finish_reason = choice["finish_reason"].as_str();

                    // 检查 tool_calls
                    if let Some(tool_calls) = delta["tool_calls"].as_array() {
                        for tc in tool_calls {
                            let tc_id = tc["id"].as_str().unwrap_or("");
                            let tc_index = tc["index"].as_u64().unwrap_or(0);
                            let func = &tc["function"];
                            let func_name = func["name"].as_str().unwrap_or("");
                            let func_args = func["arguments"].as_str().unwrap_or("");

                            // tool_use 开始事件
                            let start = json!({
                                "type": "content_block_start",
                                "index": tc_index,
                                "content_block": {
                                    "type": "tool_use",
                                    "id": tc_id,
                                    "name": func_name,
                                    "input": {}
                                }
                            });
                            let _ = tx
                                .send(Ok(Bytes::from(format!(
                                    "event: content_block_start\ndata: {}\n\n",
                                    serde_json::to_string(&start).unwrap_or_default()
                                ))))
                                .await;

                            // tool_use delta
                            if !func_args.is_empty() {
                                let delta = json!({
                                    "type": "content_block_delta",
                                    "index": tc_index,
                                    "delta": {
                                        "type": "input_json_delta",
                                        "partial_json": func_args
                                    }
                                });
                                let _ = tx
                                    .send(Ok(Bytes::from(format!(
                                        "event: content_block_delta\ndata: {}\n\n",
                                        serde_json::to_string(&delta).unwrap_or_default()
                                    ))))
                                    .await;
                            }
                        }
                        continue;
                    }

                    // 文本 delta
                    if let Some(text) = delta["content"].as_str() {
                        if text.is_empty() && finish_reason.is_none() {
                            continue;
                        }

                        if !text_started && !text.is_empty() {
                            text_started = true;
                            // message_start
                            let start = json!({
                                "type": "message_start",
                                "message": {
                                    "type": "message",
                                    "model": model,
                                    "role": "assistant",
                                    "id": msg_id,
                                    "content": [],
                                    "usage": {
                                        "input_tokens": 0,
                                        "output_tokens": 0
                                    }
                                }
                            });
                            let _ = tx
                                .send(Ok(Bytes::from(format!(
                                    "event: message_start\ndata: {}\n\n",
                                    serde_json::to_string(&start).unwrap_or_default()
                                ))))
                                .await;
                            // content_block_start
                            let cbs = json!({
                                "type": "content_block_start",
                                "index": 0,
                                "content_block": {"type": "text", "text": ""}
                            });
                            let _ = tx
                                .send(Ok(Bytes::from(format!(
                                    "event: content_block_start\ndata: {}\n\n",
                                    serde_json::to_string(&cbs).unwrap_or_default()
                                ))))
                                .await;
                        }

                        if !text.is_empty() {
                            let delta = json!({
                                "type": "content_block_delta",
                                "index": 0,
                                "delta": {"type": "text_delta", "text": text}
                            });
                            let _ = tx
                                .send(Ok(Bytes::from(format!(
                                    "event: content_block_delta\ndata: {}\n\n",
                                    serde_json::to_string(&delta).unwrap_or_default()
                                ))))
                                .await;
                        }
                    }

                    // finish
                    if finish_reason.is_some() && text_started {
                        // content_block_stop
                        let cbs = json!({"type": "content_block_stop", "index": 0});
                        let _ = tx
                            .send(Ok(Bytes::from(format!(
                                "event: content_block_stop\ndata: {}\n\n",
                                serde_json::to_string(&cbs).unwrap_or_default()
                            ))))
                            .await;
                        // message_delta
                        let stop_reason = match finish_reason {
                            Some("stop") | Some("end_turn") => "end_turn",
                            Some("tool_calls") => "tool_use",
                            _ => "end_turn",
                        };
                        let md = json!({
                            "type": "message_delta",
                            "delta": {"stop_reason": stop_reason},
                            "usage": {"input_tokens": 0, "output_tokens": 0}
                        });
                        let _ = tx
                            .send(Ok(Bytes::from(format!(
                                "event: message_delta\ndata: {}\n\n",
                                serde_json::to_string(&md).unwrap_or_default()
                            ))))
                            .await;
                        // message_stop
                        let ms = json!({"type": "message_stop"});
                        let _ = tx
                            .send(Ok(Bytes::from(format!(
                                "event: message_stop\ndata: {}\n\n",
                                serde_json::to_string(&ms).unwrap_or_default()
                            ))))
                            .await;
                    }
                }
            }
        }

        proxy_log!(
            "流式处理完成: msg_id={}, model={}, done={}, text_started={}",
            msg_id, model, done, text_started,
        );
        if !done && !text_started {
            // 空响应，至少发个 message_start + stop
            let start = json!({
                "type": "message_start",
                "message": {
                    "type": "message",
                    "model": model,
                    "role": "assistant",
                    "id": msg_id,
                    "content": [],
                    "usage": {"input_tokens": 0, "output_tokens": 0}
                }
            });
            let _ = tx
                .send(Ok(Bytes::from(format!(
                    "event: message_start\ndata: {}\n\n",
                    serde_json::to_string(&start).unwrap_or_default()
                ))))
                .await;
            let ms = json!({"type": "message_stop"});
            let _ = tx
                .send(Ok(Bytes::from(format!(
                    "event: message_stop\ndata: {}\n\n",
                    serde_json::to_string(&ms).unwrap_or_default()
                ))))
                .await;
        }
    });

    // 把 mpsc receiver 转成 StreamBody（直接用第一个 channel 的 rx，不中继）
    let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
    let stream_body = http_body_util::StreamBody::new(
        stream.map(|r| match r {
            Ok(data) => Ok(hyper::body::Frame::data(data)),
            Err(e) => Err(e),
        })
    );
    let boxed: ProxyBody = stream_body.boxed();

    let mut r = Response::new(boxed);
    r.headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static("text/event-stream"));
    r.headers_mut()
        .insert("Cache-Control", HeaderValue::from_static("no-cache"));
    Ok(r)
}

// ── 格式转换：Anthropic 请求 → OpenAI 请求 ──────────────────────────────

fn anthropic_to_openai(req: &Value) -> Value {
    let mut openai = json!({});

    // model — 不动
    if let Some(model) = req.get("model").and_then(|v| v.as_str()) {
        openai["model"] = json!(model);
    }

    // max_tokens — 不动
    if let Some(mt) = req.get("max_tokens") {
        openai["max_tokens"] = mt.clone();
    }

    // stream — 不动
    if let Some(stream) = req.get("stream") {
        openai["stream"] = stream.clone();
    }

    // temperature — 不动
    if let Some(temp) = req.get("temperature") {
        openai["temperature"] = temp.clone();
    }

    // top_p — 不动
    if let Some(top_p) = req.get("top_p") {
        openai["top_p"] = top_p.clone();
    }

    // stop_sequences → stop
    if let Some(stop) = req.get("stop_sequences") {
        openai["stop"] = stop.clone();
    }

    // ── messages 转换 ──
    let mut openai_messages: Vec<Value> = Vec::new();

    // Anthropic system → OpenAI system message（放在 messages[0]）
    if let Some(system) = req.get("system") {
        let system_content = match system {
            Value::String(s) => s.clone(),
            Value::Array(blocks) => {
                // system 是 content blocks 数组，提取 text 类型的
                let mut parts: Vec<String> = Vec::new();
                for block in blocks {
                    if block["type"] == "text" {
                        if let Some(t) = block["text"].as_str() {
                            parts.push(t.to_string());
                        }
                    }
                }
                parts.join("\n\n")
            }
            _ => system.to_string(),
        };
        if !system_content.is_empty() {
            openai_messages.push(json!({
                "role": "system",
                "content": system_content
            }));
        }
    }

    // Anthropic messages → OpenAI messages
    if let Some(messages) = req.get("messages").and_then(|v| v.as_array()) {
        for msg in messages {
            let role = msg["role"].as_str().unwrap_or("user");
            let content = &msg["content"];

            let openai_msg = match content {
                // content 是字符串
                Value::String(text) => {
                    json!({"role": role, "content": text})
                }
                // content 是数组（content blocks）
                Value::Array(blocks) => {
                    let mut text_parts: Vec<String> = Vec::new();
                    let mut tool_calls: Vec<Value> = Vec::new();
                    let mut tool_call_id_counter: u64 = 0;

                    for block in blocks {
                        match block["type"].as_str() {
                            Some("text") => {
                                if let Some(t) = block["text"].as_str() {
                                    text_parts.push(t.to_string());
                                }
                            }
                            Some("tool_use") => {
                                let tc_id = block["id"]
                                    .as_str()
                                    .map(|s| s.to_string())
                                    .unwrap_or_else(|| format!("call_{}", tool_call_id_counter));
                                tool_call_id_counter += 1;
                                let tc_name = block["name"].as_str().unwrap_or("");
                                let tc_input = block["input"].clone();

                                tool_calls.push(json!({
                                    "id": tc_id,
                                    "type": "function",
                                    "function": {
                                        "name": tc_name,
                                        "arguments": serde_json::to_string(&tc_input).unwrap_or_default()
                                    }
                                }));
                            }
                            Some("tool_result") => {
                                // tool_result → tool role message
                                let tool_id = block["tool_use_id"].as_str().unwrap_or("");
                                let result_content = match &block["content"] {
                                    Value::String(s) => s.clone(),
                                    Value::Array(blocks) => {
                                        blocks
                                            .iter()
                                            .filter_map(|b| b["text"].as_str().map(|s| s.to_string()))
                                            .collect::<Vec<_>>()
                                            .join("")
                                    }
                                    _ => block["content"].to_string(),
                                };
                                // 先插入之前的 assistant tool_calls
                                if !tool_calls.is_empty() {
                                    openai_messages.push(json!({
                                        "role": "assistant",
                                        "content": null,
                                        "tool_calls": tool_calls.clone()
                                    }));
                                    tool_calls.clear();
                                }
                                openai_messages.push(json!({
                                    "role": "tool",
                                    "tool_call_id": tool_id,
                                    "content": result_content
                                }));
                            }
                            _ => {}
                        }
                    }

                    if !text_parts.is_empty() {
                        let text_content = text_parts.join("");
                        json!({"role": role, "content": text_content})
                    } else if !tool_calls.is_empty() {
                        json!({
                            "role": "assistant",
                            "content": null,
                            "tool_calls": tool_calls
                        })
                    } else {
                        continue; // 空消息，跳过
                    }
                }
                _ => continue,
            };

            openai_messages.push(openai_msg);
        }
    }

    openai["messages"] = json!(openai_messages);

    // ── tools 转换 ──
    if let Some(tools) = req.get("tools").and_then(|v| v.as_array()) {
        let openai_tools: Vec<Value> = tools
            .iter()
            .map(|tool| {
                json!({
                    "type": "function",
                    "function": {
                        "name": tool["name"],
                        "description": tool.get("description").unwrap_or(&json!("")),
                        "parameters": tool.get("input_schema").cloned().unwrap_or(json!({"type": "object", "properties": {}}))
                    }
                })
            })
            .collect();
        openai["tools"] = json!(openai_tools);
    }

    // tool_choice — 只在有 tools 时才转换，否则 OpenAI API 会拒绝
    // Anthropic: "auto" | "any" | {"type":"auto"} | {"type":"any"} | {"type":"tool","name":"..."}
    // OpenAI:   "auto" | "none" | "required" | {"type":"function","function":{"name":"..."}}
    if openai.get("tools").is_some() {
        if let Some(tc) = req.get("tool_choice") {
            openai["tool_choice"] = match tc {
            // 字符串格式（直通）
            Value::String(s) if s == "any" => json!("required"),
            Value::String(s) if s == "auto" || s == "none" || s == "required" => json!(s),
            // 对象格式 — 最常见，CLI 发送的是 {"type":"auto"} 或 {"type":"any"} 等
            Value::Object(obj) => {
                match obj.get("type").and_then(|v| v.as_str()) {
                    Some("auto") => json!("auto"),
                    Some("any") => json!("required"),
                    Some("tool") => {
                        json!({"type": "function", "function": {"name": obj.get("name").and_then(|v| v.as_str()).unwrap_or("")}})
                    }
                    _ => json!("auto"), // 未知类型，默认 auto
                }
            }
            _ => json!("auto"),
        };
    }
    } // end if tools.is_some()

    // 移除 Anthropic 特有字段（不在标准 OpenAI schema 里）
    // thinking, metadata 等都不传

    openai
}

// ── 格式转换：OpenAI 响应 → Anthropic 响应 ──────────────────────────────

fn openai_to_anthropic(resp: &Value) -> Value {
    let id = resp["id"].as_str().unwrap_or("msg_");
    let model = resp["model"].as_str().unwrap_or("");
    let usage = resp.get("usage");

    let mut content: Vec<Value> = Vec::new();
    let mut stop_reason = "end_turn";

    if let Some(choices) = resp["choices"].as_array() {
        for choice in choices {
            let msg = &choice["message"];

            // 文本内容
            if let Some(text) = msg["content"].as_str() {
                if !text.is_empty() {
                    content.push(json!({
                        "type": "text",
                        "text": text
                    }));
                }
            }

            // tool_calls → tool_use content blocks
            if let Some(tool_calls) = msg["tool_calls"].as_array() {
                for tc in tool_calls {
                    let func = &tc["function"];
                    let input: Value = serde_json::from_str(
                        func["arguments"].as_str().unwrap_or("{}")
                    )
                    .unwrap_or(json!({}));

                    content.push(json!({
                        "type": "tool_use",
                        "id": tc["id"],
                        "name": func["name"],
                        "input": input
                    }));
                }
                stop_reason = "tool_use";
            }

            if let Some(fr) = choice["finish_reason"].as_str() {
                if fr == "stop" || fr == "end_turn" {
                    stop_reason = "end_turn";
                } else if fr == "tool_calls" {
                    stop_reason = "tool_use";
                } else if fr == "length" {
                    stop_reason = "max_tokens";
                }
            }
        }
    }

    // 获取 token 用量
    let input_tokens = usage
        .and_then(|u| u["prompt_tokens"].as_u64())
        .unwrap_or(0);
    let output_tokens = usage
        .and_then(|u| u["completion_tokens"].as_u64())
        .unwrap_or(0);

    json!({
        "id": id,
        "type": "message",
        "role": "assistant",
        "model": model,
        "content": content,
        "stop_reason": stop_reason,
        "stop_sequence": null,
        "usage": {
            "input_tokens": input_tokens,
            "output_tokens": output_tokens
        }
    })
}

// ── 测试 ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tools_conversion() {
        let anthropic = json!({
            "model": "test-model",
            "max_tokens": 100,
            "messages": [{"role": "user", "content": "hi"}],
            "tools": [
                {
                    "name": "Bash",
                    "description": "Run a shell command",
                    "input_schema": {
                        "type": "object",
                        "properties": {"command": {"type": "string"}},
                        "required": ["command"]
                    }
                }
            ]
        });

        let openai = anthropic_to_openai(&anthropic);

        // 验证 tools 转换
        let tools = openai["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0]["type"], "function");
        assert_eq!(tools[0]["function"]["name"], "Bash");
        assert_eq!(tools[0]["function"]["parameters"]["type"], "object");

        // 验证 messages 不变
        let msgs = openai["messages"].as_array().unwrap();
        assert_eq!(msgs[0]["role"], "user");
        assert_eq!(msgs[0]["content"], "hi");
    }

    #[test]
    fn test_system_conversion() {
        let anthropic = json!({
            "model": "test",
            "max_tokens": 10,
            "messages": [{"role": "user", "content": "hi"}],
            "system": [{"type": "text", "text": "You are helpful."}]
        });

        let openai = anthropic_to_openai(&anthropic);
        let msgs = openai["messages"].as_array().unwrap();
        assert_eq!(msgs[0]["role"], "system");
        assert_eq!(msgs[0]["content"], "You are helpful.");
        assert_eq!(msgs[1]["role"], "user");
    }

    #[test]
    fn test_response_conversion_text() {
        let openai_resp = json!({
            "id": "chatcmpl-123",
            "model": "test",
            "choices": [{
                "index": 0,
                "finish_reason": "stop",
                "message": {"role": "assistant", "content": "Hello!"}
            }],
            "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}
        });

        let anthropic = openai_to_anthropic(&openai_resp);
        assert_eq!(anthropic["type"], "message");
        assert_eq!(anthropic["role"], "assistant");
        assert_eq!(anthropic["content"][0]["text"], "Hello!");
        assert_eq!(anthropic["usage"]["input_tokens"], 10);
        assert_eq!(anthropic["usage"]["output_tokens"], 5);
    }

    #[test]
    fn test_response_conversion_tool_use() {
        let openai_resp = json!({
            "id": "chatcmpl-456",
            "model": "test",
            "choices": [{
                "index": 0,
                "finish_reason": "tool_calls",
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_1",
                        "type": "function",
                        "function": {"name": "Bash", "arguments": "{\"command\":\"ls\"}"}
                    }]
                }
            }],
            "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}
        });

        let anthropic = openai_to_anthropic(&openai_resp);
        assert_eq!(anthropic["stop_reason"], "tool_use");
        let content = anthropic["content"].as_array().unwrap();
        assert_eq!(content[0]["type"], "tool_use");
        assert_eq!(content[0]["name"], "Bash");
        assert_eq!(content[0]["input"]["command"], "ls");
    }

    /// 端到端测试：启动代理 → 发 Anthropic 请求 → 验证响应
    /// 需要网络连接和有效的 Agnes AI API key
    #[tokio::test]
    async fn test_e2e_proxy_with_real_api() {
        let api_key = "sk-o5BUpyY2j6D57I2F1XqSmOt1Qnr1ffiz4Y1TKpX4MKBmJNbt";
        let target_url = "https://apihub.agnes-ai.com/v1";

        // 启动代理
        let proxy = start_proxy(target_url.to_string(), api_key.to_string())
            .await
            .expect("代理启动失败");
        let port = proxy.port;
        eprintln!("=== 测试代理启动在 127.0.0.1:{port} ===");

        // 等待代理就绪
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        // 构建 Anthropic 格式请求（带 tools，模拟 CLI）
        let anthropic_req = json!({
            "model": "agnes-2.0-flash",
            "max_tokens": 100,
            "stream": false,
            "messages": [
                {"role": "user", "content": "你好，用一句话回复"}
            ],
            "system": [{"type": "text", "text": "你是助手"}],
            "tools": [
                {
                    "name": "Bash",
                    "description": "Run shell commands",
                    "input_schema": {
                        "type": "object",
                        "properties": {"command": {"type": "string"}},
                        "required": ["command"]
                    }
                }
            ],
            "tool_choice": {"type": "auto"}
        });

        let client = reqwest::Client::new();
        let resp = client
            .post(format!("http://127.0.0.1:{port}/v1/messages"))
            .header("Content-Type", "application/json")
            .header("x-api-key", api_key)
            .json(&anthropic_req)
            .send()
            .await
            .expect("代理请求失败");

        eprintln!("=== 响应状态: {} ===", resp.status());
        assert!(resp.status().is_success(), "代理返回非 200: {}", resp.status());

        let body: Value = resp.json().await.expect("响应 JSON 解析失败");
        eprintln!("=== 响应体: {} ===", serde_json::to_string_pretty(&body).unwrap_or_default());

        // 验证 Anthropic 格式
        assert_eq!(body["type"], "message", "应该是 message 类型");
        assert_eq!(body["role"], "assistant", "role 应该是 assistant");
        assert!(body["content"].is_array(), "content 应该是数组");
        assert!(body["stop_reason"].is_string(), "应该有 stop_reason");
        assert!(body["usage"]["input_tokens"].as_u64().unwrap_or(0) > 0, "应该有 input_tokens");
        assert!(body["usage"]["output_tokens"].as_u64().unwrap_or(0) > 0, "应该有 output_tokens");

        eprintln!("=== 端到端测试通过 ===");
    }

    /// 端到端测试：流式请求（无 tools）
    #[tokio::test]
    async fn test_e2e_proxy_streaming_no_tools() {
        let api_key = "sk-o5BUpyY2j6D57I2F1XqSmOt1Qnr1ffiz4Y1TKpX4MKBmJNbt";
        let target_url = "https://apihub.agnes-ai.com/v1";

        let proxy = start_proxy(target_url.to_string(), api_key.to_string())
            .await
            .expect("代理启动失败");
        let port = proxy.port;
        eprintln!("=== 流式无tools测试: 代理端口 {port} ===");

        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        // 无 tools、无 tool_choice — 模拟纯对话
        let anthropic_req = json!({
            "model": "agnes-2.0-flash",
            "max_tokens": 100,
            "stream": true,
            "messages": [
                {"role": "user", "content": "说一句话"}
            ],
            "system": [{"type": "text", "text": "你是助手，用中文回复"}]
        });

        let client = reqwest::Client::new();
        let resp = client
            .post(format!("http://127.0.0.1:{port}/v1/messages"))
            .header("Content-Type", "application/json")
            .header("x-api-key", api_key)
            .json(&anthropic_req)
            .send()
            .await
            .expect("代理流式请求失败");

        assert!(resp.status().is_success(), "流式响应状态: {}", resp.status());
        assert_eq!(
            resp.headers().get("content-type").map(|v| v.to_str().unwrap_or("")),
            Some("text/event-stream"),
            "应该是 SSE"
        );

        let body = resp.text().await.expect("读流式响应失败");
        eprintln!("=== 流式响应（前 500 字符）===\n{}", &body[..body.len().min(500)]);

        // 验证包含正确的 SSE 事件
        assert!(body.contains("event: message_start"), "应该有 message_start");
        assert!(body.contains("event: content_block_start"), "应该有 content_block_start");
        assert!(body.contains("event: content_block_delta"), "应该有 content_block_delta");
        assert!(body.contains("event: message_stop"), "应该有 message_stop");

        eprintln!("=== 流式无tools测试通过 ===");
    }

    /// 端到端测试：流式请求（带 tools 和 tool_choice）
    #[tokio::test]
    async fn test_e2e_proxy_streaming_with_tools() {
        let api_key = "sk-o5BUpyY2j6D57I2F1XqSmOt1Qnr1ffiz4Y1TKpX4MKBmJNbt";
        let target_url = "https://apihub.agnes-ai.com/v1";

        let proxy = start_proxy(target_url.to_string(), api_key.to_string())
            .await
            .expect("代理启动失败");
        let port = proxy.port;
        eprintln!("=== 流式带tools测试: 代理端口 {port} ===");

        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        let anthropic_req = json!({
            "model": "agnes-2.0-flash",
            "max_tokens": 100,
            "stream": true,
            "messages": [
                {"role": "user", "content": "say hello in one word"}
            ],
            "system": [{"type": "text", "text": "You are a helpful assistant."}],
            "tools": [
                {
                    "name": "Bash",
                    "description": "Run shell commands",
                    "input_schema": {
                        "type": "object",
                        "properties": {"command": {"type": "string"}},
                        "required": ["command"]
                    }
                }
            ],
            "tool_choice": {"type": "auto"}
        });

        let client = reqwest::Client::new();
        let resp = client
            .post(format!("http://127.0.0.1:{port}/v1/messages"))
            .header("Content-Type", "application/json")
            .header("x-api-key", api_key)
            .json(&anthropic_req)
            .send()
            .await
            .expect("代理流式请求失败");

        assert!(resp.status().is_success(), "流式带tools响应状态: {}", resp.status());
        let body = resp.text().await.expect("读流式响应失败");
        eprintln!("=== 流式带tools响应（前 500 字符）===\n{}", &body[..body.len().min(500)]);

        assert!(body.contains("event: message_start"), "应该有 message_start");
        assert!(body.contains("event: content_block_delta"), "应该有 delta");
        assert!(body.contains("event: message_stop"), "应该有 message_stop");

        eprintln!("=== 流式带tools测试通过 ===");
    }
}
