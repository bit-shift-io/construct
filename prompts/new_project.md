Generate documentation for project '{{NAME}}'.
Current Date: {{CURRENT_DATE}}

Requirements:
{{REQUIREMENTS}}

IMPORTANT: The project directory is ALREADY scaffolded. You are ALREADY inside it (`{{WORKDIR}}`).
Use `.` to refer to this directory in commands. Do not use the absolute path.
The following structure exists:
- `tasks/specs/` (roadmap.md, architecture.md, request.md)
- `tasks/` (Task subfolders)

REQUIRED ACTIONS (Execute in order):
**CONTINUOUS EXECUTION**: Perform Steps 0 through 9 in a *single* continuous output stream if possible. Do NOT stop after individual steps unless you need to ask the user a Question (Step 0).

0. **Analyze Requirements**: 
   - Review `{{REQUIREMENTS}}`.
   - **CRITICAL**: If the request is Vuaue (e.g. "make a website", "build an app"), you MUST **STOP** and ask the user for clarification.
   - Output your questions clearly.
   - Do NOT proceed to Step 1 until you have a granular understanding of the goal (e.g. "Rust CLI for system monitoring using sysinfo").
   - If requirements are clear, proceed immediately to Step 1.

1. Create `tasks/specs/guidelines.md` by POPULATING the following template:
   - **FILL IN THE BLANKS**: The template contains generic placeholders. You MUST Replace them with specific values.
   - **STRICTLY PRESERVE HEADERS**: Do NOT change any lines starting with optional `#`. Keep the document structure EXACTLY as read.
   - **CONTINUE**: After writing `tasks/specs/guidelines.md`, IMMEDIATELY proceed to Step 2.
   - **FORMATTING**: You MUST use **QUADRUPLE BACKTICKS** (` ```` `) to wrap the `write` block, because the content contains markdown (triple backticks).
     Example:
     ````write tasks/specs/guidelines.md
     # Content
     ````
````markdown
{{TEMPLATE_GUIDELINES}}
````

2. Create `tasks/specs/roadmap.md` by POPULATING the following template with high-level project milestones:
   - **SPECIFIC GOALS**: Do NOT use generic goals like "Core Features". Define WHAT features (e.g. "User Login", "Data Export").
   - **REAL MILESTONES**: Ensure the milestones map to the actual project phases (Init -> MVP -> Polish).
   - **FORMATTING**: Use **QUADRUPLE BACKTICKS** (` ```` `).
````markdown
{{TEMPLATE_ROADMAP}}
````

3. Create `tasks/specs/architecture.md` by POPULATING the following template with the system design (components, stack, data flow):
   - **CRITICAL**: You MUST fully populate ALL 10 SECTIONS.
   - **DETAILED**: Do NOT just list "Component A". You must describe its Responsibility, Dependencies, and Key Technologies.
   - **NO PLACEHOLDERS**: Do NOT leave any `<<...>>` text. Every field must be a concrete decision.
   - **FORMATTING**: Use **QUADRUPLE BACKTICKS** (` ```` `).
````markdown
{{TEMPLATE_ARCHITECTURE}}
````


4. Create `tasks/specs/progress.md` by POPULATING the following template (initialize with "Project Start"):
````markdown
{{TEMPLATE_PROGRESS}}
````

5. **Initialize First Milestone**:
   - **Review**: Look at `tasks/specs/roadmap.md` (Step 2). Identify **Milestone 1**.
   - **Create Folder**: Create the task directory for it (e.g. `tasks/001-initialization` or `tasks/001-mvp`).
   - **Constraint**: Do NOT use `tasks/001-init` unless "Initialization" is the actual name. Use the Roadmap Name.

6. Create `tasks/[MILESTONE_1_SLUG]/tasks.md` by POPULATING the following template:
   - **GRANULAR**: Break down high-level items into atomic actions (e.g. "Create struct", "Impl function", "Add test").
   - **ORDERING**: MUST be logical: Init -> Implementation -> Verification. Never verify before creating.
   - **NO "TBD"**: Every task must be actionable.
   - **FORMATTING**: Use **QUADRUPLE BACKTICKS** (` ```` `).
````markdown
{{TEMPLATE_TASKS}}
````

7. Create `tasks/[MILESTONE_1_SLUG]/plan.md` by POPULATING the following template:
   - **NO PLACEHOLDERS**: Do NOT leave any `<<...>>` or generic text.
   - **TECHNICAL DETAILS**: Specify the exact crates/libraries and commands to be used.
   - **FORMATTING**: Use **QUADRUPLE BACKTICKS** (` ```` `).
````markdown
{{TEMPLATE_PLAN}}
````
(Use `{{REQUIREMENTS}}` as requirements source. Do NOT leave template placeholders.)

8. Create `tasks/[MILESTONE_1_SLUG]/walkthrough.md` by POPULATING the following template (Initialize with headers ONLY):
   - **FORMATTING**: Use **QUADRUPLE BACKTICKS** (` ```` `).
````markdown
{{TEMPLATE_WALKTHROUGH}}
````

9. Output 'NO_MORE_STEPS' *only* after you have attempted all previous steps.
