use std::collections::{HashMap, HashSet};
use std::io::{self, BufRead, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread;

use serde_json::{Value as Json, json};
use zirk_cli::frontend::{self, FrontendResult};
use zirk_diagnostics::{DiagnosticSink, SourceMap, Span};
use zirk_interpreter::{
    DebugVariable, ExecutionResult, HostOutputError, Interpreter, InterpreterHost,
    InterpreterLimits, OutputDestination, OutputEvent,
};
use zirk_ir::Module;

const MAX_MESSAGE_BYTES: usize = 8 * 1024 * 1024;
const DEBUG_MAX_FRAMES: usize = 128;
const DEBUG_MAX_VARIABLES: usize = 2048;
const DEBUG_MAX_DEPTH: usize = 4;
const EXECUTION_SLICE: u64 = 1;

fn main() {
    if let Err(error) = serve(io::stdin(), io::stdout()) {
        eprintln!("zirk-dap: {error}");
        std::process::exit(1);
    }
}

fn serve<R, W>(input: R, output: W) -> io::Result<()>
where
    R: Read + Send + 'static,
    W: Write,
{
    let (sender, receiver) = mpsc::channel();
    thread::Builder::new()
        .name("zirk-dap-input".into())
        .spawn(move || read_requests(input, sender))?;
    Session::new(output).run(receiver)
}

fn read_requests<R: Read>(input: R, sender: Sender<Json>) {
    let mut reader = BufReader::new(input);
    loop {
        let mut content_length = None;
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => return,
                Ok(_) => {}
                Err(_) => return,
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
        let Some(length) = content_length.filter(|length| *length <= MAX_MESSAGE_BYTES) else {
            return;
        };
        let mut bytes = vec![0; length];
        if reader.read_exact(&mut bytes).is_err() {
            return;
        }
        let Ok(message) = serde_json::from_slice::<Json>(&bytes) else {
            return;
        };
        if sender.send(message).is_err() {
            return;
        }
    }
}

#[derive(Default)]
struct DebugHost {
    output: Vec<OutputEvent>,
    cancelled: bool,
}

impl DebugHost {
    fn cancel(&mut self) {
        self.cancelled = true;
    }
}

impl InterpreterHost for DebugHost {
    fn emit(&mut self, event: OutputEvent) -> Result<(), HostOutputError> {
        self.output.push(event);
        Ok(())
    }

    fn is_cancelled(&self) -> bool {
        self.cancelled
    }
}

struct RunningProgram {
    interpreter: Interpreter<'static>,
    sources: SourceMap,
    executable_lines: HashSet<(String, u32)>,
    path: PathBuf,
    output_cursor: usize,
    stop_on_entry: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RunMode {
    Paused,
    Continue,
    StepIn {
        location: Option<(String, u32)>,
        depth: usize,
    },
    StepOver {
        location: Option<(String, u32)>,
        depth: usize,
    },
    StepOut {
        depth: usize,
    },
    Terminated,
}

struct Session<W: Write> {
    writer: BufWriter<W>,
    sequence: u64,
    program: Option<RunningProgram>,
    host: DebugHost,
    mode: RunMode,
    breakpoints: HashMap<String, HashSet<u32>>,
    suppressed_breakpoint: Option<(String, u32)>,
    frame_handles: HashMap<i64, usize>,
    scope_handles: HashMap<i64, Vec<DebugVariable>>,
    next_variable_handle: i64,
    next_frame_handle: i64,
    terminate_requested: bool,
}

impl<W: Write> Session<W> {
    fn new(writer: W) -> Self {
        Self {
            writer: BufWriter::new(writer),
            sequence: 1,
            program: None,
            host: DebugHost::default(),
            mode: RunMode::Paused,
            breakpoints: HashMap::new(),
            suppressed_breakpoint: None,
            frame_handles: HashMap::new(),
            scope_handles: HashMap::new(),
            next_variable_handle: 1,
            next_frame_handle: 1,
            terminate_requested: false,
        }
    }

    fn run(mut self, receiver: Receiver<Json>) -> io::Result<()> {
        loop {
            if self.terminate_requested {
                break;
            }
            if self.mode == RunMode::Terminated {
                match receiver.recv() {
                    Ok(message) => self.handle_request(message)?,
                    Err(_) => break,
                }
                continue;
            }
            if self.mode == RunMode::Paused {
                match receiver.recv() {
                    Ok(message) => self.handle_request(message)?,
                    Err(_) => break,
                }
            } else {
                match receiver.try_recv() {
                    Ok(message) => self.handle_request(message)?,
                    Err(TryRecvError::Empty) => self.advance_execution()?,
                    Err(TryRecvError::Disconnected) => break,
                }
            }
        }
        Ok(())
    }

    fn handle_request(&mut self, request: Json) -> io::Result<()> {
        let command = request["command"].as_str().unwrap_or_default().to_owned();
        let request_seq = request["seq"].as_i64().unwrap_or_default();
        let args = request
            .get("arguments")
            .cloned()
            .unwrap_or_else(|| json!({}));
        match command.as_str() {
            "initialize" => {
                self.respond(
                    request_seq,
                    &command,
                    true,
                    json!({
                        "supportsConfigurationDoneRequest": true,
                        "supportsTerminateRequest": true,
                        "supportsPauseRequest": true,
                        "supportsStepBack": false,
                        "supportsConditionalBreakpoints": false,
                        "supportsSetVariable": false,
                        "supportsEvaluateForHovers": false
                    }),
                    None,
                )?;
                self.event("initialized", json!({}))?;
            }
            "launch" => match self.launch(&args) {
                Ok(()) => self.respond(request_seq, &command, true, json!({}), None)?,
                Err(message) => {
                    self.respond(request_seq, &command, false, json!({}), Some(&message))?;
                    self.output("stderr", &message)?;
                    self.event("terminated", json!({}))?;
                    self.mode = RunMode::Terminated;
                }
            },
            "setBreakpoints" => self.set_breakpoints(request_seq, &command, &args)?,
            "configurationDone" => {
                self.respond(request_seq, &command, true, json!({}), None)?;
                if self
                    .program
                    .as_ref()
                    .is_some_and(|program| program.stop_on_entry)
                {
                    self.mode = RunMode::Paused;
                    self.event(
                        "stopped",
                        json!({
                            "reason": "entry",
                            "threadId": 1,
                            "allThreadsStopped": true
                        }),
                    )?;
                } else {
                    self.mode = RunMode::Continue;
                }
            }
            "threads" => {
                self.respond(
                    request_seq,
                    &command,
                    true,
                    json!({"threads": [{"id": 1, "name": "Zirk interpreter"}]}),
                    None,
                )?;
            }
            "stackTrace" => self.stack_trace(request_seq, &command, &args)?,
            "scopes" => self.scopes(request_seq, &command, &args)?,
            "variables" => self.variables(request_seq, &command, &args)?,
            "continue" => self.resume(request_seq, &command, RunMode::Continue)?,
            "next" => self.step(request_seq, &command, "over")?,
            "stepIn" => self.step(request_seq, &command, "in")?,
            "stepOut" => self.step(request_seq, &command, "out")?,
            "pause" => {
                self.mode = RunMode::Paused;
                self.clear_debug_handles();
                self.respond(request_seq, &command, true, json!({}), None)?;
                self.event(
                    "stopped",
                    json!({
                        "reason": "pause",
                        "threadId": 1,
                        "allThreadsStopped": true
                    }),
                )?;
            }
            "terminate" | "disconnect" => {
                self.terminate_requested = args["terminateDebuggee"].as_bool().unwrap_or(true);
                self.host.cancel();
                self.mode = RunMode::Terminated;
                self.clear_debug_handles();
                self.respond(request_seq, &command, true, json!({}), None)?;
                self.event("terminated", json!({}))?;
                if self.terminate_requested {
                    self.program = None;
                }
            }
            "source" => {
                self.respond(
                    request_seq,
                    &command,
                    false,
                    json!({}),
                    Some("Zirk source is already available in the workspace"),
                )?;
            }
            _ => self.respond(
                request_seq,
                &command,
                false,
                json!({}),
                Some("This DAP command is not supported by the Zirk interpreter"),
            )?,
        }
        Ok(())
    }

    fn send(&mut self, message: Json) -> io::Result<()> {
        let body = serde_json::to_vec(&message)?;
        write!(self.writer, "Content-Length: {}\r\n\r\n", body.len())?;
        self.writer.write_all(&body)?;
        self.writer.flush()
    }

    fn respond(
        &mut self,
        request_seq: i64,
        command: &str,
        success: bool,
        body: Json,
        message: Option<&str>,
    ) -> io::Result<()> {
        let mut response = json!({
            "seq": self.next_sequence(),
            "type": "response",
            "request_seq": request_seq,
            "command": command,
            "success": success,
            "body": body
        });
        if let Some(message) = message {
            response["message"] = json!(message);
        }
        self.send(response)
    }

    fn event(&mut self, event: &str, body: Json) -> io::Result<()> {
        let sequence = self.next_sequence();
        self.send(json!({
            "seq": sequence,
            "type": "event",
            "event": event,
            "body": body
        }))
    }

    fn output(&mut self, category: &str, output: &str) -> io::Result<()> {
        self.event("output", json!({"category": category, "output": output}))
    }

    fn next_sequence(&mut self) -> u64 {
        let sequence = self.sequence;
        self.sequence = self.sequence.saturating_add(1);
        sequence
    }

    fn launch(&mut self, arguments: &Json) -> Result<(), String> {
        if let Some(target) = arguments["target"].as_str()
            && target != "interpreter"
        {
            return Err(format!(
                "unsupported Zirk debug target '{target}'; the available target is 'interpreter'"
            ));
        }
        let requested = arguments["program"]
            .as_str()
            .ok_or_else(|| "launch requires an explicit `program` path".to_owned())?;
        let requested = PathBuf::from(requested);
        let path = requested.canonicalize().map_err(|error| {
            format!("cannot open Zirk program {}: {error}", requested.display())
        })?;
        if !(path.is_file() || (path.is_dir() && path.join(".zkinit").is_file())) {
            return Err(format!(
                "{} is not a Zirk source file or project directory",
                path.display()
            ));
        }
        let root = arguments["cwd"]
            .as_str()
            .map(PathBuf::from)
            .unwrap_or_else(|| project_root(&path));
        let root = root
            .canonicalize()
            .map_err(|error| format!("cannot resolve workspace {}: {error}", root.display()))?;
        if !path.starts_with(&root) {
            return Err(format!(
                "program {} is outside the selected workspace {}",
                path.display(),
                root.display()
            ));
        }

        let mut sink = DiagnosticSink::new();
        let frontend = frontend::run_frontend(&path, &mut sink)
            .ok_or_else(|| format_frontend_diagnostics(&sink, &path))?;
        let FrontendResult {
            program,
            checked,
            sources,
            ..
        } = frontend;
        let module = zirk_ir::lower(&program, &checked);
        let executable_lines = executable_lines(&module, &sources);
        let mut interpreter = Interpreter::new_owned(module, InterpreterLimits::default())
            .map_err(|failure| format!("cannot create interpreter: {}", failure.message))?;
        self.host = DebugHost::default();
        interpreter
            .prepare_main_for_debug(&self.host)
            .map_err(|failure| format!("cannot start interpreter: {failure:?}"))?;

        let name = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("Zirk program")
            .to_owned();
        self.breakpoints.clear();
        self.clear_debug_handles();
        self.suppressed_breakpoint = None;
        self.program = Some(RunningProgram {
            interpreter,
            sources,
            executable_lines,
            path: path.clone(),
            output_cursor: 0,
            stop_on_entry: arguments["stopOnEntry"].as_bool().unwrap_or(false),
        });
        self.mode = RunMode::Terminated;
        self.event(
            "process",
            json!({
                "name": name,
                "zirkTarget": "interpreter",
                "systemProcessId": std::process::id(),
                "isLocalProcess": true,
                "startMethod": "launch"
            }),
        )
        .map_err(|error| error.to_string())?;
        Ok(())
    }

    fn set_breakpoints(
        &mut self,
        request_seq: i64,
        command: &str,
        arguments: &Json,
    ) -> io::Result<()> {
        let requested_path = arguments["source"]["path"].as_str().unwrap_or_default();
        let path = canonical_or_original(Path::new(requested_path));
        let lines = arguments["breakpoints"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let program = self.program.as_ref();
        let mut accepted = HashSet::new();
        let mut response = Vec::with_capacity(lines.len());
        for breakpoint in lines {
            let line = breakpoint["line"].as_u64().unwrap_or_default() as u32;
            let verified = program
                .is_some_and(|program| program.executable_lines.contains(&(path.clone(), line)));
            if verified {
                accepted.insert(line);
            }
            let mut result = json!({"verified": verified, "line": line});
            if !verified {
                result["message"] =
                    json!("No executable Zirk instruction maps to this source line");
            }
            response.push(result);
        }
        self.breakpoints.insert(path, accepted);
        self.respond(
            request_seq,
            command,
            true,
            json!({"breakpoints": response}),
            None,
        )
    }

    fn stack_trace(&mut self, request_seq: i64, command: &str, arguments: &Json) -> io::Result<()> {
        let Some(program) = self.program.as_ref() else {
            return self.respond(request_seq, command, true, json!({"stackFrames": []}), None);
        };
        if self.mode != RunMode::Paused {
            return self.respond(request_seq, command, true, json!({"stackFrames": []}), None);
        }
        let snapshot = program.interpreter.debug_snapshot(
            DEBUG_MAX_FRAMES,
            DEBUG_MAX_VARIABLES,
            DEBUG_MAX_DEPTH,
        );
        let Some(snapshot) = snapshot else {
            return self.respond(request_seq, command, true, json!({"stackFrames": []}), None);
        };
        let start = arguments["startFrame"].as_u64().unwrap_or(0) as usize;
        let levels = arguments["levels"]
            .as_u64()
            .unwrap_or(DEBUG_MAX_FRAMES as u64) as usize;
        self.frame_handles.clear();
        self.scope_handles.clear();
        let mut frames = Vec::new();
        for (index, frame) in snapshot
            .frames
            .iter()
            .enumerate()
            .skip(start)
            .take(levels.min(DEBUG_MAX_FRAMES))
        {
            let frame_id = self.next_frame_handle;
            self.next_frame_handle = self.next_frame_handle.saturating_add(1);
            self.frame_handles.insert(frame_id, index);
            let (source, line, column) = frame
                .span
                .and_then(|span| source_location(&program.sources, span))
                .unwrap_or_else(|| (program.path.clone(), 1, 1));
            frames.push(json!({
                "id": frame_id,
                "name": frame.function,
                "source": {"name": source.file_name().and_then(|name| name.to_str()).unwrap_or("<source>"), "path": source},
                "line": line,
                "column": column
            }));
        }
        self.respond(
            request_seq,
            command,
            true,
            json!({"stackFrames": frames, "totalFrames": snapshot.frames.len()}),
            None,
        )
    }

    fn scopes(&mut self, request_seq: i64, command: &str, arguments: &Json) -> io::Result<()> {
        if self.mode != RunMode::Paused {
            return self.respond(request_seq, command, true, json!({"scopes": []}), None);
        }
        let frame_id = arguments["frameId"].as_i64().unwrap_or_default();
        let Some(index) = self.frame_handles.get(&frame_id).copied() else {
            return self.respond(request_seq, command, true, json!({"scopes": []}), None);
        };
        let Some(program) = self.program.as_ref() else {
            return self.respond(request_seq, command, true, json!({"scopes": []}), None);
        };
        let Some(snapshot) = program.interpreter.debug_snapshot(
            DEBUG_MAX_FRAMES,
            DEBUG_MAX_VARIABLES,
            DEBUG_MAX_DEPTH,
        ) else {
            return self.respond(request_seq, command, true, json!({"scopes": []}), None);
        };
        let Some(frame) = snapshot.frames.get(index) else {
            return self.respond(request_seq, command, true, json!({"scopes": []}), None);
        };
        let variables_reference = self.allocate_handle();
        self.scope_handles
            .insert(variables_reference, frame.locals.clone());
        self.respond(
            request_seq,
            command,
            true,
            json!({"scopes": [{
                "name": "Locals",
                "presentationHint": "locals",
                "variablesReference": variables_reference,
                "expensive": false
            }]}),
            None,
        )
    }

    fn variables(&mut self, request_seq: i64, command: &str, arguments: &Json) -> io::Result<()> {
        if self.mode != RunMode::Paused {
            return self.respond(request_seq, command, true, json!({"variables": []}), None);
        }
        let reference = arguments["variablesReference"].as_i64().unwrap_or_default();
        let Some(variables) = self.scope_handles.get(&reference).cloned() else {
            return self.respond(request_seq, command, true, json!({"variables": []}), None);
        };
        let dap_variables = variables
            .into_iter()
            .map(|variable| {
                let variables_reference = if variable.children.is_empty() {
                    0
                } else {
                    let handle = self.allocate_handle();
                    self.scope_handles.insert(handle, variable.children.clone());
                    handle
                };
                json!({
                    "name": variable.name,
                    "value": variable.value,
                    "type": variable.type_name,
                    "variablesReference": variables_reference,
                    "evaluateName": variable.name
                })
            })
            .collect::<Vec<_>>();
        self.respond(
            request_seq,
            command,
            true,
            json!({"variables": dap_variables}),
            None,
        )
    }

    fn resume(&mut self, request_seq: i64, command: &str, mode: RunMode) -> io::Result<()> {
        let location = self.current_location();
        self.suppressed_breakpoint = location;
        self.mode = mode;
        self.clear_debug_handles();
        self.respond(
            request_seq,
            command,
            true,
            json!({"allThreadsContinued": true}),
            None,
        )?;
        self.event(
            "continued",
            json!({"threadId": 1, "allThreadsContinued": true}),
        )
    }

    fn step(&mut self, request_seq: i64, command: &str, kind: &str) -> io::Result<()> {
        let location = self.current_location();
        let depth = self
            .program
            .as_ref()
            .map_or(0, |program| program.interpreter.debug_frame_depth());
        let mode = match kind {
            "in" => RunMode::StepIn {
                location: location.clone(),
                depth,
            },
            "over" => RunMode::StepOver {
                location: location.clone(),
                depth,
            },
            _ => RunMode::StepOut { depth },
        };
        self.suppressed_breakpoint = location;
        self.mode = mode;
        self.clear_debug_handles();
        self.respond(request_seq, command, true, json!({}), None)?;
        self.event(
            "continued",
            json!({"threadId": 1, "allThreadsContinued": true}),
        )
    }

    fn advance_execution(&mut self) -> io::Result<()> {
        if self.program.is_none() {
            self.mode = RunMode::Paused;
            return Ok(());
        }
        let current_location = self.current_location();
        if let Some(current) = &current_location {
            if self.suppressed_breakpoint.as_ref() == Some(current) {
                // Suppress all lowered instructions on the same source line,
                // then let a loop hit that line again after control leaves it.
            } else {
                self.suppressed_breakpoint = None;
                if self
                    .breakpoints
                    .get(&current.0)
                    .is_some_and(|lines| lines.contains(&current.1))
                {
                    self.stop("breakpoint")?;
                    return Ok(());
                }
            }
        }

        let start_location = current_location;
        let result = self
            .program
            .as_mut()
            .unwrap()
            .interpreter
            .run_main_with_host_slice(&mut self.host, EXECUTION_SLICE);
        self.flush_program_output()?;
        match result {
            ExecutionResult::Yielded => {
                let next_location = self.current_location();
                let next_depth = self
                    .program
                    .as_ref()
                    .map_or(0, |program| program.interpreter.debug_frame_depth());
                let should_stop = match &self.mode {
                    RunMode::StepIn { location, depth } => {
                        next_location != *location || next_depth > *depth
                    }
                    RunMode::StepOver { location, depth } => {
                        next_depth <= *depth && next_location != *location
                    }
                    RunMode::StepOut { depth } => next_depth < *depth,
                    RunMode::Continue | RunMode::Paused | RunMode::Terminated => false,
                };
                if should_stop {
                    self.stop("step")?;
                } else if start_location.is_none() {
                    self.suppressed_breakpoint = None;
                }
            }
            ExecutionResult::Completed { .. } => self.finish(0, "Program completed")?,
            other => {
                self.output("stderr", &format!("Zirk execution stopped: {other:?}\n"))?;
                self.finish(1, "Program failed")?;
            }
        }
        Ok(())
    }

    fn stop(&mut self, reason: &str) -> io::Result<()> {
        self.mode = RunMode::Paused;
        self.suppressed_breakpoint = None;
        self.clear_debug_handles();
        self.event(
            "stopped",
            json!({
                "reason": reason,
                "threadId": 1,
                "allThreadsStopped": true
            }),
        )
    }

    fn finish(&mut self, exit_code: i32, message: &str) -> io::Result<()> {
        self.mode = RunMode::Terminated;
        self.suppressed_breakpoint = None;
        self.clear_debug_handles();
        self.event("exited", json!({"exitCode": exit_code}))?;
        self.event("terminated", json!({"restart": false}))?;
        if !message.is_empty() {
            self.output("console", &format!("{message}\n"))?;
        }
        Ok(())
    }

    fn current_location(&self) -> Option<(String, u32)> {
        let program = self.program.as_ref()?;
        let span = program.interpreter.debug_location()?;
        let (path, line, _) = source_location(&program.sources, span)?;
        Some((canonical_or_original(&path), line))
    }

    fn flush_program_output(&mut self) -> io::Result<()> {
        let pending = {
            let Some(program) = self.program.as_mut() else {
                return Ok(());
            };
            let mut pending = Vec::new();
            while program.output_cursor < self.host.output.len() {
                let event = &self.host.output[program.output_cursor];
                program.output_cursor += 1;
                let category = match event.destination {
                    OutputDestination::Stdout => "stdout",
                    OutputDestination::Stderr => "stderr",
                };
                let output = if event.truncated {
                    "[Zirk output truncated]\n".to_owned()
                } else {
                    event.text.clone()
                };
                pending.push((category, output));
            }
            pending
        };
        for (category, output) in pending {
            self.output(category, &output)?;
        }
        Ok(())
    }

    fn allocate_handle(&mut self) -> i64 {
        let handle = self.next_variable_handle;
        self.next_variable_handle = self.next_variable_handle.saturating_add(1);
        handle
    }

    fn clear_debug_handles(&mut self) {
        self.frame_handles.clear();
        self.scope_handles.clear();
    }
}

fn project_root(path: &Path) -> PathBuf {
    if path.is_dir() {
        return path.to_path_buf();
    }
    path.ancestors()
        .find(|candidate| candidate.join(".zkinit").is_file())
        .map(Path::to_path_buf)
        .or_else(|| path.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| path.to_path_buf())
}

fn canonical_or_original(path: &Path) -> String {
    path.canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .into_owned()
}

fn format_frontend_diagnostics(sink: &DiagnosticSink, fallback: &Path) -> String {
    let messages = sink
        .diagnostics()
        .iter()
        .map(|diagnostic| format!("{}: {}", diagnostic.code, diagnostic.message))
        .collect::<Vec<_>>();
    if messages.is_empty() {
        format!("could not compile {}", fallback.display())
    } else {
        format!(
            "could not compile {}:\n{}",
            fallback.display(),
            messages.join("\n")
        )
    }
}

fn executable_lines(module: &Module, sources: &SourceMap) -> HashSet<(String, u32)> {
    module
        .functions
        .iter()
        .flat_map(|function| function.blocks.iter())
        .flat_map(|block| block.instructions.iter())
        .filter_map(|instruction| {
            let (path, line, _) = source_location(sources, instruction.span)?;
            Some((canonical_or_original(&path), line))
        })
        .collect()
}

fn source_location(sources: &SourceMap, span: Span) -> Option<(PathBuf, u32, u32)> {
    let source = sources.file(span.file);
    let (line, column) = source.line_column_utf16(span.start);
    Some((
        PathBuf::from(source.name()),
        line.saturating_add(1),
        column.saturating_add(1),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch() -> PathBuf {
        let root = std::env::temp_dir().join(format!("zirk-dap-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    fn decoded_messages(bytes: &[u8]) -> Vec<Json> {
        let mut reader = BufReader::new(bytes);
        let mut messages = Vec::new();
        loop {
            let mut length = None;
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap() == 0 {
                    return messages;
                }
                if line == "\r\n" || line == "\n" {
                    break;
                }
                if let Some((name, value)) = line.split_once(':')
                    && name.eq_ignore_ascii_case("content-length")
                {
                    length = value.trim().parse::<usize>().ok();
                }
            }
            let Some(length) = length else {
                return messages;
            };
            let mut body = vec![0; length];
            reader.read_exact(&mut body).unwrap();
            messages.push(serde_json::from_slice(&body).unwrap());
        }
    }

    #[test]
    fn dap_framing_reads_utf8_content_by_byte_length() {
        let body =
            r#"{"seq":7,"type":"request","command":"initialize","arguments":{"locale":"español"}}"#;
        let framed = format!("Content-Length: {}\r\n\r\n", body.len());
        let (sender, receiver) = mpsc::channel();
        let mut packet = framed.into_bytes();
        packet.extend_from_slice(body.as_bytes());
        read_requests(packet.as_slice(), sender);
        let request = receiver.recv().unwrap();
        assert_eq!(request["arguments"]["locale"], "español");
    }

    #[test]
    fn interpreter_dap_verifies_breakpoints_and_exposes_paused_local_values() {
        let root = scratch();
        let file = root.join("main.zrk");
        std::fs::write(
            &file,
            "fn main(): Void {\n    inmut value: Int32 = 41;\n    inmut copy = value;\n}\n",
        )
        .unwrap();
        let file = file.canonicalize().unwrap();
        let mut session = Session::new(Vec::new());
        session
            .launch(&json!({"program": file, "cwd": root, "stopOnEntry": false}))
            .expect("valid fixture launches in interpreter");
        session
            .set_breakpoints(
                2,
                "setBreakpoints",
                &json!({
                    "source": {"path": file},
                    "breakpoints": [{"line": 4}, {"line": 3}]
                }),
            )
            .unwrap();
        assert!(
            session
                .program
                .as_ref()
                .unwrap()
                .executable_lines
                .contains(&(file.to_string_lossy().into_owned(), 3))
        );
        session.mode = RunMode::Continue;
        for _ in 0..16 {
            session.advance_execution().unwrap();
            if session.mode == RunMode::Paused {
                break;
            }
        }
        assert_eq!(session.mode, RunMode::Paused, "breakpoint stops execution");
        assert_eq!(session.current_location().unwrap().1, 3);

        session
            .stack_trace(3, "stackTrace", &json!({"threadId": 1}))
            .unwrap();
        session.scopes(4, "scopes", &json!({"frameId": 1})).unwrap();
        session
            .variables(5, "variables", &json!({"variablesReference": 1}))
            .unwrap();
        let session = session;
        let messages = decoded_messages(session.writer.get_ref());
        let breakpoint_response = messages
            .iter()
            .find(|message| message["request_seq"] == 2)
            .unwrap();
        assert_eq!(
            breakpoint_response["body"]["breakpoints"][0]["verified"],
            false
        );
        assert_eq!(
            breakpoint_response["body"]["breakpoints"][1]["verified"],
            true
        );
        let variables_response = messages
            .iter()
            .find(|message| message["request_seq"] == 5)
            .unwrap();
        let variables = variables_response["body"]["variables"].as_array().unwrap();
        let value = variables
            .iter()
            .find(|variable| variable["name"] == "value")
            .unwrap();
        assert_eq!(value["value"], "41");
        assert_eq!(value["type"], "Int32");
        let copy = variables
            .iter()
            .find(|variable| variable["name"] == "copy")
            .unwrap();
        assert_eq!(copy["value"], "<unavailable>");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn unsupported_debug_targets_fail_with_available_target() {
        let mut session = Session::new(Vec::new());
        let error = session
            .launch(&json!({"target": "native", "program": "unused.zrk"}))
            .unwrap_err();
        assert!(error.contains("unsupported Zirk debug target 'native'"));
        assert!(error.contains("available target is 'interpreter'"));
    }
}
