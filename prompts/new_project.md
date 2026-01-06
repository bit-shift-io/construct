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

5. Output 'NO_MORE_STEPS' immediately after writing all 4 files.
