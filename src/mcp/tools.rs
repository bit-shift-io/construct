// MCP Tool Definitions
//
// This module documents the tools that will be available through the MCP server.
// The actual tool implementations use the `#[tool]` macro from rmcp and are
// defined in the MCP server implementation.
//
// Available Tools:
// - execute_command: Execute shell commands with timeout support
// - read_file: Read the contents of a file within allowed directories
// - write_file: Write content to a file within allowed directories
// - list_directory: List contents of a directory
// - create_directory: Create a new directory
// - delete_file: Delete a file or directory
//
// These tools are implemented using the rmcp `#[tool]` macro and are
// exposed through the MCP server sidecar process.
