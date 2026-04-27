use std::collections::BTreeMap;
use std::fs;
use std::io::ErrorKind;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::json;
use tokio::runtime::Builder;

use crate::modules::runtime::config::{
    ConfigSource, McpRemoteServerConfig, McpSdkServerConfig, McpServerConfig, McpStdioServerConfig,
    McpWebSocketServerConfig, ScopedMcpServerConfig,
};
use crate::modules::runtime::mcp::mcp_tool_name;
use crate::modules::runtime::mcp_client::McpClientBootstrap;

use super::{
    clear_mcp_workbench_activity_for_tests, mcp_workbench_activity_entry,
    mcp_workbench_activity_snapshot, mcp_workbench_discovery_dto,
    mcp_workbench_unsupported_server_dtos, record_mcp_workbench_activity,
    sanitize_mcp_workbench_value, spawn_mcp_stdio_process, summarize_mcp_counts, JsonRpcId,
    JsonRpcRequest, JsonRpcResponse, McpInitializeClientInfo, McpInitializeParams,
    McpInitializeResult, McpInitializeServerInfo, McpListToolsResult, McpReadResourceParams,
    McpReadResourceResult, McpServerManager, McpServerManagerError, McpStdioProcess, McpTool,
    McpToolCallParams, McpWorkbenchActivityStatus,
};

static TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

fn temp_dir() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time should be after epoch")
        .as_nanos();
    let sequence = TEMP_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "runtime-mcp-stdio-{}-{nanos}-{sequence}",
        std::process::id()
    ))
}

