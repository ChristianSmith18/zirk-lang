//! # zirk-lsp
//!
//! **Responsibility:** the `zirk-lsp` executable — the Zirk language server.
//! It serves the editor-facing capabilities (diagnostics, hover, symbols,
//! formatting) over LSP on stdio, backed by the compiler's own frontend.
//!
//! **Boundary:** the binary is a thin shell; the server implementation lives
//! in the `zirk_lsp` library where tests can drive it without a pipe.

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = zirk_lsp::service();
    tower_lsp::Server::new(stdin, stdout, socket)
        .serve(service)
        .await;
}
