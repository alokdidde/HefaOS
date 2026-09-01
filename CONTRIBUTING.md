# Contributing to Hefaos

Thank you for your interest in contributing to Hefaos! This document provides guidelines and information for contributors.

## Code of Conduct

Be respectful and constructive in all interactions. We're all here to build great robotics software.

## Getting Started

1. Fork the repository
2. Clone your fork: `git clone https://github.com/YOUR_USERNAME/hefaos.git`
3. Set up development environment (see README.md)
4. Create a feature branch: `git checkout -b feature/your-feature-name`

## Development Workflow

### Branch Naming

- `feature/` - New features
- `fix/` - Bug fixes
- `docs/` - Documentation updates
- `refactor/` - Code refactoring
- `test/` - Test additions or modifications

### Commit Messages

Use conventional commits format:

```
type(scope): description

[optional body]

[optional footer]
```

Types: `feat`, `fix`, `docs`, `style`, `refactor`, `test`, `chore`

Examples:
```
feat(sdk): add support for custom component schemas
fix(runtime): resolve race condition in state store
docs(api): update IMU component documentation
```

### Code Style

**C++ (runtime/)**
- Follow the existing code style
- Use `clang-format` before committing
- Run `clang-tidy` for static analysis
- Prefer modern C++20 features

**TypeScript (sdk/)**
- ESLint and Prettier are configured
- Run `pnpm lint` before committing
- Use TypeScript strict mode

### Testing

All contributions must include appropriate tests:

**C++ Tests**
```bash
cd runtime
cmake -B build -DBUILD_TESTING=ON
cmake --build build
ctest --test-dir build --output-on-failure
```

**TypeScript Tests**
```bash
cd sdk
pnpm test
```

## Pull Request Process

1. Ensure all tests pass
2. Update documentation if needed
3. Add entry to CHANGELOG.md under "Unreleased"
4. Request review from maintainers
5. Address feedback and iterate

### PR Checklist

- [ ] Tests added/updated
- [ ] Documentation updated
- [ ] CHANGELOG.md updated
- [ ] Code formatted (`clang-format` / `prettier`)
- [ ] No new warnings
- [ ] CI passes

## Architecture Guidelines

### Runtime (C++)

- Prefer stack allocation over heap
- Use RAII for resource management
- Avoid dynamic allocation in hot paths
- Document thread safety guarantees
- Target < 1ms latency for control loops

### SDK (TypeScript)

- Keep the API surface minimal
- Provide type-safe interfaces
- Document all public APIs
- Support tree-shaking

## Questions?

- Open an issue for bugs or feature requests
- Start a discussion for questions or ideas

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
