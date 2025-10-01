#!/bin/bash

set -e

# Find all example files
examples=$(find examples -name "*.rs" -type f 2>/dev/null | sed 's|examples/||; s|\.rs$||' | sort)

if [ -z "$examples" ]; then
    echo "No examples found in examples/ directory"
    exit 0
fi

failed=0
total=0

echo "Running all examples..."
echo

for example in $examples; do
    total=$((total + 1))
    echo "[$total] Running example: $example"
    
    if cargo run --example "$example" --quiet; then
        echo "✓ $example passed"
    else
        echo "✗ $example failed"
        failed=$((failed + 1))
    fi
    echo
done

echo "================================"
echo "Results: $((total - failed))/$total passed"

if [ $failed -eq 0 ]; then
    echo "All examples passed!"
    exit 0
else
    echo "$failed example(s) failed"
    # Cap at 255 (max exit code)
    [ $failed -gt 255 ] && failed=255
    exit $failed
fi
