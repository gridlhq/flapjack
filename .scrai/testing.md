## Testing

### Structure
- `engine/tests/` — integration tests
- `engine/src/integ_tests/` — in-crate integration tests
- Unit tests live alongside source in `#[cfg(test)]` modules

### Quick-Reference Commands
```bash
# Run all lib tests (fast)
cd engine && cargo test -p flapjack --lib

# Run a specific test
cd engine && cargo test -p flapjack --lib test_name

# Run integration tests
cd engine && cargo test -p flapjack --test '*'

# Run tests for a subcrate
cd engine && cargo test -p flapjack-server

# Full workspace (ask first)
cd engine && cargo test --workspace
```
