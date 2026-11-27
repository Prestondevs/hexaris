use std::{
    io::{BufRead, Read, Write},
    ops::Bound::{Excluded, Unbounded},
};

use hexaris_engine::{
    Analysis, CONTEXT_LIMIT, ContextAddress, ImageIdentity, Project, ReferenceIndex, analyze,
    parse_address,
};
use serde_json::{Value, json};

use super::function_context;

const MAX_MESSAGE: u64 = 64 * 1024;
const PROTOCOL_VERSION: &str = "2025-11-25";

pub(super) fn serve(
    project: Project,
    mut input: impl BufRead,
    output: &mut impl Write,
) -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze(project.image());
    let references = ReferenceIndex::build(&analysis);
    let server = Server {
        project,
        analysis,
        references,
    };
    let mut phase = Phase::New;
    let mut line = Vec::new();
    loop {
        line.clear();
        if (&mut input)
            .take(MAX_MESSAGE + 1)
            .read_until(b'\n', &mut line)?
            == 0
        {
            return Ok(());
        }
        if line.len() as u64 > MAX_MESSAGE {
            send(
                output,
                &rpc_error(Value::Null, -32600, "Message exceeds 64 KiB"),
            )?;
            return Ok(());
        }
        let response = match serde_json::from_slice::<Value>(&line) {
            Ok(message) => server.handle(&mut phase, &message),
            Err(_) => Some(rpc_error(Value::Null, -32700, "Invalid JSON")),
        };
        if let Some(response) = response {
            send(output, &response)?;
        }
    }
}

fn send(output: &mut impl Write, message: &Value) -> Result<(), Box<dyn std::error::Error>> {
    serde_json::to_writer(&mut *output, message)?;
    writeln!(output)?;
    output.flush()?;
    Ok(())
}

