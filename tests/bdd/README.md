# SmartAssist BDD Tests

This crate holds behavior-driven development (BDD) tests using `cucumber-rust`.

## Structure

- `features/` — Gherkin `.feature` files organized by domain.
- `tests/cucumber.rs` — Main Cucumber runner and step definitions.

## Running

```bash
# Run all BDD scenarios
cargo test --test bdd -p smartassist-bdd-tests

# Run a specific feature file
cargo test --test bdd -p smartassist-bdd-tests -- features/gateway/health.feature
```

## Adding Scenarios

1. Create a `.feature` file under `features/<domain>/`.
2. Add step definitions in `tests/cucumber.rs` or `tests/steps/<domain>_steps.rs`.
3. Run `cargo test --test bdd` to verify.
