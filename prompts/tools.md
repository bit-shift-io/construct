
# AVAILABLE TOOLS
1. **Write File**:
```write path/to/file
Content here...
```
2. **Read File**:
```read path/to/file```
3. **List Directory**:
```list path/to/dir```
4. **Run Command**:
```bash
cmd args
```

# RULES
1. Use `write` blocks for ALL file creation/edits. DO NOT use `cat` or `echo` redirection.
2. Wait for the result before proceeding.
3. CRITICAL: Do NOT put commentary inside the code block.
4. **Command Arguments**: Use relative paths (e.g. `.`, `src/`). Do NOT use absolute paths.
5. **File Updates**: The `write` tool OVERWRITES the entire file. To update a file, you MUST read it first, modify the content, and then write the entire updated content back.

# Tool Use
* Make sure to adhere to the tools schema.
* Provide every required argument.
* DO NOT use tools to access items that are already available in the context section, UNLESS you are reading the file to update it (Read-Modify-Write).
* Use only the tools that are currently available.
* DO NOT use a tool that is not available just because it appears in the conversation. This means the user turned it off.
* NEVER run commands that don't terminate on their own such as web servers (like npm run start, npm run dev, python -m http.server, etc) or file watchers.
