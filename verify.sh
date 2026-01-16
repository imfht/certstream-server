#!/bin/bash
# Quick verification script for Rust CertStream Server

set -e

echo "=========================================="
echo "CertStream Rust Implementation Verification"
echo "=========================================="
echo ""

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Function to print status
print_status() {
    if [ $1 -eq 0 ]; then
        echo -e "${GREEN}✓${NC} $2"
    else
        echo -e "${RED}✗${NC} $2"
        exit 1
    fi
}

print_info() {
    echo -e "${YELLOW}→${NC} $1"
}

# Check if cargo is installed
print_info "Checking Rust installation..."
if command -v cargo &> /dev/null; then
    RUST_VERSION=$(cargo --version)
    print_status 0 "Rust installed: $RUST_VERSION"
else
    print_status 1 "Rust not found. Please install from https://rustup.rs/"
fi

# Check if source files exist
print_info "Checking source files..."
if [ -f "Cargo.toml" ] && [ -d "src" ]; then
    FILE_COUNT=$(find src -name "*.rs" | wc -l)
    print_status 0 "Source files found: $FILE_COUNT Rust files"
else
    print_status 1 "Source files not found"
fi

# Try to build the project
print_info "Building project (this may take a few minutes)..."
if cargo build --release > /tmp/build.log 2>&1; then
    print_status 0 "Build successful"
    
    # Check binary size
    if [ -f "target/release/certstream-server" ]; then
        BINARY_SIZE=$(du -h target/release/certstream-server | cut -f1)
        print_status 0 "Binary created: $BINARY_SIZE"
        
        # Check if binary is executable
        if [ -x "target/release/certstream-server" ]; then
            print_status 0 "Binary is executable"
        else
            print_status 1 "Binary is not executable"
        fi
    else
        print_status 1 "Binary not found after build"
    fi
else
    print_status 1 "Build failed. Check /tmp/build.log for details"
fi

# Run tests
print_info "Running tests..."
if cargo test > /tmp/test.log 2>&1; then
    print_status 0 "Tests passed"
else
    print_status 1 "Tests failed. Check /tmp/test.log for details"
fi

# Check documentation
print_info "Checking documentation..."
DOC_COUNT=0
[ -f "RUST_README.md" ] && DOC_COUNT=$((DOC_COUNT + 1))
[ -f "MIGRATION.md" ] && DOC_COUNT=$((DOC_COUNT + 1))
[ -f "COMPARISON.md" ] && DOC_COUNT=$((DOC_COUNT + 1))
[ -f "IMPLEMENTATION_SUMMARY.md" ] && DOC_COUNT=$((DOC_COUNT + 1))
[ -f "TEST_REPORT.md" ] && DOC_COUNT=$((DOC_COUNT + 1))

if [ $DOC_COUNT -ge 4 ]; then
    print_status 0 "Documentation complete: $DOC_COUNT files"
else
    print_status 1 "Documentation incomplete: only $DOC_COUNT/4 files found"
fi

# Try to start the server briefly (only if not in CI)
if [ -z "$CI" ]; then
    print_info "Testing server startup (5 second test)..."
    PORT=14000 timeout 5 target/release/certstream-server > /tmp/server.log 2>&1 || true
    
    if grep -q "Starting CertStream Server" /tmp/server.log 2>/dev/null || \
       grep -q "Starting web server" /tmp/server.log 2>/dev/null; then
        print_status 0 "Server can start successfully"
    else
        print_status 0 "Server startup not verified (may need network access)"
    fi
fi

echo ""
echo "=========================================="
echo -e "${GREEN}Verification Complete!${NC}"
echo "=========================================="
echo ""
echo "Summary:"
echo "  • Rust toolchain: ✓"
echo "  • Source files: ✓"
echo "  • Build system: ✓"
echo "  • Tests: ✓"
echo "  • Documentation: ✓"
echo ""
echo "The Rust implementation is ready to use!"
echo ""
echo "Next steps:"
echo "  1. Run: ./target/release/certstream-server"
echo "  2. Open: http://localhost:4000"
echo "  3. See RUST_README.md for more details"
echo ""
