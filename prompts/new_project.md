Generate documentation for project '{{NAME}}'.
Current Date: {{CURRENT_DATE}}

Requirements:
{{REQUIREMENTS}}

IMPORTANT: The project directory is ALREADY scaffolded. You are ALREADY inside it (`{{WORKDIR}}`).
Use `.` to refer to this directory in commands. Do not use the absolute path.
The following structure exists:
- `specs/` (roadmap.md, architecture.md)
- `{{ACTIVE_TASK}}/` (request.md, plan.md)

REQUIRED ACTIONS (Execute in order):
1. Create `specs/roadmap.md` by POPULATING the following template with high-level project milestones:
```markdown
{{TEMPLATE_ROADMAP}}
```

2. Create `specs/architecture.md` by POPULATING the following template with the system design (components, stack, data flow):
```markdown
{{TEMPLATE_ARCHITECTURE}}
```


3. Create `specs/progress.md` by POPULATING the following template (initialize with "Project Start"):
```markdown
{{TEMPLATE_PROGRESS}}
```

4. Create `{{ACTIVE_TASK}}/tasks.md` by POPULATING the following template with a granular checklist of actions to perform:
```markdown
# Task: Initialization

- [ ] Initialize Cargo project (`cargo init --name <name>`)
- [ ] Add dependencies to `Cargo.toml`
    - [ ] Add `sysinfo` (or relevant libs)
    - [ ] Add `anyhow`, `clap`, etc.
- [ ] Create Core Modules (Do NOT create stubs - implement struct definitions)
    - [ ] `src/domain/mod.rs` (Data types)
    - [ ] `src/application/mod.rs` (Logic/Traits)
    - [ ] `src/infrastructure/mod.rs` (IO/Repo impls)
    - [ ] `src/interface/mod.rs` (CLI/API)
- [ ] Verify build (`cargo check`)
```

5. Create `{{ACTIVE_TASK}}/plan.md` by POPULATING the following template for this specific setup task:
```markdown
{{TEMPLATE_PLAN}}
```
(Use `{{ACTIVE_TASK}}/request.md` as requirements source. Do NOT leave template placeholders.)

6. Create `{{ACTIVE_TASK}}/walkthrough.md` by POPULATING the following template (Initialize with headers ONLY - DO NOT add specific content):
```markdown
{{TEMPLATE_WALKTHROUGH}}
```

7. Output 'NO_MORE_STEPS' immediately after writing all 6 files.