fn write_echo_script() -> PathBuf {
    let root = temp_dir();
    fs::create_dir_all(&root).expect("temp dir");
    let script_path = root.join("echo-mcp.sh");
    fs::write(
        &script_path,
        "#!/bin/sh\nprintf 'READY:%s\\n' \"$MCP_TEST_TOKEN\"\nIFS= read -r line\nprintf 'ECHO:%s\\n' \"$line\"\n",
    )
    .expect("write script");
    let mut permissions = fs::metadata(&script_path).expect("metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&script_path, permissions).expect("chmod");
    script_path
}

fn write_jsonrpc_script() -> PathBuf {
    let root = temp_dir();
    fs::create_dir_all(&root).expect("temp dir");
    let script_path = root.join("jsonrpc-mcp.py");
    let script = [
        "#!/usr/bin/env python3",
        "import json, sys",
        "header = b''",
        r"while not header.endswith(b'\r\n\r\n'):",
        "    chunk = sys.stdin.buffer.read(1)",
        "    if not chunk:",
        "        raise SystemExit(1)",
        "    header += chunk",
        "length = 0",
        r"for line in header.decode().split('\r\n'):",
        r"    if line.lower().startswith('content-length:'):",
        r"        length = int(line.split(':', 1)[1].strip())",
        "payload = sys.stdin.buffer.read(length)",
        "request = json.loads(payload.decode())",
        r"assert request['jsonrpc'] == '2.0'",
        r"assert request['method'] == 'initialize'",
        r"response = json.dumps({",
        r"    'jsonrpc': '2.0',",
        r"    'id': request['id'],",
        r"    'result': {",
        r"        'protocolVersion': request['params']['protocolVersion'],",
        r"        'capabilities': {'tools': {}},",
        r"        'serverInfo': {'name': 'fake-mcp', 'version': '0.1.0'}",
        r"    }",
        r"}).encode()",
        r"sys.stdout.buffer.write(f'Content-Length: {len(response)}\r\n\r\n'.encode() + response)",
        "sys.stdout.buffer.flush()",
        "",
    ]
    .join("\n");
    fs::write(&script_path, script).expect("write script");
    let mut permissions = fs::metadata(&script_path).expect("metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&script_path, permissions).expect("chmod");
    script_path
}

#[allow(clippy::too_many_lines)]
fn write_mcp_server_script() -> PathBuf {
    let root = temp_dir();
    fs::create_dir_all(&root).expect("temp dir");
    let script_path = root.join("fake-mcp-server.py");
    let script = [
        "#!/usr/bin/env python3",
        "import json, sys",
        "",
        "def read_message():",
        "    header = b''",
        r"    while not header.endswith(b'\r\n\r\n'):",
        "        chunk = sys.stdin.buffer.read(1)",
        "        if not chunk:",
        "            return None",
        "        header += chunk",
        "    length = 0",
        r"    for line in header.decode().split('\r\n'):",
        r"        if line.lower().startswith('content-length:'):",
        r"            length = int(line.split(':', 1)[1].strip())",
        "    payload = sys.stdin.buffer.read(length)",
        "    return json.loads(payload.decode())",
        "",
        "def send_message(message):",
        "    payload = json.dumps(message).encode()",
        r"    sys.stdout.buffer.write(f'Content-Length: {len(payload)}\r\n\r\n'.encode() + payload)",
        "    sys.stdout.buffer.flush()",
        "",
        "while True:",
        "    request = read_message()",
        "    if request is None:",
        "        break",
        "    method = request['method']",
        "    if method == 'initialize':",
        "        send_message({",
        "            'jsonrpc': '2.0',",
        "            'id': request['id'],",
        "            'result': {",
        "                'protocolVersion': request['params']['protocolVersion'],",
        "                'capabilities': {'tools': {}, 'resources': {}, 'prompts': {}},",
        "                'serverInfo': {'name': 'fake-mcp', 'version': '0.2.0'}",
        "            }",
        "        })",
        "    elif method == 'tools/list':",
        "        send_message({",
        "            'jsonrpc': '2.0',",
        "            'id': request['id'],",
        "            'result': {",
        "                'tools': [",
        "                    {",
        "                        'name': 'echo',",
        "                        'description': 'Echoes text',",
        "                        'inputSchema': {",
        "                            'type': 'object',",
        "                            'properties': {'text': {'type': 'string'}},",
        "                            'required': ['text']",
        "                        }",
        "                    }",
        "                ]",
        "            }",
        "        })",
        "    elif method == 'tools/call':",
        "        args = request['params'].get('arguments') or {}",
        "        if request['params']['name'] == 'fail':",
        "            send_message({",
        "                'jsonrpc': '2.0',",
        "                'id': request['id'],",
        "                'error': {'code': -32001, 'message': 'tool failed'},",
        "            })",
        "        else:",
        "            text = args.get('text', '')",
        "            send_message({",
        "                'jsonrpc': '2.0',",
        "                'id': request['id'],",
        "                'result': {",
        "                    'content': [{'type': 'text', 'text': f'echo:{text}'}],",
        "                    'structuredContent': {'echoed': text},",
        "                    'isError': False",
        "                }",
        "            })",
        "    elif method == 'resources/list':",
        "        send_message({",
        "            'jsonrpc': '2.0',",
        "            'id': request['id'],",
        "            'result': {",
        "                'resources': [",
        "                    {",
        "                        'uri': 'file://guide.txt',",
        "                        'name': 'guide',",
        "                        'description': 'Guide text',",
        "                        'mimeType': 'text/plain'",
        "                    }",
        "                ]",
        "            }",
        "        })",
        "    elif method == 'resources/read':",
        "        uri = request['params']['uri']",
        "        send_message({",
        "            'jsonrpc': '2.0',",
        "            'id': request['id'],",
        "            'result': {",
        "                'contents': [",
        "                    {",
        "                        'uri': uri,",
        "                        'mimeType': 'text/plain',",
        "                        'text': f'contents for {uri}'",
        "                    }",
        "                ]",
        "            }",
        "        })",
        "    elif method == 'prompts/list':",
        "        send_message({",
        "            'jsonrpc': '2.0',",
        "            'id': request['id'],",
        "            'result': {",
        "                'prompts': [",
        "                    {",
        "                        'name': 'summarize',",
        "                        'description': 'Summarize input',",
        "                        'arguments': [{'name': 'topic', 'required': True}]",
        "                    }",
        "                ]",
        "            }",
        "        })",
        "    elif method == 'prompts/get':",
        "        topic = (request['params'].get('arguments') or {}).get('topic', 'general')",
        "        send_message({",
        "            'jsonrpc': '2.0',",
        "            'id': request['id'],",
        "            'result': {",
        "                'description': 'Summarize input',",
        "                'messages': [{'role': 'user', 'content': {'type': 'text', 'text': f'summarize {topic}'}}]",
        "            }",
        "        })",
        "    else:",
        "        send_message({",
        "            'jsonrpc': '2.0',",
        "            'id': request['id'],",
        "            'error': {'code': -32601, 'message': f'unknown method: {method}'},",
        "        })",
        "",
    ]
    .join("\n");
    fs::write(&script_path, script).expect("write script");
    let mut permissions = fs::metadata(&script_path).expect("metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&script_path, permissions).expect("chmod");
    script_path
}

#[allow(clippy::too_many_lines)]
fn write_manager_mcp_server_script() -> PathBuf {
    let root = temp_dir();
    fs::create_dir_all(&root).expect("temp dir");
    let script_path = root.join("manager-mcp-server.py");
    let script = [
        "#!/usr/bin/env python3",
        "import json, os, sys",
        "",
        "LABEL = os.environ.get('MCP_SERVER_LABEL', 'server')",
        "LOG_PATH = os.environ.get('MCP_LOG_PATH')",
        "initialize_count = 0",
        "",
        "def log(method):",
        "    if LOG_PATH:",
        "        with open(LOG_PATH, 'a', encoding='utf-8') as handle:",
        "            handle.write(f'{method}\\n')",
        "",
        "def read_message():",
        "    header = b''",
        r"    while not header.endswith(b'\r\n\r\n'):",
        "        chunk = sys.stdin.buffer.read(1)",
        "        if not chunk:",
        "            return None",
        "        header += chunk",
        "    length = 0",
        r"    for line in header.decode().split('\r\n'):",
        r"        if line.lower().startswith('content-length:'):",
        r"            length = int(line.split(':', 1)[1].strip())",
        "    payload = sys.stdin.buffer.read(length)",
        "    return json.loads(payload.decode())",
        "",
        "def send_message(message):",
        "    payload = json.dumps(message).encode()",
        r"    sys.stdout.buffer.write(f'Content-Length: {len(payload)}\r\n\r\n'.encode() + payload)",
        "    sys.stdout.buffer.flush()",
        "",
        "while True:",
        "    request = read_message()",
        "    if request is None:",
        "        break",
        "    method = request['method']",
        "    log(method)",
        "    if method == 'initialize':",
        "        initialize_count += 1",
        "        send_message({",
        "            'jsonrpc': '2.0',",
        "            'id': request['id'],",
        "            'result': {",
        "                'protocolVersion': request['params']['protocolVersion'],",
        "                'capabilities': {'tools': {}, 'resources': {}, 'prompts': {}},",
        "                'serverInfo': {'name': LABEL, 'version': '1.0.0'}",
        "            }",
        "        })",
        "    elif method == 'tools/list':",
        "        send_message({",
        "            'jsonrpc': '2.0',",
        "            'id': request['id'],",
        "            'result': {",
        "                'tools': [",
        "                    {",
        "                        'name': 'echo',",
        "                        'description': f'Echo tool for {LABEL}',",
        "                        'inputSchema': {",
        "                            'type': 'object',",
        "                            'properties': {'text': {'type': 'string'}},",
        "                            'required': ['text']",
        "                        }",
        "                    }",
        "                ]",
        "            }",
        "        })",
        "    elif method == 'tools/call':",
        "        args = request['params'].get('arguments') or {}",
        "        text = args.get('text', '')",
        "        send_message({",
        "            'jsonrpc': '2.0',",
        "            'id': request['id'],",
        "            'result': {",
        "                'content': [{'type': 'text', 'text': f'{LABEL}:{text}'}],",
        "                'structuredContent': {",
        "                    'server': LABEL,",
        "                    'echoed': text,",
        "                    'initializeCount': initialize_count",
        "                },",
        "                'isError': False",
        "            }",
        "        })",
        "    elif method == 'resources/list':",
        "        send_message({",
        "            'jsonrpc': '2.0',",
        "            'id': request['id'],",
        "            'result': {'resources': [{'uri': f'file://{LABEL}/guide.txt', 'name': 'guide'}]}",
        "        })",
        "    elif method == 'resources/read':",
        "        uri = request['params']['uri']",
        "        send_message({",
        "            'jsonrpc': '2.0',",
        "            'id': request['id'],",
        "            'result': {'contents': [{'uri': uri, 'mimeType': 'text/plain', 'text': f'{LABEL}:{uri}'}]}",
        "        })",
        "    elif method == 'prompts/list':",
        "        send_message({",
        "            'jsonrpc': '2.0',",
        "            'id': request['id'],",
        "            'result': {'prompts': [{'name': 'summarize', 'description': f'Summarize for {LABEL}'}]}",
        "        })",
        "    elif method == 'prompts/get':",
        "        send_message({",
        "            'jsonrpc': '2.0',",
        "            'id': request['id'],",
        "            'result': {'description': f'Summarize for {LABEL}', 'messages': [{'role': 'user', 'content': {'type': 'text', 'text': LABEL}}]}",
        "        })",
        "    else:",
        "        send_message({",
        "            'jsonrpc': '2.0',",
        "            'id': request['id'],",
        "            'error': {'code': -32601, 'message': f'unknown method: {method}'},",
        "        })",
        "",
    ]
    .join("\n");
    fs::write(&script_path, script).expect("write script");
    let mut permissions = fs::metadata(&script_path).expect("metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&script_path, permissions).expect("chmod");
    script_path
}

fn sample_bootstrap(script_path: &Path) -> McpClientBootstrap {
    let config = ScopedMcpServerConfig {
        scope: ConfigSource::Local,
        config: McpServerConfig::Stdio(McpStdioServerConfig {
            command: "/bin/sh".to_string(),
            args: vec![script_path.to_string_lossy().into_owned()],
            env: BTreeMap::from([("MCP_TEST_TOKEN".to_string(), "secret-value".to_string())]),
        }),
    };
    McpClientBootstrap::from_scoped_config("stdio server", &config)
}

fn script_transport(script_path: &Path) -> crate::modules::runtime::mcp_client::McpStdioTransport {
    crate::modules::runtime::mcp_client::McpStdioTransport {
        command: python_command(),
        args: vec![script_path.to_string_lossy().into_owned()],
        env: BTreeMap::new(),
    }
}

fn python_command() -> String {
    for key in ["MCP_TEST_PYTHON", "PYTHON3", "PYTHON"] {
        if let Ok(value) = std::env::var(key) {
            if !value.trim().is_empty() {
                return value;
            }
        }
    }

    for candidate in ["python3", "python"] {
        if Command::new(candidate).arg("--version").output().is_ok() {
            return candidate.to_string();
        }
    }

    panic!("expected a Python interpreter for MCP stdio tests")
}

fn cleanup_script(script_path: &Path) {
    if let Err(error) = fs::remove_file(script_path) {
        assert_eq!(error.kind(), std::io::ErrorKind::NotFound, "cleanup script");
    }
    if let Err(error) = fs::remove_dir_all(script_path.parent().expect("script parent")) {
        assert_eq!(error.kind(), std::io::ErrorKind::NotFound, "cleanup dir");
    }
}

fn manager_server_config(
    script_path: &Path,
    label: &str,
    log_path: &Path,
) -> ScopedMcpServerConfig {
    ScopedMcpServerConfig {
        scope: ConfigSource::Local,
        config: McpServerConfig::Stdio(McpStdioServerConfig {
            command: python_command(),
            args: vec![script_path.to_string_lossy().into_owned()],
            env: BTreeMap::from([
                ("MCP_SERVER_LABEL".to_string(), label.to_string()),
                (
                    "MCP_LOG_PATH".to_string(),
                    log_path.to_string_lossy().into_owned(),
                ),
            ]),
        }),
    }
}

#[test]
fn spawns_stdio_process_and_round_trips_io() {
    let runtime = Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    runtime.block_on(async {
        let script_path = write_echo_script();
        let bootstrap = sample_bootstrap(&script_path);
        let mut process = spawn_mcp_stdio_process(&bootstrap).expect("spawn stdio process");

        let ready = process.read_line().await.expect("read ready");
        assert_eq!(ready, "READY:secret-value\n");

        process
            .write_line("ping from client")
            .await
            .expect("write line");

        let echoed = process.read_line().await.expect("read echo");
        assert_eq!(echoed, "ECHO:ping from client\n");

        let status = process.wait().await.expect("wait for exit");
        assert!(status.success());

        cleanup_script(&script_path);
    });
}

#[test]
fn rejects_non_stdio_bootstrap() {
    let config = ScopedMcpServerConfig {
        scope: ConfigSource::Local,
        config: McpServerConfig::Sdk(crate::modules::runtime::config::McpSdkServerConfig {
            name: "sdk-server".to_string(),
        }),
    };
    let bootstrap = McpClientBootstrap::from_scoped_config("sdk server", &config);
    let error = spawn_mcp_stdio_process(&bootstrap).expect_err("non-stdio should fail");
    assert_eq!(error.kind(), ErrorKind::InvalidInput);
}

#[test]
fn round_trips_initialize_request_and_response_over_stdio_frames() {
    let runtime = Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    runtime.block_on(async {
        let script_path = write_jsonrpc_script();
        let transport = script_transport(&script_path);
        let mut process = McpStdioProcess::spawn(&transport).expect("spawn transport directly");

        let response = process
            .initialize(
                JsonRpcId::Number(1),
                McpInitializeParams {
                    protocol_version: "2025-03-26".to_string(),
                    capabilities: json!({"roots": {}}),
                    client_info: McpInitializeClientInfo {
                        name: "runtime-tests".to_string(),
                        version: "0.1.0".to_string(),
                    },
                },
            )
            .await
            .expect("initialize roundtrip");

        assert_eq!(response.id, JsonRpcId::Number(1));
        assert_eq!(response.error, None);
        assert_eq!(
            response.result,
            Some(McpInitializeResult {
                protocol_version: "2025-03-26".to_string(),
                capabilities: json!({"tools": {}}),
                server_info: McpInitializeServerInfo {
                    name: "fake-mcp".to_string(),
                    version: "0.1.0".to_string(),
                },
            })
        );

        let status = process.wait().await.expect("wait for exit");
        assert!(status.success());

        cleanup_script(&script_path);
    });
}

#[test]
fn write_jsonrpc_request_emits_content_length_frame() {
    let runtime = Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    runtime.block_on(async {
        let script_path = write_jsonrpc_script();
        let transport = script_transport(&script_path);
        let mut process = McpStdioProcess::spawn(&transport).expect("spawn transport directly");
        let request = JsonRpcRequest::new(
            JsonRpcId::Number(7),
            "initialize",
            Some(json!({
                "protocolVersion": "2025-03-26",
                "capabilities": {},
                "clientInfo": {"name": "runtime-tests", "version": "0.1.0"}
            })),
        );

        process.send_request(&request).await.expect("send request");
        let response: JsonRpcResponse<serde_json::Value> =
            process.read_response().await.expect("read response");

        assert_eq!(response.id, JsonRpcId::Number(7));
        assert_eq!(response.jsonrpc, "2.0");

        let status = process.wait().await.expect("wait for exit");
        assert!(status.success());

        cleanup_script(&script_path);
    });
}

#[test]
fn direct_spawn_uses_transport_env() {
    let runtime = Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    runtime.block_on(async {
        let script_path = write_echo_script();
        let transport = crate::modules::runtime::mcp_client::McpStdioTransport {
            command: "/bin/sh".to_string(),
            args: vec![script_path.to_string_lossy().into_owned()],
            env: BTreeMap::from([("MCP_TEST_TOKEN".to_string(), "direct-secret".to_string())]),
        };
        let mut process = McpStdioProcess::spawn(&transport).expect("spawn transport directly");
        let ready = process.read_available().await.expect("read ready");
        assert_eq!(String::from_utf8_lossy(&ready), "READY:direct-secret\n");
        process.terminate().await.expect("terminate child");
        let _ = process.wait().await.expect("wait after kill");

        cleanup_script(&script_path);
    });
}

#[test]
#[ignore = "requires external Python MCP server script; run with --ignored in integration environments"]
fn lists_tools_calls_tool_and_reads_resources_over_jsonrpc() {
    let runtime = Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    runtime.block_on(async {
        let script_path = write_mcp_server_script();
        let transport = script_transport(&script_path);
        let mut process = McpStdioProcess::spawn(&transport).expect("spawn fake mcp server");

        let tools = process
            .list_tools(JsonRpcId::Number(2), None)
            .await
            .expect("list tools");
        assert_eq!(tools.error, None);
        assert_eq!(tools.id, JsonRpcId::Number(2));
        assert_eq!(
            tools.result,
            Some(McpListToolsResult {
                tools: vec![McpTool {
                    name: "echo".to_string(),
                    description: Some("Echoes text".to_string()),
                    input_schema: Some(json!({
                        "type": "object",
                        "properties": {"text": {"type": "string"}},
                        "required": ["text"]
                    })),
                    annotations: None,
                    meta: None,
                }],
                next_cursor: None,
            })
        );

        let call = process
            .call_tool(
                JsonRpcId::String("call-1".to_string()),
                McpToolCallParams {
                    name: "echo".to_string(),
                    arguments: Some(json!({"text": "hello"})),
                    meta: None,
                },
            )
            .await
            .expect("call tool");
        assert_eq!(call.error, None);
        let call_result = call.result.expect("tool result");
        assert_eq!(call_result.is_error, Some(false));
        assert_eq!(
            call_result.structured_content,
            Some(json!({"echoed": "hello"}))
        );
        assert_eq!(call_result.content.len(), 1);
        assert_eq!(call_result.content[0].kind, "text");
        assert_eq!(
            call_result.content[0].data.get("text"),
            Some(&json!("echo:hello"))
        );

        let resources = process
            .list_resources(JsonRpcId::Number(3), None)
            .await
            .expect("list resources");
        let resources_result = resources.result.expect("resources result");
        assert_eq!(resources_result.resources.len(), 1);
        assert_eq!(resources_result.resources[0].uri, "file://guide.txt");
        assert_eq!(
            resources_result.resources[0].mime_type.as_deref(),
            Some("text/plain")
        );

        let read = process
            .read_resource(
                JsonRpcId::Number(4),
                McpReadResourceParams {
                    uri: "file://guide.txt".to_string(),
                },
            )
            .await
            .expect("read resource");
        assert_eq!(
            read.result,
            Some(McpReadResourceResult {
                contents: vec![super::McpResourceContents {
                    uri: "file://guide.txt".to_string(),
                    mime_type: Some("text/plain".to_string()),
                    text: Some("contents for file://guide.txt".to_string()),
                    blob: None,
                    meta: None,
                }],
            })
        );

        process.terminate().await.expect("terminate child");
        let _ = process.wait().await.expect("wait after kill");
        cleanup_script(&script_path);
    });
}

#[test]
fn surfaces_jsonrpc_errors_from_tool_calls() {
    let runtime = Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    runtime.block_on(async {
        let script_path = write_mcp_server_script();
        let transport = script_transport(&script_path);
        let mut process = McpStdioProcess::spawn(&transport).expect("spawn fake mcp server");

        let response = process
            .call_tool(
                JsonRpcId::Number(9),
                McpToolCallParams {
                    name: "fail".to_string(),
                    arguments: None,
                    meta: None,
                },
            )
            .await
            .expect("call tool with error response");

        assert_eq!(response.id, JsonRpcId::Number(9));
        assert!(response.result.is_none());
        assert_eq!(response.error.as_ref().map(|e| e.code), Some(-32001));
        assert_eq!(
            response.error.as_ref().map(|e| e.message.as_str()),
            Some("tool failed")
        );

        process.terminate().await.expect("terminate child");
        let _ = process.wait().await.expect("wait after kill");
        cleanup_script(&script_path);
    });
}

#[test]
fn manager_discovers_tools_from_stdio_config() {
    let runtime = Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    runtime.block_on(async {
        let script_path = write_manager_mcp_server_script();
        let root = script_path.parent().expect("script parent");
        let log_path = root.join("alpha.log");
        let servers = BTreeMap::from([(
            "alpha".to_string(),
            manager_server_config(&script_path, "alpha", &log_path),
        )]);
        let mut manager = McpServerManager::from_servers(&servers);

        let tools = manager.discover_tools().await.expect("discover tools");

        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].server_name, "alpha");
        assert_eq!(tools[0].raw_name, "echo");
        assert_eq!(tools[0].qualified_name, mcp_tool_name("alpha", "echo"));
        assert_eq!(tools[0].tool.name, "echo");
        assert!(manager.unsupported_servers().is_empty());

        manager.shutdown().await.expect("shutdown");
        cleanup_script(&script_path);
    });
}

#[test]
fn manager_routes_tool_calls_to_correct_server() {
    let runtime = Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    runtime.block_on(async {
        let script_path = write_manager_mcp_server_script();
        let root = script_path.parent().expect("script parent");
        let alpha_log = root.join("alpha.log");
        let beta_log = root.join("beta.log");
        let servers = BTreeMap::from([
            (
                "alpha".to_string(),
                manager_server_config(&script_path, "alpha", &alpha_log),
            ),
            (
                "beta".to_string(),
                manager_server_config(&script_path, "beta", &beta_log),
            ),
        ]);
        let mut manager = McpServerManager::from_servers(&servers);

        let tools = manager.discover_tools().await.expect("discover tools");
        assert_eq!(tools.len(), 2);

        let alpha = manager
            .call_tool(
                &mcp_tool_name("alpha", "echo"),
                Some(json!({"text": "hello"})),
            )
            .await
            .expect("call alpha tool");
        let beta = manager
            .call_tool(
                &mcp_tool_name("beta", "echo"),
                Some(json!({"text": "world"})),
            )
            .await
            .expect("call beta tool");

        assert_eq!(
            alpha
                .result
                .as_ref()
                .and_then(|result| result.structured_content.as_ref())
                .and_then(|value| value.get("server")),
            Some(&json!("alpha"))
        );
        assert_eq!(
            beta.result
                .as_ref()
                .and_then(|result| result.structured_content.as_ref())
                .and_then(|value| value.get("server")),
            Some(&json!("beta"))
        );

        manager.shutdown().await.expect("shutdown");
        cleanup_script(&script_path);
    });
}

#[test]
fn manager_records_unsupported_non_stdio_servers_without_panicking() {
    let servers = BTreeMap::from([
        (
            "http".to_string(),
            ScopedMcpServerConfig {
                scope: ConfigSource::Local,
                config: McpServerConfig::Http(McpRemoteServerConfig {
                    url: "https://example.test/mcp".to_string(),
                    headers: BTreeMap::new(),
                    headers_helper: None,
                    oauth: None,
                }),
            },
        ),
        (
            "sdk".to_string(),
            ScopedMcpServerConfig {
                scope: ConfigSource::Local,
                config: McpServerConfig::Sdk(McpSdkServerConfig {
                    name: "sdk-server".to_string(),
                }),
            },
        ),
        (
            "ws".to_string(),
            ScopedMcpServerConfig {
                scope: ConfigSource::Local,
                config: McpServerConfig::Ws(McpWebSocketServerConfig {
                    url: "wss://example.test/mcp".to_string(),
                    headers: BTreeMap::new(),
                    headers_helper: None,
                }),
            },
        ),
    ]);

    let manager = McpServerManager::from_servers(&servers);
    let unsupported = manager.unsupported_servers();

    assert_eq!(unsupported.len(), 3);
    assert_eq!(unsupported[0].server_name, "http");
    assert_eq!(unsupported[1].server_name, "sdk");
    assert_eq!(unsupported[2].server_name, "ws");
}

#[test]
fn manager_shutdown_terminates_spawned_children_and_is_idempotent() {
    let runtime = Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    runtime.block_on(async {
        let script_path = write_manager_mcp_server_script();
        let root = script_path.parent().expect("script parent");
        let log_path = root.join("alpha.log");
        let servers = BTreeMap::from([(
            "alpha".to_string(),
            manager_server_config(&script_path, "alpha", &log_path),
        )]);
        let mut manager = McpServerManager::from_servers(&servers);

        manager.discover_tools().await.expect("discover tools");
        manager.shutdown().await.expect("first shutdown");
        manager.shutdown().await.expect("second shutdown");

        cleanup_script(&script_path);
    });
}

#[test]
fn manager_reuses_spawned_server_between_discovery_and_call() {
    let runtime = Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    runtime.block_on(async {
        let script_path = write_manager_mcp_server_script();
        let root = script_path.parent().expect("script parent");
        let log_path = root.join("alpha.log");
        let servers = BTreeMap::from([(
            "alpha".to_string(),
            manager_server_config(&script_path, "alpha", &log_path),
        )]);
        let mut manager = McpServerManager::from_servers(&servers);

        manager.discover_tools().await.expect("discover tools");
        let response = manager
            .call_tool(
                &mcp_tool_name("alpha", "echo"),
                Some(json!({"text": "reuse"})),
            )
            .await
            .expect("call tool");

        assert_eq!(
            response
                .result
                .as_ref()
                .and_then(|result| result.structured_content.as_ref())
                .and_then(|value| value.get("initializeCount")),
            Some(&json!(1))
        );

        let log = fs::read_to_string(&log_path).expect("read log");
        assert_eq!(log.lines().filter(|line| *line == "initialize").count(), 1);
        assert_eq!(
            log.lines().collect::<Vec<_>>(),
            vec!["initialize", "tools/list", "tools/call"]
        );

        manager.shutdown().await.expect("shutdown");
        cleanup_script(&script_path);
    });
}

#[test]
fn manager_reports_unknown_qualified_tool_name() {
    let runtime = Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    runtime.block_on(async {
        let script_path = write_manager_mcp_server_script();
        let root = script_path.parent().expect("script parent");
        let log_path = root.join("alpha.log");
        let servers = BTreeMap::from([(
            "alpha".to_string(),
            manager_server_config(&script_path, "alpha", &log_path),
        )]);
        let mut manager = McpServerManager::from_servers(&servers);

        let error = manager
            .call_tool(
                &mcp_tool_name("alpha", "missing"),
                Some(json!({"text": "nope"})),
            )
            .await
            .expect_err("unknown qualified tool should fail");

        match error {
            McpServerManagerError::UnknownTool { qualified_name } => {
                assert_eq!(qualified_name, mcp_tool_name("alpha", "missing"));
            }
            other => panic!("expected unknown tool error, got {other:?}"),
        }

        cleanup_script(&script_path);
    });
}

#[test]
fn manager_call_tool_discovering_builds_route_index_on_demand() {
    let runtime = Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    runtime.block_on(async {
        let script_path = write_manager_mcp_server_script();
        let root = script_path.parent().expect("script parent");
        let log_path = root.join("alpha.log");
        let tool_name = mcp_tool_name("alpha", "echo");
        let servers = BTreeMap::from([(
            "alpha".to_string(),
            manager_server_config(&script_path, "alpha", &log_path),
        )]);
        let mut manager = McpServerManager::from_servers(&servers);

        assert!(!manager.has_tool_route(&tool_name));
        let response = manager
            .call_tool_discovering(&tool_name, Some(json!({"text": "on-demand"})))
            .await
            .expect("call tool with automatic discovery");

        assert!(manager.has_tool_route(&tool_name));
        assert_eq!(
            response
                .result
                .as_ref()
                .and_then(|result| result.structured_content.as_ref())
                .and_then(|value| value.get("echoed")),
            Some(&json!("on-demand"))
        );

        let log = fs::read_to_string(&log_path).expect("read log");
        assert_eq!(
            log.lines().collect::<Vec<_>>(),
            vec!["initialize", "tools/list", "tools/call"]
        );

        manager.shutdown().await.expect("shutdown");
        cleanup_script(&script_path);
    });
}

#[test]
fn manager_lists_resources_and_reads_resource() {
    let runtime = Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    runtime.block_on(async {
        let script_path = write_manager_mcp_server_script();
        let root = script_path.parent().expect("script parent");
        let log_path = root.join("alpha.log");
        let servers = BTreeMap::from([(
            "alpha".to_string(),
            manager_server_config(&script_path, "alpha", &log_path),
        )]);
        let mut manager = McpServerManager::from_servers(&servers);

        let resources = manager.list_resources().await.expect("list resources");
        assert_eq!(resources.len(), 1);
        assert_eq!(resources[0].server_name, "alpha");
        assert_eq!(resources[0].resource.uri, "file://alpha/guide.txt");

        let read = manager
            .read_resource("alpha", "file://alpha/guide.txt")
            .await
            .expect("read resource");
        let result = read.result.expect("read result");
        assert_eq!(result.contents.len(), 1);
        assert_eq!(
            result.contents[0].text.as_deref(),
            Some("alpha:file://alpha/guide.txt")
        );

        manager.shutdown().await.expect("shutdown");
        cleanup_script(&script_path);
    });
}

#[test]
fn manager_lists_and_gets_prompts() {
    let runtime = Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    runtime.block_on(async {
        let script_path = write_manager_mcp_server_script();
        let root = script_path.parent().expect("script parent");
        let log_path = root.join("alpha.log");
        let servers = BTreeMap::from([(
            "alpha".to_string(),
            manager_server_config(&script_path, "alpha", &log_path),
        )]);
        let mut manager = McpServerManager::from_servers(&servers);

        let prompts = manager.list_prompts().await.expect("list prompts");
        assert_eq!(prompts.len(), 1);
        assert_eq!(prompts[0].server_name, "alpha");
        assert_eq!(prompts[0].prompt.name, "summarize");

        let prompt = manager
            .get_prompt("alpha", "summarize", None)
            .await
            .expect("get prompt");
        let result = prompt.result.expect("prompt result");
        assert_eq!(result.messages.len(), 1);
        assert_eq!(result.messages[0].role, "user");

        manager.shutdown().await.expect("shutdown");
        cleanup_script(&script_path);
    });
}

#[test]
fn mcp_workbench_discovers_capabilities() {
    let runtime = Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    runtime.block_on(async {
        let script_path = write_manager_mcp_server_script();
        let root = script_path.parent().expect("script parent");
        let log_path = root.join("alpha.log");
        let servers = BTreeMap::from([
            (
                "alpha".to_string(),
                manager_server_config(&script_path, "alpha", &log_path),
            ),
            (
                "http".to_string(),
                ScopedMcpServerConfig {
                    scope: ConfigSource::Local,
                    config: McpServerConfig::Http(McpRemoteServerConfig {
                        url: "https://example.test/mcp".to_string(),
                        headers: BTreeMap::new(),
                        headers_helper: None,
                        oauth: None,
                    }),
                },
            ),
        ]);
        let mut manager = McpServerManager::from_servers(&servers);
        let unsupported = mcp_workbench_unsupported_server_dtos(manager.unsupported_servers());

        let tools = manager.discover_tools().await.expect("discover tools");
        let resources = manager.list_resources().await.expect("list resources");
        let prompts = manager.list_prompts().await.expect("list prompts");
        let dto = mcp_workbench_discovery_dto(tools, resources, prompts, unsupported);

        assert_eq!(dto.tools.len(), 1);
        assert_eq!(dto.tools[0].qualified_name, mcp_tool_name("alpha", "echo"));
        assert_eq!(dto.resources[0].uri, "file://alpha/guide.txt");
        assert_eq!(dto.prompts[0].name, "summarize");
        assert_eq!(dto.unsupported_servers.len(), 1);
        assert!(!dto.unsupported_servers[0].active);
        assert_eq!(dto.unsupported_servers[0].transport, "http");

        manager.shutdown().await.expect("shutdown");
        cleanup_script(&script_path);
    });
}

#[test]
fn mcp_workbench_records_activity() {
    clear_mcp_workbench_activity_for_tests();

    let redacted = sanitize_mcp_workbench_value(&json!({
        "token": "super-secret",
        "nested": {"apiKey": "also-secret", "value": "visible"},
    }));
    assert_eq!(redacted["token"], json!("[redacted]"));
    assert_eq!(redacted["nested"]["apiKey"], json!("[redacted]"));
    assert_eq!(redacted["nested"]["value"], json!("visible"));

    record_mcp_workbench_activity(mcp_workbench_activity_entry(
        "alpha",
        "call_tool",
        Some("echo".to_string()),
        McpWorkbenchActivityStatus::Ok,
        12,
        Some(json!({"token": "secret", "text": "hello"})),
        Some(summarize_mcp_counts(1, 1, 1)),
        None,
    ));

    let entries = mcp_workbench_activity_snapshot();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].server_id, "alpha");
    assert_eq!(entries[0].operation, "call_tool");
    assert_eq!(entries[0].status, McpWorkbenchActivityStatus::Ok);
    assert_eq!(
        entries[0]
            .params
            .as_ref()
            .and_then(|params| params.get("token")),
        Some(&json!("[redacted]"))
    );
    assert_eq!(
        entries[0]
            .result_summary
            .as_ref()
            .and_then(|summary| summary.get("tools")),
        Some(&json!(1))
    );
}
