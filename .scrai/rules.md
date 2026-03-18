## Rules

### Validation Commands

Run the relevant checks below after every code change:

```bash
# Check compilation
cd engine && cargo check

# Run tests (single crate)
cd engine && cargo test -p flapjack --lib

# Run tests (specific test)
cd engine && cargo test -p flapjack --lib test_name

# Run clippy
cd engine && cargo clippy --workspace -- -D warnings

# Format check
cd engine && cargo fmt --check
```

### Permissions
- **Allowed without asking**: read files, cargo check, cargo clippy, cargo fmt --check, run single test files
- **Ask first**: cargo add (new dependencies), git push, deleting files, full test suite (`cargo test --workspace`)

### Never
- Run `cargo clean` — rebuilds take too long
- Break Algolia API compatibility without discussion
- Add `unsafe` blocks without justification in comments
