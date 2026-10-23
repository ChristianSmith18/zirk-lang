use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Duration;

use serde_json::{Value as Json, json};

struct DapClient {
    child: Child,
    input: Option<ChildStdin>,
    messages: Receiver<std::io::Result<Json>>,
    pending: Vec<Json>,
    sequence: u64,
}

impl DapClient {
    fn spawn() -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_zirk-dap"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("launch DAP adapter process");
        let input = child.stdin.take().expect("adapter stdin");
        let output = child.stdout.take().expect("adapter stdout");
        let (sender, messages) = mpsc::channel();
        thread::spawn(move || {
            let mut reader = BufReader::new(output);
            loop {
                let mut content_length = None;
                loop {
                    let mut line = String::new();
                    match reader.read_line(&mut line) {
                        Ok(0) => return,
                        Ok(_) => {}
                        Err(error) => {
                            let _ = sender.send(Err(error));
                            return;
                        }
                    }
                    if line == "\r\n" || line == "\n" {
                        break;
                    }
                    if let Some((name, value)) = line.split_once(':')
                        && name.eq_ignore_ascii_case("content-length")
                    {
                        content_length = value.trim().parse::<usize>().ok();
                    }
                }
                let Some(length) = content_length else {
                    let _ = sender.send(Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "DAP packet has no Content-Length header",
                    )));
                    return;
                };
                let mut body = vec![0; length];
                if let Err(error) = reader.read_exact(&mut body) {
                    let _ = sender.send(Err(error));
                    return;
                }
                let message = serde_json::from_slice(&body).map_err(std::io::Error::other);
                if sender.send(message).is_err() {
                    return;
                }
            }
        });
        Self {
            child,
            input: Some(input),
            messages,
            pending: Vec::new(),
            sequence: 1,
        }
    }

    fn request(&mut self, command: &str, arguments: Json) {
        let message = json!({
            "seq": self.sequence,
            "type": "request",
            "command": command,
            "arguments": arguments
        });
        self.sequence += 1;
        let body = serde_json::to_vec(&message).expect("serialize DAP request");
        let input = self.input.as_mut().expect("adapter stdin is open");
        write!(input, "Content-Length: {}\r\n\r\n", body.len()).unwrap();
        input.write_all(&body).unwrap();
        input.flush().unwrap();
    }

    fn take_matching(&mut self, predicate: impl Fn(&Json) -> bool) -> Json {
        if let Some(index) = self.pending.iter().position(&predicate) {
            return self.pending.remove(index);
        }
        loop {
            let message = self
                .messages
                .recv_timeout(Duration::from_secs(10))
                .expect("timed out waiting for DAP message")
                .expect("read DAP message");
            if predicate(&message) {
                return message;
            }
            self.pending.push(message);
        }
    }

    fn response(&mut self, command: &str) -> Json {
        let message = self.take_matching(|message| {
            message["type"] == "response" && message["command"] == command
        });
        assert_eq!(
            message["success"], true,
            "DAP {command} request failed: {message}"
        );
        message
    }

    fn event(&mut self, event: &str) -> Json {
        self.take_matching(|message| message["type"] == "event" && message["event"] == event)
    }

    fn close(mut self) {
        drop(self.input.take());
        let status = self.child.wait().expect("wait for DAP adapter");
        assert!(status.success(), "DAP adapter exited with {status}");
    }
}

