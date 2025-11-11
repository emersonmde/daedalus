#!/bin/bash
# Protocol Diagram Generator
#
# Wrapper for the protocol tool (git submodule in scripts/protocol/)
#
# Usage:
#   ./scripts/protocol.sh tcp
#   ./scripts/protocol.sh "Hardware Type:16,Protocol Type:16,..."
#
# Install:
#   git submodule update --init scripts/protocol
#
# See: https://github.com/luismartingarcia/protocol

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROTOCOL_TOOL="$SCRIPT_DIR/protocol/protocol"

if [ ! -f "$PROTOCOL_TOOL" ]; then
    echo "Error: protocol tool not found at $PROTOCOL_TOOL"
    echo ""
    echo "Initialize the submodule with:"
    echo "  git submodule update --init scripts/protocol"
    exit 1
fi

exec python3 "$PROTOCOL_TOOL" "$@"
