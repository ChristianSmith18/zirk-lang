## ADDED Requirements

### Requirement: Installable per-user macOS toolchain bundle
A macOS arm64 development release SHALL provide a versioned compiler and matching runtime bundle that can be installed for the current user without administrator privileges. The installed compiler SHALL resolve its matching runtime and bundled non-system dynamic dependencies without relying on the source checkout or machine-specific package-manager paths.

#### Scenario: Install a verified release bundle
- **WHEN** a user runs the installer on a supported macOS arm64 host with a valid release archive
- **THEN** the installer verifies the payload and installs the versioned toolchain beneath the user's local data directory without requesting administrator privileges

#### Scenario: Reject an unsupported host or damaged payload
- **WHEN** the installer runs on an unsupported operating system or architecture, or the archive checksum does not match
- **THEN** it stops before activating the release and prints an actionable error message

### Requirement: Stable per-user command entry point
The installation SHALL expose a `zirk` command through the user's local executable directory without silently editing shell startup files. The stable command SHALL select the active installed release while retaining its compiler and runtime pairing.

#### Scenario: Invoke the installed compiler from another directory
- **WHEN** the user's local executable directory is on `PATH` and the user invokes `zirk` outside the repository
- **THEN** the installed compiler reports its version and locates the runtime bundled with the same release

#### Scenario: Local executable directory is not on PATH
- **WHEN** the installer detects that its executable directory is not on `PATH`
- **THEN** it completes the installation and prints the exact path that must be added by the user

### Requirement: Self-service per-user uninstallation
An installed macOS Zirk compiler SHALL provide `zirk uninstall` to remove its managed command link, active-version link, and all recognized Zirk compiler/runtime version directories beneath the current user's `~/.local` hierarchy. Uninstallation SHALL leave repository release artifacts and unrelated command links untouched, and SHALL print clear progress and completion output.

#### Scenario: Remove the managed installation
- **WHEN** a user runs `zirk uninstall` from the installed compiler
- **THEN** the stable command link, active-version link, and managed version directories are removed without changing shell startup files or repository releases

#### Scenario: Refuse an unrelated compiler or command link
- **WHEN** `uninstall` is invoked from outside a managed Zirk version or the stable command path is not Zirk's managed symlink
- **THEN** the command explains the mismatch and stops before deleting installation files

### Requirement: Native build prerequisite diagnostics
The installed compiler SHALL distinguish frontend-only checking from native compilation and SHALL diagnose unavailable Apple native-build tools clearly.

#### Scenario: Check a source file without Apple build tools
- **WHEN** Apple Command Line Tools or an Xcode macOS SDK is unavailable and the user runs a frontend-only check
- **THEN** checking remains available without invoking the native linker

#### Scenario: Build without Apple build tools
- **WHEN** Apple Command Line Tools and an Xcode macOS SDK are unavailable and the user requests a native build
- **THEN** the compiler reports the missing prerequisite and explains how to install Apple's tools
