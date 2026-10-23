## Purpose

Provide a runtime debugging interface for Zirk programs that connects source locations to executable state through the Debug Adapter Protocol. The adapter reports only values and stepping behavior the selected execution target can actually observe.

## ADDED Requirements

### Requirement: Debug target and capability discovery

The debugger SHALL identify the selected Zirk execution target and report the capabilities it supports before the editor enables corresponding controls. Unsupported target features SHALL be reported explicitly.

#### Scenario: Interpreter target capabilities
- **WHEN** a Zirk program is launched in the interpreter with source-span and frame inspection support
- **THEN** the adapter reports the available breakpoint, stepping, stack, scope, and variable capabilities

#### Scenario: Native target capabilities
- **WHEN** a native Zirk executable is launched with supported debug metadata
- **THEN** the adapter reports the available native breakpoint, stepping, stack, scope, and variable capabilities

#### Scenario: Unsupported target capability
- **WHEN** the selected target cannot expose a requested debugging feature
- **THEN** the adapter reports that limitation and does not present fabricated source state

#### Scenario: Unsupported Zirk target
- **WHEN** a launch configuration selects a target without an implemented adapter
- **THEN** launch fails with the supported target name and the unsupported target named explicitly

### Requirement: Source breakpoints and stepping

The debugger SHALL map source breakpoints to executable locations and SHALL support continue, pause, step-in, step-over, and step-out when supported by the target. Breakpoint verification SHALL indicate whether the requested source location is executable.

#### Scenario: Verified breakpoint
- **WHEN** a breakpoint is set on an executable Zirk source line
- **THEN** the adapter marks it verified and execution pauses at the mapped source location

#### Scenario: Non-executable source line
- **WHEN** a breakpoint is set on a comment, blank line, or non-executable declaration
- **THEN** the adapter reports it as unverified or relocates it to the next executable location with an explicit explanation

#### Scenario: Stepping across source locations
- **WHEN** the paused program is stepped
- **THEN** the editor receives the current Zirk source location and the requested stepping behavior is honored when supported

### Requirement: Runtime stack and variable inspection

When execution is paused, the debugger SHALL expose stack frames, scopes, and values available from the running target. Runtime values SHALL be distinguished from static types shown by language analysis, and unavailable or optimized-out values SHALL be identified as unavailable.

#### Scenario: Inspect a live local
- **WHEN** execution pauses in a function with a live local variable
- **THEN** the variables view shows its current runtime value and, when available, its Zirk static type

#### Scenario: Runtime object identity
- **WHEN** a local contains a reference to a runtime object and the target can inspect it
- **THEN** the debugger exposes the observed object and its fields at that pause
- **AND** the static language service does not claim this identity outside the paused runtime state

#### Scenario: Optimized-out variable
- **WHEN** the target no longer retains a source variable at the pause location
- **THEN** the debugger reports the value as unavailable rather than returning stale data

#### Scenario: SecretString inspection
- **WHEN** a paused local or aggregate contains a standard-library SecretString
- **THEN** the debugger displays a redacted marker and does not expose the payload or its private fields

#### Scenario: Runtime handle invalidation
- **WHEN** execution resumes or the debuggee terminates
- **THEN** frame and variable handles from the previous pause return no runtime values

### Requirement: Debug session lifecycle and safety

The debugger SHALL support launch and terminate lifecycle messages, associate each session with an explicit workspace and executable, and release processes and resources when the session ends. It SHALL not execute a different file or workspace from the selected launch configuration.

#### Scenario: Terminate session
- **WHEN** the user terminates a debug session
- **THEN** the launched process is stopped and adapter resources are released

#### Scenario: Launch configuration selects a program
- **WHEN** the user starts debugging with a configured Zirk program
- **THEN** the adapter launches that program using the selected target and reports launch failures in the debug console
