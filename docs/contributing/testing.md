# Testing Guide

This document outlines the testing strategies for the SynapticFlow project.

## 1. Unit Tests

- **Purpose**: To test individual functions and components in isolation.
- **Location**: Unit tests should be located alongside the code they are testing.
- **Frameworks**: Use standard testing frameworks for Rust and TypeScript (e.g., `jest`, `vitest`).

## 2. Integration Tests

- **Purpose**: To test the interaction between different parts of the application, such as the frontend and backend.
- **Location**: Integration tests should be located in a separate `tests/` directory.
- **Scenarios**: Test scenarios should cover common user workflows, such as sending a message, calling a tool, and adding a new server.

## 3. End-to-End (E2E) Tests

- **Purpose**: To test the application as a whole, from the user's perspective.
- **Frameworks**: Use a framework like Playwright or Cypress to automate E2E tests.
- **Coverage**: E2E tests should cover the most critical application features.
