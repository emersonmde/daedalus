# Protocol Diagram Verification Workflow

**Purpose**: Verify that protocol diagrams in documentation are technically equivalent to RFC/standard specifications.

**Success Criteria**: Field names, bit positions, and bit widths match the authoritative source.

**When to Use**:
- Adding a new protocol diagram to documentation
- User requests verification of an existing diagram
- Updating a diagram after specification changes

---

## Step 1: Locate Authoritative Source

**Input**: Protocol name (e.g., "ARP", "Ethernet II", "TCP")

**Actions**:
1. Identify the authoritative specification:
   - IETF protocols → RFC (e.g., ARP = RFC 826)
   - IEEE protocols → IEEE standard (e.g., Ethernet = IEEE 802.3)
   - Vendor protocols → Official datasheet

2. Use WebFetch to retrieve the specification:
   ```
   WebFetch(url="https://datatracker.ietf.org/doc/html/rfcXXXX",
            prompt="Find and extract the packet format specification showing field names, bit positions, and bit widths")
   ```

3. If RFC doesn't have ASCII diagram, search for official sources:
   - Wikipedia (cross-reference with RFC text)
   - Authoritative textbooks (e.g., Stevens' TCP/IP Illustrated)
   - Reference implementations (Linux kernel comments)

**Output**: Field specification from authoritative source

**Example Output**:
```
RFC 826 - ARP Packet Format:
- Hardware Type: 16 bits (offset 0)
- Protocol Type: 16 bits (offset 16)
- HW Address Length: 8 bits (offset 32)
- Protocol Address Length: 8 bits (offset 40)
- Operation: 16 bits (offset 48)
- Sender Hardware Address: 48 bits (offset 64)
- Sender Protocol Address: 32 bits (offset 112)
- Target Hardware Address: 48 bits (offset 144)
- Target Protocol Address: 32 bits (offset 192)
Total: 224 bits (28 bytes)
```

---

## Step 2: Extract Field Specification from Documentation

**Input**: Path to documentation file containing the diagram

**Actions**:
1. Read the documentation file
2. Locate the protocol diagram in the file
3. Extract field specification by reading the diagram:
   - Field name (from diagram labels)
   - Bit width (count bits or parse field size annotations)
   - Bit offset (calculate from previous fields)

4. Create a structured field list with:
   - Field name
   - Bit offset (starting position)
   - Bit width (field size)
   - Cumulative position check

**Output**: Field specification from documentation diagram

**Example Output**:
```
docs/src/architecture/networking.md - ARP Packet:
Row 1 (bits 0-31):
  - Hardware Type (2): 16 bits at offset 0
  - Protocol Type (2): 16 bits at offset 16
Row 2 (bits 32-63):
  - HW Addr Len: 8 bits at offset 32
  - Proto Addr Len: 8 bits at offset 40
  - Operation (2): 16 bits at offset 48
Row 3-4 (bits 64-111):
  - Sender Hardware Address: 48 bits at offset 64
Row 4-5 (bits 112-143):
  - Sender Protocol Address: 32 bits at offset 112
Row 5-6 (bits 144-191):
  - Target Hardware Address: 48 bits at offset 144
Row 7 (bits 192-223):
  - Target Protocol Address: 32 bits at offset 192
Total: 224 bits (28 bytes)
```

---

## Step 3: Compare Field Specifications

**Input**:
- Field spec from RFC (Step 1)
- Field spec from docs (Step 2)

**Actions**:
1. Compare field count:
   ```
   IF rfc_field_count ≠ doc_field_count THEN FAIL
   ```

2. For each field (in order):
   ```
   Compare:
   - Field name (semantic match, allow abbreviations)
   - Bit offset (MUST match exactly)
   - Bit width (MUST match exactly)
   ```

3. Check total size:
   ```
   IF rfc_total_bits ≠ doc_total_bits THEN FAIL
   ```

4. Generate comparison report

**Output**: PASS or FAIL with detailed differences

**Example Output (PASS)**:
```
✓ Field count matches: 9 fields
✓ Hardware Type: 16 bits at offset 0 ✓
✓ Protocol Type: 16 bits at offset 16 ✓
✓ HW Addr Len: 8 bits at offset 32 ✓
✓ Proto Addr Len: 8 bits at offset 40 ✓
✓ Operation: 16 bits at offset 48 ✓
✓ Sender HW Addr: 48 bits at offset 64 ✓
✓ Sender Proto Addr: 32 bits at offset 112 ✓
✓ Target HW Addr: 48 bits at offset 144 ✓
✓ Target Proto Addr: 32 bits at offset 192 ✓
✓ Total size matches: 224 bits (28 bytes)

VERIFICATION: PASS
```

**Example Output (FAIL)**:
```
✗ Field count matches: 9 fields
✓ Hardware Type: 16 bits at offset 0 ✓
✓ Protocol Type: 16 bits at offset 16 ✓
✗ HW Addr Len: 8 bits at offset 32 (RFC) vs 16 bits at offset 32 (docs)
  ^ BIT WIDTH MISMATCH

VERIFICATION: FAIL
Reason: Bit width mismatch in field 3 (HW Addr Len)
```

---

## Step 4: Cross-Verify with Protocol Tool

**Purpose**: Additional verification using the `protocol` tool as a canonical generator.

**Input**: Field specification from RFC (Step 1)

**Actions**:
1. Convert RFC field spec to protocol tool format:
   ```
   "Field1:bits,Field2:bits,..."
   ```

2. Generate canonical diagram:
   ```bash
   ./scripts/protocol.sh "Hardware Type:16,Protocol Type:16,..."
   ```

3. Visually inspect that bit positions match (automated parsing optional)

**Output**: Protocol tool diagram for reference

**Note**: This is a secondary check. The primary verification is Step 3 (field-by-field comparison).

---

## Step 5: Document Verification

**Input**: Verification result from Step 3

**Actions**:
If verification PASSED:

1. Add source citation after the diagram:
   ```markdown
   **Source**: RFC 826 - Address Resolution Protocol
   **Verified**: [DATE] - Field specifications match RFC 826 Section 3
   **Total Size**: 28 bytes (224 bits)
   ```

2. Optionally add protocol tool command for future re-verification:
   ```markdown
   **Re-verify**: `./scripts/protocol.sh "Hardware Type:16,..."`
   ```

If verification FAILED:

1. Document the discrepancy
2. Determine correct specification (RFC is authoritative)
3. Update diagram to match RFC
4. Re-run verification workflow

**Output**: Updated documentation with verification metadata

---

## Workflow Execution Example

**User request**: "Verify the ARP diagram in networking.md"

**Agent execution**:
```
Step 1: Locate RFC 826
  - WebFetch: https://datatracker.ietf.org/doc/html/rfc826
  - Extract field spec from RFC
  → RFC spec: 9 fields, 224 bits total

Step 2: Extract from docs/src/architecture/networking.md
  - Read lines 398-421
  - Parse diagram structure
  → Doc spec: 9 fields, 224 bits total

Step 3: Compare specifications
  - Field count: 9 = 9 ✓
  - Field 1: Hardware Type (16 bits @ 0) ✓
  - Field 2: Protocol Type (16 bits @ 16) ✓
  - ... [all fields match]
  - Total: 224 bits ✓
  → PASS

Step 4: Cross-verify with protocol tool
  - Generate: ./scripts/protocol.sh "Hardware Type:16,..."
  - Bit positions align ✓

Step 5: Document verification
  - Add source citation to docs
  - Add verification date
  → Complete

Report to user: "✓ ARP diagram verified against RFC 826. All fields match."
```

---

## Common Issues and Solutions

### Issue: RFC has no ASCII diagram

**Solution**:
1. Check RFC text for field descriptions
2. Verify against reference implementation (Linux kernel)
3. Cross-reference with authoritative textbook

### Issue: Field names differ slightly

**Solution**: Allow semantic equivalence:
- "HW Addr Len" = "Hardware Address Length" ✓
- "Sender MAC" = "Sender Hardware Address" ✓
- "Dest IP" ≠ "Source IP" ✗ (different fields)

### Issue: Diagram uses different row width

**Solution**: Row width doesn't matter. Only verify:
- Bit offsets (starting position)
- Bit widths (field size)
- Field order

### Issue: Diagram includes padding/reserved fields, RFC doesn't explicitly show them

**Solution**:
1. Calculate if padding is necessary (fields must align)
2. Verify total size matches RFC
3. Document padding as implementation detail

---

## Automation Potential

**Current**: Semi-automated (AI follows workflow, human reviews)

**Future**: Fully automated if:
1. Diagram parser extracts fields reliably
2. RFC fetcher returns structured data
3. Comparison logic implemented in script

**Recommended**: Keep human-in-the-loop for ambiguous cases (field name variations, implicit padding, etc.)

---

## Success Criteria Checklist

Before marking verification complete:

- [ ] RFC/standard source identified and fetched
- [ ] Field specification extracted from source
- [ ] Field specification extracted from documentation
- [ ] Field-by-field comparison performed
- [ ] All bit offsets match
- [ ] All bit widths match
- [ ] Total size matches
- [ ] Source citation added to documentation
- [ ] Verification date documented
- [ ] Protocol tool command added (optional)

---

## Notes

- **This workflow verifies technical correctness, not visual style**
- Diagrams can look different visually but be technically equivalent
- Focus on data: field names, bit positions, bit widths
- RFC/IEEE standards are always authoritative over other sources
