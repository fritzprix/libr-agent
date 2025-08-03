# Coding Standards

This document outlines the coding standards and conventions for the SynapticFlow project.

## General

-   **Indentation**: Use 2 spaces for indentation in all files.
-   **Naming**: Use descriptive names for variables, functions, and classes.

## Rust Backend (`src-tauri/`)

-   **Style Guide**: Follow the official [Rust Style Guide](https://doc.rust-lang.org/1.0.0/style/).
-   **Formatting**: Use `cargo fmt` to format Rust code.
-   **Linting**: Use `cargo clippy` to identify common mistakes and improve code quality.
-   **Case**: Use `snake_case` for functions and variables, and `PascalCase` for types.

## TypeScript/React Frontend (`src/`)

-   **Style Guide**: Follow the configured Prettier and ESLint rules.
-   **Formatting**: Use `pnpm format` to format TypeScript and React code.
-   **Linting**: Use `pnpm lint` to check for code quality and style issues.
-   **Case**: Use `camelCase` for functions and variables, and `PascalCase` for components and interfaces.
-   **Types**: Avoid using `any`. Use specific types and interfaces whenever possible.
