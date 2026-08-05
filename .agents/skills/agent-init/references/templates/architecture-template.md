# 🏗️ Architecture & Key File Locations (`architecture-and-files.md`)

> **Note for AI Agents**: Read this guide when planning code changes, locating files, understanding module boundaries, or analyzing dependencies.

---

## 🧭 System Architecture Overview

Brief description of system components, layer separation, and key runtime flows:

```
[ Frontend / UI Layer ] ──> [ Service / API Layer ] ──> [ Backend / Engine Layer ]
```

- **Frontend**: `{frontend_framework}`
- **Backend / Core Engine**: `{backend_framework}`
- **Data Persistence**: `{database_or_storage}`
- **Communication Protocol**: `{ipc_or_api_protocol}`

---

## 📁 Key Directory & File Map

```
{project_root}/
├── src/ / app/               # Main source code
├── src-tauri/ / backend/      # Core engine / backend
├── docs/                      # Documentation & guidelines
│   └── guidelines/            # Modular guidelines
├── tests/                     # Test suites
└── package.json / Cargo.toml  # Project configuration
```

### Key File Locations

| File / Directory Path | Purpose / Description | Key Exports / Responsibilities |
| --------------------- | --------------------- | ------------------------------ |
| `{key_path_1}`        | `{purpose_1}`         | `{responsibility_1}`           |
| `{key_path_2}`        | `{purpose_2}`         | `{responsibility_2}`           |
| `{key_path_3}`        | `{purpose_3}`         | `{responsibility_3}`           |

---

## 🔗 Module Dependencies & Layering Rules

1. **Dependency Direction**: `{dependency_direction_rule}`
2. **Layer Isolation**: `{layer_isolation_rule}`
3. **Data Access**: `{data_access_rule}`
