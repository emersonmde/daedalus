#!/usr/bin/env python3
"""
Generate accurate ASCII protocol diagrams from JSON specifications.

USAGE:
    # Generate all protocols in scripts/protocols/
    ./scripts/gen-protocol-diagrams.py

    # Generate specific protocol
    ./scripts/gen-protocol-diagrams.py scripts/protocols/arp.json

    # Generate from stdin
    cat myprotocol.json | ./scripts/gen-protocol-diagrams.py -

Based on RFC-style diagrams where each row represents 32 bits (4 bytes).

JSON FORMAT:
{
  "name": "Protocol Name",
  "source": "RFC XXX or IEEE Standard",
  "total_note": "Total size description",
  "fields": [
    {
      "name": "Field Name",
      "bits": 16,
      "description": "Optional description"
    }
  ]
}

NOTES:
- Fields <= 32 bits are positioned at exact bit locations
- Fields > 32 bits get dedicated rows (shown conceptually, not bit-accurate)
- Variable-length fields should use approximate bit count (will span rows)

EXAMPLES:
  See scripts/protocols/ethernet.json and scripts/protocols/arp.json
"""

import json
import sys
from pathlib import Path
from typing import List, Tuple, Dict, Any


def generate_diagram(protocol: Dict[str, Any]) -> str:
    """
    Generate RFC-style ASCII diagram from protocol specification.

    Args:
        protocol: Dict with 'name', 'fields', optional 'total_note' and 'source'

    Returns:
        Complete ASCII diagram as string
    """
    lines = []

    # Title
    if 'name' in protocol:
        title = protocol['name']
        if 'source' in protocol:
            title += f" ({protocol['source']})"
        lines.append(title)
        lines.append("")

    # Bit position header
    lines.append(" 0                   1                   2                   3")
    lines.append(" 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1")

    # Process fields into rows
    fields = protocol['fields']
    bit_pos = 0
    row_fields: List[Tuple[Dict, int, int]] = []
    first_row = True

    for field in fields:
        field_bits = field['bits']
        start_bit = bit_pos % 32

        # Handle fields > 32 bits: give them dedicated row(s)
        if field_bits > 32:
            # Render any pending fields first
            if row_fields:
                lines.extend(_render_row(row_fields, is_first=first_row))
                first_row = False
                row_fields = []

            # Align to next row if not at start
            if start_bit != 0:
                bit_pos = ((bit_pos + 31) // 32) * 32

            # Render this field in dedicated row(s)
            # For >32-bit fields, show them conceptually (full width)
            lines.extend(_render_wide_field(field, is_first=first_row))
            first_row = False

            # Advance bit position by field size
            bit_pos += field_bits
            continue

        # Check if field fits in current row
        if start_bit + field_bits > 32:
            # Doesn't fit, render current row and start new one
            if row_fields:
                lines.extend(_render_row(row_fields, is_first=first_row))
                first_row = False
                row_fields = []

            # Move to next row
            bit_pos = ((bit_pos + 31) // 32) * 32
            start_bit = 0

        # Add field to current row
        end_bit = start_bit + field_bits - 1
        row_fields.append((field, start_bit, end_bit))
        bit_pos += field_bits

        # If row is now complete, render it
        if bit_pos % 32 == 0:
            lines.extend(_render_row(row_fields, is_first=first_row))
            first_row = False
            row_fields = []

    # Render any remaining fields
    if row_fields:
        lines.extend(_render_row(row_fields, is_first=first_row))

    # Closing border
    lines.append("└" + "─" * 63 + "┘")

    # Total note
    if 'total_note' in protocol:
        lines.append("")
        lines.append(protocol['total_note'])

    return '\n'.join(lines)


def _render_wide_field(field: Dict, is_first: bool) -> List[str]:
    """Render a field > 32 bits in dedicated row(s)."""
    lines = []

    # Top border (no internal separators for wide fields)
    if is_first:
        lines.append("┌" + "─" * 63 + "┐")
    else:
        lines.append("├" + "─" * 63 + "┤")

    # Field name line (centered across full width)
    name_line = ['│'] + [' '] * 63 + ['│']
    label = field['name']
    left_pad = (63 - len(label)) // 2
    for i, ch in enumerate(label):
        pos = left_pad + i + 1
        if pos < 64:
            name_line[pos] = ch
    lines.append(''.join(name_line))

    # Description line if present
    if field.get('description'):
        desc_line = ['│'] + [' '] * 63 + ['│']
        desc = field['description']
        left_pad = (63 - len(desc)) // 2
        for i, ch in enumerate(desc):
            pos = left_pad + i + 1
            if pos < 64:
                desc_line[pos] = ch
        lines.append(''.join(desc_line))

    return lines


def _render_row(row_fields: List[Tuple[Dict, int, int]], is_first: bool) -> List[str]:
    """Render one 32-bit row with one or more fields."""
    # Build top border with field separators
    border = [('┌' if is_first else '├')]

    for i in range(63):
        # Check if this position is a field boundary
        is_boundary = False
        for _, _, end in row_fields[:-1]:  # Exclude last field
            if i == end * 2 + 2:  # Position after field end
                is_boundary = True
                break

        border.append('┬' if is_boundary else '─')

    border.append('┐' if is_first else '┤')

    # Build field name line
    name_line = ['│'] + [' '] * 63 + ['│']

    for field, start, end in row_fields:
        char_start = start * 2 + 1
        char_end = end * 2 + 2
        width = char_end - char_start

        # Format field label
        label = field['name']

        # Center label
        pad = width - len(label)
        left_pad = pad // 2

        # Write label
        for i, ch in enumerate(label):
            pos = char_start + left_pad + i
            if pos < char_end and pos < 64:
                name_line[pos] = ch

        # Add field separator
        if end < 31 and char_end < 64:
            name_line[char_end] = '│'

    # Build description line if needed
    has_desc = any(f.get('description') for f, _, _ in row_fields)
    lines = [''.join(border), ''.join(name_line)]

    if has_desc:
        desc_line = ['│'] + [' '] * 63 + ['│']

        for field, start, end in row_fields:
            desc = field.get('description', '')
            if desc:
                char_start = start * 2 + 1
                char_end = end * 2 + 2
                width = char_end - char_start

                pad = width - len(desc)
                left_pad = pad // 2

                for i, ch in enumerate(desc):
                    pos = char_start + left_pad + i
                    if pos < char_end and pos < 64:
                        desc_line[pos] = ch

            # Add separator
            if end < 31:
                sep_pos = end * 2 + 3
                if sep_pos < 64:
                    desc_line[sep_pos] = '│'

        lines.append(''.join(desc_line))

    return lines


def main():
    """Main entry point."""
    if len(sys.argv) > 1:
        # Process specific file(s)
        for filepath in sys.argv[1:]:
            if filepath == '-':
                # Read from stdin
                protocol = json.load(sys.stdin)
                print(generate_diagram(protocol))
            else:
                # Read from file
                with open(filepath) as f:
                    protocol = json.load(f)
                print(generate_diagram(protocol))
                print()
    else:
        # Process all protocols in scripts/protocols/
        protocols_dir = Path(__file__).parent / 'protocols'

        if not protocols_dir.exists():
            print(f"Error: {protocols_dir} does not exist", file=sys.stderr)
            print("Create protocol JSON files in scripts/protocols/", file=sys.stderr)
            sys.exit(1)

        protocol_files = sorted(protocols_dir.glob('*.json'))

        if not protocol_files:
            print(f"Error: No .json files found in {protocols_dir}", file=sys.stderr)
            sys.exit(1)

        for i, filepath in enumerate(protocol_files):
            with open(filepath) as f:
                protocol = json.load(f)

            print("=" * 70)
            print(f"{protocol.get('name', filepath.stem).upper()}")
            print("=" * 70)
            print(generate_diagram(protocol))

            if i < len(protocol_files) - 1:
                print("\n")


if __name__ == "__main__":
    main()
