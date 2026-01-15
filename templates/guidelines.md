# AI Coding Assistant Guidelines

## 1. Tech Stack & Key Libraries
- **Language:** <<LANGUAGE_VERSION>>
- **Framework:** <<FRAMEWORK>>
- **Styling:** <<STYLING_LIB>>
- **State Management:** <<STATE_MANAGEMENT>>
- **Database/ORM:** <<DATABASE_ORM>>
- **Testing:** <<TESTING_LIB>>

## 2. Coding Style & Principles
- **Functional Patterns:** Prefer functional programming patterns over classes where possible (if applicable).
- **Type Safety:** <<TYPE_SAFETY_RULES>>
- **Naming:**
  - Variables/Functions: <<NAMING_CONVENTION_VARS>>
  - Components/Structs: <<NAMING_CONVENTION_TYPES>>
  - Constants: SCREAMING_SNAKE_CASE
- **Comments:** Document public interfaces. Explain "Why", not "What".
- **Error Handling:** Handle errors at boundaries. Avoid unwrap/panic in production code.

## 3. Architecture & Structure
- **Folder Structure:**
  - `src/domain`: Core logic
  - `src/application`: Use cases
  - `src/infrastructure`: Implementation details
- **Component Pattern:** <<COMPONENT_PATTERN>>

## 4. "Do Not" (Negative Constraints)
- DO NOT remove existing comments unless they are obsolete.
- DO NOT output large blocks of code if only 3 lines changed (use search/replace blocks).
- [DO NOT use filesystem operations in frontend code]
- [DO NOT use print/console.log for production; use a structured Logger]
- [DO NOT use raw SQL if an ORM is specified]

## 5. Common Commands
- **Run Dev Server:** `<<CMD_RUN_DEV>>`
- **Run Tests:** `<<CMD_RUN_TEST>>`
- **Lint:** `<<CMD_LINT>>`
- **Build:** `<<CMD_BUILD>>`
