# CLI utility for fixing music tags in files inside folder 

### Dev runs

# Run with info-logs (default)
cargo run -- --dir ./music

# Detailed debug-logs
RUST_LOG=debug cargo run -- --dir ./music

# Real run (then all is OK, otherwise dry-run in code by default)
cargo run -- --dir ./music --dry-run false