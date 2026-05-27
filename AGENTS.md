# AGENTS.md - Development Guidelines for LegalGenie Agent

## Project Overview
LegalGenie Agent is a legal technology project. Currently in initial setup phase with requirement analysis structure in place.

## Build / Lint / Test Commands

### Python Project (Anticipated)
```bash
# Install dependencies
pip install -e ".[dev]"

# Run all tests
pytest

# Run a single test
pytest tests/test_module.py::test_function_name -v

# Run tests with coverage
pytest --cov=legalminds --cov-report=html

# Linting
ruff check .
ruff format .
mypy legalminds/

# Run a single file's tests
pytest tests/test_specific.py -v
```

### Node.js/TypeScript Project (Alternative)
```bash
# Install dependencies
npm install

# Run all tests
npm test

# Run a single test
npm test -- --testNamePattern="test name"
# or
npx jest path/to/test.ts -t "test name"

# Linting
npm run lint
npm run format

# Type checking
npm run typecheck
```

## Code Style Guidelines

### General Principles
- Write clear, readable, and maintainable code
- Prefer explicit over implicit behavior
- Document complex logic with comments
- Keep functions small and focused (single responsibility)

### Imports
- Group imports: standard library → third-party → local modules
- Use absolute imports for internal modules
- Avoid circular dependencies
- TypeScript: Use explicit type imports (`import type { }`)

### Formatting
- Indentation: 4 spaces (Python) or 2 spaces (TypeScript)
- Line length: 88 chars (Python/Black) or 100 chars (TypeScript)
- Use trailing commas in multi-line structures
- Blank lines: 2 between top-level definitions, 1 between methods

### Naming Conventions
- **Classes/Types**: PascalCase (`LegalDocument`, `CaseAnalysis`)
- **Functions/Methods**: snake_case (Python) or camelCase (TS)
- **Constants**: UPPER_SNAKE_CASE
- **Private members**: Prefix with `_` (Python) or `#` (TS private)
- **Test functions**: Prefix with `test_` or `it_`

### Type Annotations
- Python: Use type hints for all function signatures
- TypeScript: Enable strict mode, avoid `any`
- Document complex types with type aliases
- Use generics for reusable components

### Error Handling
- Use specific exception types, not bare `Exception`
- Python: Prefer custom exception classes for domain errors
- TypeScript: Use typed errors with discriminated unions
- Log errors with context (file, line, input data)
- Never silently catch and ignore errors

### Testing Guidelines
- Name tests descriptively: `test_<function>_<scenario>_<expected>`
- Arrange-Act-Assert (AAA) pattern for test structure
- Mock external dependencies (APIs, databases, filesystem)
- Test edge cases and error conditions
- Maintain >80% code coverage for critical modules

### Documentation
- Docstrings for all public functions/classes
- Include Args, Returns, Raises sections (Python)
- JSDoc comments for TypeScript
- Keep README updated with setup instructions

## Cursor / Copilot Rules
No existing `.cursorrules` or `.github/copilot-instructions.md` found in this repository.

## Architecture Notes
- Domain-driven design recommended for legal domain complexity
- Separate business logic from framework/presentation code
- Use dependency injection for testability
- Consider repository pattern for data access

## Security Considerations (Legal Tech)
- Never log sensitive client data
- Implement proper access controls
- Encrypt data at rest and in transit
- Follow legal industry compliance requirements

## Git Workflow
- Feature branches: `feature/description`
- Bug fixes: `fix/description`
- Squash commits before merging
- Descriptive commit messages following Conventional Commits

## Agent-Specific Guidelines
- **Read before writing**: Always read existing files before modifying them
- **Verify changes**: After edits, verify the file still compiles/passes linting
- **Atomic commits**: Group related changes logically; avoid mixing unrelated fixes
- **Respect existing patterns**: Match existing code style even if different from above
- **Ask when uncertain**: If requirements are ambiguous, ask clarifying questions
- **Test locally**: Run relevant tests before marking a task complete

## Directory Structure (Recommended)
```
legalminds/
├── src/              # Source code
├── tests/            # Test files
├── docs/             # Documentation
├── scripts/          # Utility scripts
└── config/           # Configuration files
```

## Environment Setup
- Use virtual environments (Python) or nvm (Node.js)
- Document setup steps in README.md
- Include `.env.example` for required environment variables
- Pin dependency versions for reproducibility

---
*Last updated: 2026-03-14*