impl Drop for DapClient {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn fixture(source: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    let root = std::env::temp_dir().join(format!(
        "zirk-dap-protocol-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let file = root.join("main.zrk");
    std::fs::write(&file, source).unwrap();
    (root, file.canonicalize().unwrap())
}

fn initialize(client: &mut DapClient) {
    client.request("initialize", json!({"adapterID": "zirk"}));
    let response = client.response("initialize");
    assert_eq!(response["body"]["supportsConfigurationDoneRequest"], true);
    client.event("initialized");
}

fn launch(
    client: &mut DapClient,
    root: &std::path::Path,
    file: &std::path::Path,
    stop_on_entry: bool,
) {
    client.request(
        "launch",
        json!({
            "program": file,
            "cwd": root,
            "stopOnEntry": stop_on_entry
        }),
    );
    client.response("launch");
    client.event("process");
}

#[test]
fn adapter_process_runs_breakpoint_stack_and_variable_lifecycle() {
    let (root, file) =
        fixture("fn main(): Void {\n    inmut value: Int32 = 41;\n    inmut copy = value;\n}\n");
    let mut client = DapClient::spawn();
    initialize(&mut client);
    launch(&mut client, &root, &file, false);

    client.request(
        "setBreakpoints",
        json!({
            "source": {"path": file},
            "breakpoints": [{"line": 3}]
        }),
    );
    let breakpoints = client.response("setBreakpoints");
    assert_eq!(breakpoints["body"]["breakpoints"][0]["verified"], true);

    client.request("configurationDone", json!({}));
    client.response("configurationDone");
    assert_eq!(client.event("stopped")["body"]["reason"], "breakpoint");

    client.request("stackTrace", json!({"threadId": 1}));
    let frames = client.response("stackTrace");
    let frame_id = frames["body"]["stackFrames"][0]["id"].as_i64().unwrap();
    client.request("scopes", json!({"frameId": frame_id}));
    let scopes = client.response("scopes");
    let locals_id = scopes["body"]["scopes"][0]["variablesReference"]
        .as_i64()
        .unwrap();
    client.request("variables", json!({"variablesReference": locals_id}));
    let variables = client.response("variables")["body"]["variables"]
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(
        variables
            .iter()
            .find(|variable| variable["name"] == "value")
            .unwrap()["value"],
        "41"
    );
    assert_eq!(
        variables
            .iter()
            .find(|variable| variable["name"] == "copy")
            .unwrap()["value"],
        "<unavailable>"
    );

    client.request("continue", json!({"threadId": 1}));
    client.response("continue");
    client.event("continued");
    client.request("scopes", json!({"frameId": frame_id}));
    assert!(
        client.response("scopes")["body"]["scopes"]
            .as_array()
            .unwrap()
            .is_empty(),
        "frame handles from a previous pause must be invalid after resume"
    );
    client.request("variables", json!({"variablesReference": locals_id}));
    assert!(
        client.response("variables")["body"]["variables"]
            .as_array()
            .unwrap()
            .is_empty(),
        "handles from a previous pause must be invalid after resume"
    );
    client.event("terminated");
    client.request("variables", json!({"variablesReference": locals_id}));
    assert!(
        client.response("variables")["body"]["variables"]
            .as_array()
            .unwrap()
            .is_empty(),
        "terminated sessions must not retain runtime variables"
    );
    client.close();
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn adapter_process_pauses_a_running_program_and_terminates_it() {
    let (root, file) =
        fixture("fn main(): Void {\n    while true {\n        inmut value: Int32 = 1;\n    }\n}\n");
    let mut client = DapClient::spawn();
    initialize(&mut client);
    launch(&mut client, &root, &file, false);
    client.request("configurationDone", json!({}));
    client.response("configurationDone");

    client.request("pause", json!({"threadId": 1}));
    client.response("pause");
    assert_eq!(client.event("stopped")["body"]["reason"], "pause");

    client.request("terminate", json!({"terminateDebuggee": true}));
    client.response("terminate");
    client.event("terminated");
    client.close();
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn adapter_process_stops_on_entry_and_steps() {
    let (root, file) = fixture("fn main(): Void {\n    inmut value: Int32 = 7;\n}\n");
    let mut client = DapClient::spawn();
    initialize(&mut client);
    launch(&mut client, &root, &file, true);
    client.request("configurationDone", json!({}));
    client.response("configurationDone");
    assert_eq!(client.event("stopped")["body"]["reason"], "entry");

    client.request("stepIn", json!({"threadId": 1}));
    client.response("stepIn");
    client.event("continued");
    assert_eq!(client.event("stopped")["body"]["reason"], "step");

    client.request("continue", json!({"threadId": 1}));
    client.response("continue");
    client.event("continued");
    client.event("terminated");
    client.close();
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn adapter_process_steps_into_out_of_and_over_calls() {
    let (root, file) = fixture(
        "fn first(): Void {\n    inmut a: Int32 = 1;\n}\n\
         fn second(): Void {\n    inmut b: Int32 = 2;\n}\n\
         fn main(): Void {\n    first();\n    second();\n    inmut done: Int32 = 3;\n}\n",
    );
    let mut client = DapClient::spawn();
    initialize(&mut client);
    launch(&mut client, &root, &file, true);
    client.request("configurationDone", json!({}));
    client.response("configurationDone");
    assert_eq!(client.event("stopped")["body"]["reason"], "entry");

    client.request("stepIn", json!({"threadId": 1}));
    client.response("stepIn");
    client.event("continued");
    assert_eq!(client.event("stopped")["body"]["reason"], "step");
    client.request("stepIn", json!({"threadId": 1}));
    client.response("stepIn");
    client.event("continued");
    assert_eq!(client.event("stopped")["body"]["reason"], "step");
    client.request("stackTrace", json!({"threadId": 1}));
    let frames = client.response("stackTrace")["body"]["stackFrames"]
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(frames[0]["name"], "first", "stepIn stack: {frames:?}");

    client.request("stepOut", json!({"threadId": 1}));
    client.response("stepOut");
    client.event("continued");
    assert_eq!(client.event("stopped")["body"]["reason"], "step");
    client.request("stackTrace", json!({"threadId": 1}));
    let frames = client.response("stackTrace")["body"]["stackFrames"]
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(frames[0]["name"], "main");
    assert_eq!(frames[0]["line"], 8);

    client.request("next", json!({"threadId": 1}));
    client.response("next");
    client.event("continued");
    assert_eq!(client.event("stopped")["body"]["reason"], "step");
    client.request("stackTrace", json!({"threadId": 1}));
    let frames = client.response("stackTrace")["body"]["stackFrames"]
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(frames[0]["line"], 9);

    client.request("next", json!({"threadId": 1}));
    client.response("next");
    client.event("continued");
    assert_eq!(client.event("stopped")["body"]["reason"], "step");
    client.request("stackTrace", json!({"threadId": 1}));
    let frames = client.response("stackTrace")["body"]["stackFrames"]
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(frames[0]["line"], 10);

    client.request("continue", json!({"threadId": 1}));
    client.response("continue");
    client.event("continued");
    client.event("terminated");
    client.close();
    std::fs::remove_dir_all(root).unwrap();
}
