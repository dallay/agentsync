# AGENTS.md

## Build, Lint, and Test Commands

### Build
To compile the project, use the following commands depending on the language/layer:

#### Rust Component
```bash
cargo build --release
```
The release build artifact will be located at `./target/release/agentsync`.

#### TypeScript Component
```bash
pnpm run build
```
This generates transpiled JavaScript output in the `lib` directory.

### Linting

#### Rust
Ensure proper formatting and linting with:
```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
```

#### TypeScript
Type checking and linting with:
```bash
pnpm run typecheck
```

### Testing

#### Rust Tests
Run the tests for all targets:
```bash
cargo test --all-features
```
To run a specific test:
```bash
cargo test --all-features my_test_name
```
Replace `my_test_name` with the actual test's name.

#### CI Validations
Ensure you’ve set up CI workflows for checks (`cargo check`, linting, and testing). Refer to `.github/workflows/ci.yml`. Continuous Integration validates Rust formatting and Clippy rules as part of pull requests.

---

## Code Style Guidelines
Maintain consistency to ensure high-quality contributions.

### Formatting
1. **Rust:**
   - Use `cargo fmt` before committing.
2. **TypeScript:**
   - Use Prettier or equivalent tools for consistent formatting.

### Imports
Group imports logically:
- **Rust:** Use groups for `std`, external libraries, and internal modules.
```rust
use std::collection::HashMap; // Standard library
use external_crate::FooBar;    // External crates
use crate::mymodule::MyStruct; // Current module
```

- **TypeScript:**
```typescript
import { spawnSync } from "child_process"; // Built-ins
import { existsSync } from "fs";
import { join } from "path";
```

### Types
- Always use **strong typing** in both Rust and TypeScript. Avoid `any` unless strictly unavoidable.
- For Rust, prefer enums over magic integers and use Result for error handling.
- For TypeScript, use interfaces to type objects explicitly.

### Naming Conventions
**Rust:**
- Use `snake_case` for variables and functions.
- Use `CamelCase` and `PascalCase` for types and structs.
```rust
fn my_function() { }
struct MyStruct { field_name: u32 }
```

**TypeScript:**
- Use ‘camelCase’ for variables and methods.
- Use ‘PascalCase’ for classes and types.
```typescript
const variableName = "value";
class MyClass { doSomething() { } }
```

### Error Handling
1. In Rust, use the `Result` type to handle errors gracefully.
```rust
fn my_function() -> Result<(), Box<dyn Error>> {
    Ok(())
}
```

2. In TypeScript:
- Always reject with Error objects.
```typescript
Promise.reject(new Error("Something failed"));
```

### Commit Guidelines
Follow [Conventional Commits](https://www.conventionalcommits.org/). Example:
```bash
feat: add {specific feature description}
```
Ensure CI passes for any pull requests and include tests and docs for major changes.

---

## Toolchains and CI Integration
- Rust: Stable toolchain required, defined in [ci.yml](.github/workflows/ci.yml).
- Use cache wherever possible (`~/.cargo/registry`, `target`).
- Install agentsync locally for tests involving symlinks:
```bash
curl -LO <binary_url>
chmod +x agentsync
sudo mv agentsync /usr/local/bin/
```

---