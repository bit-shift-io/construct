Generate documentation for project '{{NAME}}'.

Requirements:
{{REQUIREMENTS}}

IMPORTANT: The project directory is ALREADY scaffolded. You are ALREADY inside it (`{{WORKDIR}}`).
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

3. Create `{{ACTIVE_TASK}}/plan.md` by POPULATING the following template for this specific setup task:
```markdown
{{TEMPLATE_PLAN}}
```
(Use `{{ACTIVE_TASK}}/request.md` as requirements source. Do NOT leave template placeholders.)

4. Create `{{ACTIVE_TASK}}/walkthrough.md` by POPULATING the following template with a verification strategy:
```markdown
{{TEMPLATE_WALKTHROUGH}}
```

5. YOU MUST CREATE ALL 4 FILES.
5. STOP IMMEDIATELY after creating/populating these files.
   - Verify files are updated before stopping.
5. DO NOT write to the root directory.
6. Output 'NO_MORE_STEPS' to finish this turn.
