# AVAILABLE TOOLS

1. **Write File**:
```write path/to/file
Content here...
```
   - **For Markdown/Code**: If content contains backticks, use 4-ticks:
````write path/to/markdown.md
# Header
```rust
fn main() {}
```
````

2. **Read File**:
```read path/to/file```

3. **List Directory**:
```list path/to/dir```

4. **Run Command**:
```run_command
cmd args
```
- **RESTRICTION**: DO NOT use this for file operations (`ls`, `cat`, `pwd`, `echo`, `sed`) if a specific tool exists (e.g. `list`, `read`, `write`).
- **RESTRICTION**: DO NOT use interactive commands (`vim`, `nano`, `top`).

5. **Find Files**:
```find path pattern```
- **Description**: Search for files matching a glob pattern (e.g., `*.rs`, `**/*.md`).
- **Usage**: `find src "*.rs"`
- **Prefer this over `run_command find`**.

# RULES

1. **Strict Formatting**: You MUST use the triple-backtick code block format shown above.
   - **DO NOT** use conversational formats like `**Action**: Read file`.
   - **DO NOT** use single backticks for the block itself.
2. **File Creation**: Use `write` blocks. DO NOT use `run_command` with `cat`, `echo`, or `sed` to create or edit files.
3. **Paths**:
   - Use **relative paths** (e.g., `src/main.rs`, `.`) whenever possible.
   - If a `read` fails with "File not found", **DO NOT RETRY IMMEDIATELY**.
   - **Verify the path** first using `list` or `find` to see where the file actually is.
4. **File Updates**: The `write` tool OVERWRITES the entire file. You MUST read the file first, apply your changes to the content, and then write the full content back.
5. **No Commentary**: Do NOT put comments inside the tool usage block.
6. **No Daemons**: NEVER run commands that don't terminate (e.g., servers, file watchers).

# ERROR HANDLING

*   **Read Errors**: If you get "Failed to read file", assume the path is wrong. List the parent directory to find the correct path.
*   **Parsing Errors**: If the system says "Unparsed action", check your formatting. Ensure you are using triple backticks.

# CRITICAL FORMATTING RULES
1. **NO XML**: DO NOT use XML tags like `<bash>`, `<write_to_file>`, or `<plan>`.
2. **MARKDOWN ONLY**: ALL code (file content, commands) MUST be inside triple-backtick code blocks.
   - **EXCEPTION**: If the content ITSELF contains triple backticks (e.g. writing a markdown file with code blocks), you MUST use **QUADRUPLE backticks** (` ```` `) to wrap the tool block. This is CRITICAL for `tasks/specs/architecture.md` and other markdown files.
3. **STRICT TOOL USAGE**: Follow the format in `Available Tools` EXACTLY. Do not invent new tools or arguments.
