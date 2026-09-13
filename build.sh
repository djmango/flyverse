#!/usr/bin/env bash
# Build helper for this VM: the NixOS default PATH has no C compiler, so point
# the linker at the gcc wrapper that is already in the nix store.
set -euo pipefail
GCC_WRAPPER=$(ls -d /nix/store/*gcc-wrapper-15.*/bin 2>/dev/null | sort | tail -1)
BINUTILS_WRAPPER=$(ls -d /nix/store/*binutils-wrapper-*/bin 2>/dev/null | sort | tail -1)
export PATH="$GCC_WRAPPER:$BINUTILS_WRAPPER:$PATH"
export CARGO_HOME="${CARGO_HOME:-/opt/data/.cargo}"
cd "$(dirname "$0")"
exec cargo "$@"
