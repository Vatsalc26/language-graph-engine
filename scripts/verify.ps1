# Language Graph Engine Verification Script
# Runs the baseline quality gates to verify code formatting, linting, tests, and benches.

Write-Host "=== Running Quality Gates ===" -ForegroundColor Cyan

# 1. Cargo Fmt Check
Write-Host "`n1. Running Code Formatting Check (cargo fmt)..." -ForegroundColor Yellow
cargo fmt --all -- --check
if ($LASTEXITCODE -ne 0) {
    Write-Error "Code formatting check failed! Run 'cargo fmt --all' to fix formatting."
    exit 1
}
Write-Host "Formatting check passed!" -ForegroundColor Green

# 2. Cargo Clippy Lint Check
Write-Host "`n2. Running Linter Check (cargo clippy)..." -ForegroundColor Yellow
cargo clippy --all-targets --all-features -- -D warnings
if ($LASTEXITCODE -ne 0) {
    Write-Error "Clippy warnings/errors found!"
    exit 1
}
Write-Host "Clippy check passed!" -ForegroundColor Green

# 3. Cargo Tests (Unit, Seeding, Content Addressing, Concurrent)
Write-Host "`n3. Running Integration & Unit Tests (cargo test)..." -ForegroundColor Yellow
cargo test --all-targets
if ($LASTEXITCODE -ne 0) {
    Write-Error "Automated tests failed!"
    exit 1
}
Write-Host "All tests passed!" -ForegroundColor Green

# 4. Cargo Doc Tests
Write-Host "`n4. Running Documentation Tests (cargo test --doc)..." -ForegroundColor Yellow
cargo test --doc
if ($LASTEXITCODE -ne 0) {
    Write-Error "Documentation tests failed!"
    exit 1
}
Write-Host "Documentation tests passed!" -ForegroundColor Green

# 5. Compile Benchmarks Check
Write-Host "`n5. Verifying Benchmark Compilation (cargo bench --no-run)..." -ForegroundColor Yellow
cargo bench --no-run
if ($LASTEXITCODE -ne 0) {
    Write-Error "Benchmark compilation check failed!"
    exit 1
}
Write-Host "Benchmark compilation verified!" -ForegroundColor Green

Write-Host "`n=== All Quality Gates Passed Successfully! ===" -ForegroundColor Green
exit 0
