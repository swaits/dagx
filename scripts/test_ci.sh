#!/bin/bash
# Test GitHub Actions CI workflows locally using act with podman
#
# Usage:
#   ./scripts/test_ci.sh              # Run ALL jobs (simulate GitHub CI)
#   ./scripts/test_ci.sh -j lint      # Run only lint job
#   ./scripts/test_ci.sh -j examples  # Run only examples job
#   ./scripts/test_ci.sh -n           # Dry run all jobs
#   ./scripts/test_ci.sh -j lint -n   # Dry run specific job

set -e

# Ensure podman socket is running
if ! systemctl --user is-active podman.socket &>/dev/null; then
  echo "Starting podman socket..."
  systemctl --user start podman.socket
fi

# Set up podman as docker replacement
export DOCKER_HOST=unix:///run/user/$(id -u)/podman/podman.sock

# Path to act (adjust if needed)
ACT=act

# Parse arguments
DRY_RUN=""
JOB_ARGS=""

while [[ $# -gt 0 ]]; do
  case $1 in
  -n | --dry-run)
    DRY_RUN="-n"
    shift
    ;;
  -j)
    JOB_ARGS="-j $2"
    shift 2
    ;;
  *)
    echo "Unknown option: $1"
    echo "Usage: $0 [-j job_name] [-n|--dry-run]"
    exit 1
    ;;
  esac
done

# Run act (default: run all jobs, just like GitHub)
echo "Running GitHub Actions locally with podman..."
if [ -n "$JOB_ARGS" ]; then
  echo "Job: ${JOB_ARGS#-j }"
fi
if [ -n "$DRY_RUN" ]; then
  echo "Mode: Dry run"
else
  echo "Mode: Full run (simulating GitHub CI)"
fi
echo ""

$ACT $JOB_ARGS $DRY_RUN
