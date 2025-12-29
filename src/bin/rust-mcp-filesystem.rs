// Wrapper for rust-mcp-filesystem binary
// This file compiles and runs the rust-mcp-filesystem tool as part of construct

use clap::Parser;

#[tokio::main]
async fn main() {
    let arguments = rust_mcp_filesystem::cli::CommandArguments::parse();
    if let Err(err) = arguments.validate() {
        eprintln!("Error: {err}");
        return;
    };

    if let Err(error) = rust_mcp_filesystem::server::start_server(arguments).await {
        eprintln!("{error}");
    }
}