fn rpc_error(id: Value, code: i32, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

#[derive(PartialEq)]
enum Phase {
    New,
    AwaitingNotification,
    Ready,
}

struct Server {
    project: Project,
    analysis: Analysis,
    references: ReferenceIndex,
}

impl Server {
    fn handle(&self, phase: &mut Phase, message: &Value) -> Option<Value> {
        let id = message.get("id");
        let method = message.get("method").and_then(Value::as_str);
        if message.get("jsonrpc").and_then(Value::as_str) != Some("2.0")
            || method.is_none()
            || id.is_some_and(|id| !(id.is_string() || id.is_i64() || id.is_u64()))
        {
            return Some(rpc_error(Value::Null, -32600, "Invalid request"));
        }
        let method = method.expect("validated method");
        let Some(id) = id.cloned() else {
            if method == "notifications/initialized" && *phase == Phase::AwaitingNotification {
                *phase = Phase::Ready;
            }
            return None;
        };
        let params = message.get("params").unwrap_or(&Value::Null);
        let result = match method {
            "initialize" if *phase == Phase::New => {
                let Some(requested) = params.get("protocolVersion").and_then(Value::as_str) else {
                    return Some(rpc_error(id, -32602, "Expected protocolVersion"));
                };
                let version = match requested {
                    "2025-06-18" | PROTOCOL_VERSION => requested,
                    _ => PROTOCOL_VERSION,
                };
                *phase = Phase::AwaitingNotification;
                json!({
                    "protocolVersion": version,
                    "capabilities": {"tools": {"listChanged": false}},
                    "serverInfo": {"name": "hexaris", "version": env!("CARGO_PKG_VERSION")},
                    "instructions": "This session is scoped to one user-selected image. Cite address evidence and carry forward Hexaris analysis notices."
                })
            }
            "ping" => json!({}),
            _ if *phase != Phase::Ready => {
                return Some(rpc_error(id, -32600, "MCP session is not initialized"));
            }
            "tools/list" => json!({"tools": tools()}),
            "tools/call" => {
                let Some(name) = params.get("name").and_then(Value::as_str) else {
                    return Some(rpc_error(id, -32602, "Expected a tool name"));
                };
                let arguments = params.get("arguments").unwrap_or(&Value::Null);
                if !arguments.is_null() && !arguments.is_object() {
                    return Some(rpc_error(id, -32602, "Expected a tool arguments object"));
                }
                match self.call_tool(name, arguments) {
                    Ok(data) => json!({
                        "structuredContent": data,
                        "content": [{"type": "text", "text": data.to_string()}]
                    }),
                    Err(error) => json!({
                        "isError": true,
                        "content": [{"type": "text", "text": error}]
                    }),
                }
            }
            _ => return Some(rpc_error(id, -32601, "Method not found")),
        };
        Some(json!({"jsonrpc": "2.0", "id": id, "result": result}))
    }

    fn call_tool(&self, name: &str, arguments: &Value) -> Result<Value, String> {
        let image = || ImageIdentity::from_project(&self.project);
        match name {
            "inspect_image" => {
                reject_extra(arguments, &[])?;
                let info = &self.project.image().info;
                Ok(json!({
                    "image": image(), "entry": info.entry.map(ContextAddress),
                    "discovered_functions": self.analysis.functions.len(), "notices": self.analysis.notices
                }))
            }
            "list_functions" => {
                reject_extra(arguments, &["after", "limit"])?;
                let limit = index(arguments, "limit", 50)?;
                if !(1..=CONTEXT_LIMIT).contains(&limit) {
                    return Err("limit must be 1–256".into());
                }
                let start = arguments
                    .get("after")
                    .map(|value| address(value, "after").map(Excluded))
                    .transpose()?
                    .unwrap_or(Unbounded);
                let mut functions = self.analysis.functions.range((start, Unbounded));
                let page: Vec<_> = functions
                    .by_ref()
                    .take(limit)
                    .map(|(&entry, function)| {
                        (
                            ContextAddress(entry),
                            json!({
                                "address": ContextAddress(entry),
                                "name": self.project.function_name(entry).unwrap_or(&function.name),
                                "instruction_count": function.instructions.len()
                            }),
                        )
                    })
                    .collect();
                let next = functions
                    .next()
                    .and_then(|_| page.last())
                    .map(|entry| entry.0);
                Ok(json!({
                    "image": image(), "functions": page.into_iter().map(|entry| entry.1).collect::<Vec<_>>(),
                    "next_after": next, "notices": self.analysis.notices
                }))
            }
            "function_context" => {
                reject_extra(arguments, &["address", "offset", "limit"])?;
                let entry = address(&arguments["address"], "address")?;
                let offset = index(arguments, "offset", 0)?;
                let limit = index(arguments, "limit", 128)?;
                let context = function_context(
                    &self.project,
                    &self.analysis,
                    &self.references,
                    entry,
                    offset,
                    limit,
                )
                .map_err(|error| error.to_string())?;
                serde_json::to_value(context).map_err(|error| error.to_string())
            }
            "read_references" => {
                reject_extra(arguments, &["address"])?;
                let entry = address(&arguments["address"], "address")?;
                let incoming = self.references.incoming(entry);
                let outgoing = self.references.outgoing(entry);
                let references = |items: &[hexaris_engine::Reference]| {
                    items.iter().take(CONTEXT_LIMIT).map(|item| json!({
                        "source": ContextAddress(item.source), "target": ContextAddress(item.target), "kind": item.kind
                    })).collect::<Vec<_>>()
                };
                Ok(json!({
                    "image": image(), "address": ContextAddress(entry),
                    "incoming": references(incoming), "outgoing": references(outgoing),
                    "truncated": incoming.len().max(outgoing.len()) > CONTEXT_LIMIT,
                    "notices": self.analysis.notices
                }))
            }
            _ => Err("Unknown read-only Hexaris tool".into()),
        }
    }
}

fn address(value: &Value, field: &str) -> Result<u64, String> {
    value
        .as_str()
        .and_then(|text| parse_address(text).ok())
        .ok_or_else(|| format!("{field} must be an address string such as 0x140001000"))
}

fn index(arguments: &Value, field: &str, default: usize) -> Result<usize, String> {
    arguments.get(field).map_or(Ok(default), |value| {
        value
            .as_u64()
            .and_then(|number| usize::try_from(number).ok())
            .ok_or_else(|| format!("{field} must be a nonnegative integer"))
    })
}

fn reject_extra(arguments: &Value, fields: &[&str]) -> Result<(), String> {
    if let Some(key) = arguments.as_object().and_then(|fields_given| {
        fields_given
            .keys()
            .find(|key| !fields.contains(&key.as_str()))
    }) {
        return Err(format!("Unknown argument {key}"));
    }
    Ok(())
}

fn tools() -> Vec<Value> {
    let address =
        json!({"type": "string", "description": "Virtual address as a hexadecimal string"});
    let limit = json!({"type": "integer", "minimum": 1, "maximum": CONTEXT_LIMIT});
    let tool = |name: &str, description: &str, properties: Value, required: &[&str]| {
        json!({
            "name": name, "description": description, "annotations": {"readOnlyHint": true},
            "inputSchema": {"type": "object", "properties": properties, "required": required, "additionalProperties": false}
        })
    };
    vec![
        tool(
            "inspect_image",
            "Read the scoped image identity, format, entry, and analysis coverage.",
            json!({}),
            &[],
        ),
        tool(
            "list_functions",
            "Page through discovered functions by virtual address.",
            json!({"after": address, "limit": limit}),
            &[],
        ),
        tool(
            "function_context",
            "Read one function's decoded instructions, blocks, direct calls and references. Includes supported IR and Pseudo-C.",
            json!({"address": address, "offset": {"type": "integer", "minimum": 0}, "limit": limit}),
            &["address"],
        ),
        tool(
            "read_references",
            "Read indexed direct references to and from one address.",
            json!({"address": address}),
            &["address"],
        ),
    ]
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use hexaris_engine::{Architecture, Image, LoadMode};

    use super::*;

    #[test]
    fn stdio_session_supports_discovery_and_read_only_context() {
        let image = Image::from_bytes(
            vec![0x90, 0xC3],
            LoadMode::Raw {
                architecture: Architecture::X86_64,
                base: 0x1000,
            },
        )
        .unwrap();
        let project = Project::new("fixture.bin".into(), image);
        let messages = [
            json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": PROTOCOL_VERSION, "capabilities": {}, "clientInfo": {"name": "fixture", "version": "1"}}}),
            json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
            json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
            json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name":"function_context", "arguments":{"address":"0x1000","limit":1}}}),
            json!({"jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {"name":"inspect_image", "arguments":{"path":"elsewhere.exe"}}}),
            json!({"jsonrpc": "2.0", "id": 5, "method": "tools/call", "params": {"name":"patch"}}),
        ];
        let input: String = messages
            .iter()
            .map(|message| format!("{message}\n"))
            .collect();
        let mut output = Vec::new();
        serve(project, Cursor::new(input), &mut output).unwrap();
        let responses: Vec<Value> = std::str::from_utf8(&output)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(responses.len(), 5);
        assert_eq!(responses[0]["result"]["protocolVersion"], PROTOCOL_VERSION);
        assert_eq!(responses[1]["result"]["tools"].as_array().unwrap().len(), 4);
        assert_eq!(
            responses[2]["result"]["structuredContent"]["next_instruction_offset"],
            1
        );
        assert_eq!(responses[3]["result"]["isError"], true);
        assert_eq!(responses[4]["result"]["isError"], true);
    }
}
