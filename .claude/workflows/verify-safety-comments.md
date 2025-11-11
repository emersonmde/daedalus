# Safety Comment Verification Workflow

**Purpose**: Meticulously verify every `// SAFETY:` comment in the codebase to ensure invariants are correct, logic is sound, and assumptions are validated against authoritative external sources.

**Success Criteria**: All safety invariants are verified against official documentation (datasheets, ARM manuals, RFCs) or high-quality sources (authoritative blog posts, reference implementations).

**When to Use**:
- Before considering a milestone complete
- After adding new `unsafe` code
- User requests safety audit
- Periodic codebase audits

---

## Step 1: Inventory All Unsafe Blocks and Verify Comment Presence

**Input**: Entire codebase (`src/` directory)

**Actions**:

### 1.1: Find All Unsafe Keywords

Find all occurrences of `unsafe` keyword:
```
Grep(pattern="unsafe", output_mode="content", -B=3, -C=2)
```

This captures:
- `unsafe fn` declarations
- `unsafe { ... }` blocks
- `unsafe impl` blocks
- `unsafe` trait declarations

### 1.2: Verify Safety Comment Presence

**Per Rust std library convention**: Every `unsafe` block/function MUST have a preceding `// SAFETY:` comment explaining why the code is safe.

**Reference**: [Rust std library SAFETY comment guidelines](https://std-dev-guide.rust-lang.org/code-considerations/safety-comments.html)

For each `unsafe` occurrence, check:

1. **Comment exists**: There MUST be a `// SAFETY:` comment immediately before the `unsafe` keyword
2. **Comment format**: Must use `// SAFETY:` (not `/* SAFETY: */` or other variants)
3. **Comment placement**: Must be on the line(s) directly preceding `unsafe` (no blank lines between)

**Example - CORRECT**:
```rust
// SAFETY: Pointer is non-null and properly aligned. Region is valid MMIO.
unsafe { ptr::write_volatile(addr, value) }
```

**Example - INCORRECT** (missing comment):
```rust
unsafe { ptr::write_volatile(addr, value) }  // ❌ MISSING SAFETY COMMENT
```

**Example - INCORRECT** (wrong format):
```rust
/* SAFETY: This is safe because... */  // ❌ WRONG FORMAT (use // not /* */)
unsafe { ... }
```

**Example - INCORRECT** (separated by blank line):
```rust
// SAFETY: This is safe because...
                                    // ❌ BLANK LINE - BREAKS ASSOCIATION
unsafe { ... }
```

**Action on violation**:
- Mark as **CRITICAL FAILURE**
- Report to user with file path and line number
- Require fix before continuing verification

### 1.3: Extract Context for Each Unsafe Block

For each unsafe block with a valid safety comment:
- Read the file
- Extract the preceding `// SAFETY:` comment
- Extract surrounding context (function name, purpose)
- Identify the unsafe operation(s) being performed

### 1.4: Create Inventory

Create inventory with:
- File path and line number
- Function/context name
- Safety comment text (full, multi-line if applicable)
- Unsafe operation(s) being performed
- **Compliance status**: ✓ Has safety comment / ❌ Missing safety comment

**Output**: Structured inventory of all unsafe blocks + list of violations

**Example Output**:
```
Unsafe Block Inventory (12 total):
✓ Compliant: 11
❌ Missing SAFETY comment: 1

COMPLIANT BLOCKS:

1. src/drivers/uart.rs:67 ✓
   Context: Uart::new() - Creating UART driver instance
   Operation: Dereferencing raw MMIO pointer (0xFE201000)
   Safety Comment:
     "UART base address (0xFE201000) is valid per BCM2711 ARM Peripherals
      datasheet Section 2.1. Pointer is aligned, non-null, and points to
      valid MMIO region. No aliasing: only one Uart instance exists."

2. src/arch/aarch64/mmu.rs:145 ✓
   Context: enable_mmu() - Enabling memory management unit
   Operation: Writing to system register SCTLR_EL1
   Safety Comment:
     "MMU configured correctly per ARM Cortex-A72 TRM Section 5.2.
      Translation tables valid, MAIR configured, TCR values match TRM
      requirements. CPU in EL1 with proper privileges."

[... continue for all compliant blocks ...]

VIOLATIONS:

❌ src/drivers/experimental.rs:42
   Context: read_register()
   Operation: Reading from raw pointer
   Issue: MISSING SAFETY COMMENT
   Required Action: Add // SAFETY: comment explaining why pointer dereference is safe
```

---

## Step 2: Categorize Safety Invariants

**Input**: Inventory from Step 1

**Actions**:
For each unsafe block, categorize the claimed invariants:

**Categories**:
1. **Hardware Addresses** - MMIO pointer validity
   - Verify: Datasheet/TRM address matches exactly
   - Verify: Address is within documented MMIO region
   - Verify: Alignment requirements met

2. **Memory Layout** - Stack, heap, code sections
   - Verify: Linker script defines the region correctly
   - Verify: Size calculations are correct
   - Verify: No overlapping regions

3. **Register Operations** - System/control register access
   - Verify: Register exists and is accessible at current EL
   - Verify: Bit field values are valid per ARM spec
   - Verify: Ordering constraints satisfied (barriers, etc.)

4. **Alignment** - Pointer and data alignment
   - Verify: ARM AAPCS or hardware alignment requirement
   - Verify: Calculation preserves alignment

5. **Lifetime/Aliasing** - Borrow checker assumptions
   - Verify: No aliasing (static mut, global state)
   - Verify: Lifetime outlives usage
   - Verify: Initialization before access

6. **Volatile Operations** - MMIO access semantics
   - Verify: Volatile read/write used correctly
   - Verify: No compiler optimization hazards
   - Verify: Memory barriers if needed

7. **Assembly Constraints** - Inline ASM safety
   - Verify: Register clobbers correct
   - Verify: Memory clobbers specified
   - Verify: No undefined behavior in ASM

**Output**: Categorized invariants with verification requirements

**Example Output**:
```
src/drivers/uart.rs:67 - Uart::new()
├─ Category: Hardware Addresses
├─ Invariants:
│  1. Address 0xFE201000 is valid UART base
│  2. Pointer is aligned (4-byte or better)
│  3. Region is MMIO (uncacheable, device memory)
│  4. No aliasing (single instance guarantee)
├─ Verification Required:
│  → BCM2711 datasheet Section 2.1 (address map)
│  → PL011 TRM (alignment requirements)
│  → Code inspection (singleton pattern)
```

---

## Step 3: Verify Against Authoritative Sources

**Input**: Categorized invariants from Step 2

**Actions**:
For each invariant, verify against the appropriate source.

### 3.1: Hardware Address Verification

**Required Sources** (in priority order):
1. **Official datasheets** (BCM2711 ARM Peripherals, BCM54213PE PHY)
2. **ARM Technical Reference Manuals** (Cortex-A72, GIC-400, PL011)
3. **IEEE/industry standards** (Ethernet, PCI, etc.)

**Verification Steps**:
1. Use WebFetch to retrieve the relevant datasheet section:
   ```
   WebFetch(url="https://datasheets.raspberrypi.com/bcm2711/bcm2711-peripherals.pdf",
            prompt="Find the memory map showing the base address for [PERIPHERAL].
                    Extract the exact address, size, and any alignment requirements.")
   ```

2. Extract exact address from source
3. Compare with code constant
4. Check alignment requirement (if specified)
5. Verify register offsets within peripheral

**Example**:
```
Verifying: UART_BASE = 0xFE201000

Source: BCM2711 ARM Peripherals, Section 2.1 "Address Map"
Fetched: https://datasheets.raspberrypi.com/bcm2711/bcm2711-peripherals.pdf
Result:
  ✓ Base address: 0xFE201000 (matches exactly)
  ✓ ARM physical mapping: 0xFE000000 + 0x201000
  ✓ Size: 4KB region
  ✓ Alignment: Implicit 4-byte (register width)

Status: VERIFIED
```

### 3.2: Register Operation Verification

**Required Sources**:
1. **ARM Architecture Reference Manual** (system register definitions)
2. **Cortex-A72 TRM** (core-specific registers)
3. **GIC-400 TRM** (interrupt controller registers)

**Verification Steps**:
1. Identify the register being accessed (e.g., SCTLR_EL1, MAIR_EL1)
2. Fetch register definition from ARM documentation:
   ```
   WebFetch(url="https://developer.arm.com/documentation/ddi0487/latest/",
            prompt="Find the definition of [REGISTER]. Extract:
                    - Which exception levels can access it
                    - Bit field definitions
                    - Reserved bits and required values
                    - Reset values")
   ```

3. Verify:
   - Current EL allows access
   - Bit field values are within valid ranges
   - Reserved bits set correctly (0 or 1 as required)
   - Dependencies satisfied (prerequisite register settings)

**Example**:
```
Verifying: SCTLR_EL1.M = 1 (MMU enable bit)

Source: ARM ARM, Section D13.2.118 "SCTLR_EL1, System Control Register"
Fetched: https://developer.arm.com/documentation/ddi0595/latest/
Result:
  ✓ Accessible from: EL1, EL2, EL3 (code runs at EL1)
  ✓ Bit 0 (M): 0 = MMU disabled, 1 = MMU enabled
  ✓ Prerequisites:
    - TTBR0_EL1/TTBR1_EL1 configured ✓
    - TCR_EL1 configured ✓
    - MAIR_EL1 configured ✓
  ✓ Reserved bits: [RES0 at bits 63:38, others checked]

Status: VERIFIED
```

### 3.3: Memory Layout Verification

**Required Sources**:
1. **Linker script** (`link.ld`)
2. **ARM AAPCS** (alignment requirements)
3. **Raspberry Pi firmware documentation** (load addresses)

**Verification Steps**:
1. Read linker script
2. Extract section definitions and symbols
3. Verify calculations in code match linker symbols:
   ```rust
   let heap_start = __heap_start as usize;  // Verify symbol exists
   let heap_size = __heap_end - __heap_start;  // Verify calculation
   ```

4. Check for overlaps:
   ```
   Sections must not overlap:
   - .text (code)
   - .rodata (read-only data)
   - .data (initialized data)
   - .bss (zero-initialized)
   - stack
   - heap
   ```

5. Verify alignment constraints from ARM AAPCS

**Example**:
```
Verifying: Stack pointer alignment (16-byte)

Source: ARM AAPCS64, Section 6.2.2 "Stack Constraints"
Fetched: https://github.com/ARM-software/abi-aa/blob/main/aapcs64/aapcs64.rst
Result:
  ✓ SP must be 16-byte aligned at public interfaces
  ✓ Code: Stack initialized to __stack_top (linker symbol)
  ✓ Linker script: ALIGN(16) applied to stack section
  ✓ Boot code: AND operation to align: and sp, x1, #~15

Status: VERIFIED
```

### 3.4: Protocol/Format Verification

**Required Sources**:
1. **IETF RFCs** (ARP, IP, TCP, UDP, ICMP)
2. **IEEE Standards** (Ethernet 802.3)
3. **Vendor specifications** (GENET, PHY datasheets)

**Verification Steps**:
1. Fetch protocol specification:
   ```
   WebFetch(url="https://datatracker.ietf.org/doc/html/rfc826",
            prompt="Find the packet structure for [PROTOCOL].
                    Extract field sizes, offsets, and valid values.")
   ```

2. Verify struct layout matches specification:
   - Field sizes (u8, u16, u32, etc.)
   - Field order (byte order matters!)
   - Padding/alignment
   - Endianness handling

3. Check constants against spec:
   - Ethertype values
   - Protocol numbers
   - Flag bit positions

**Example**:
```
Verifying: ARP packet structure

Source: RFC 826, "An Ethernet Address Resolution Protocol"
Fetched: https://datatracker.ietf.org/doc/html/rfc826
Result:
  ✓ Hardware Type (HTYPE): 16 bits at offset 0
    → Code: u16 (big-endian: 0x0001 for Ethernet)
  ✓ Protocol Type (PTYPE): 16 bits at offset 2
    → Code: u16 (big-endian: 0x0800 for IPv4)
  ✓ HW Address Length: 8 bits at offset 4
    → Code: u8 (value: 6 for Ethernet MAC)
  ✓ Protocol Address Length: 8 bits at offset 5
    → Code: u8 (value: 4 for IPv4)
  [... continue for all fields ...]
  ✓ Total size: 28 bytes (matches code struct size)

Status: VERIFIED
```

### 3.5: Singleton/Aliasing Verification

**Required Sources**:
1. **Code inspection** (manual or automated)
2. **Rust reference** (aliasing rules, static mut)

**Verification Steps**:
1. Trace all paths that could create the type
2. Verify static mut access is protected (if applicable)
3. Check for Send/Sync bounds (if multi-core)
4. Verify no multiple mutable references possible

**Example**:
```
Verifying: Only one UART instance exists (no aliasing)

Source: Code inspection + Rust reference
Method:
  1. Search for all Uart::new() calls → Found 1 in main.rs
  2. Check if Uart is Clone → No (no #[derive(Clone)])
  3. Check if passed as reference → Yes (&mut uart)
  4. Verify no interior mutability → Correct (no Cell/RefCell)

Result:
  ✓ Single instance created in main()
  ✓ Passed by mutable reference only
  ✓ No way to duplicate the instance
  ✓ Safe: No aliasing possible

Status: VERIFIED
```

**Output**: Per-invariant verification results

---

## Step 4: Logic Soundness Check

**Input**: Code context around each unsafe block

**Actions**:
Verify the logic leading to the unsafe operation is sound.

### Common Logic Patterns to Check:

**4.1: Null Pointer Checks**
```rust
// SAFETY: Pointer is non-null (checked above)
unsafe { *ptr }
```
Verify: Check actually exists and cannot be bypassed

**4.2: Bounds Checks**
```rust
// SAFETY: Index is within bounds (i < len)
unsafe { slice.get_unchecked(i) }
```
Verify: Index cannot exceed bounds due to prior checks/loop invariants

**4.3: Initialization Checks**
```rust
// SAFETY: Initialized in init() before use
unsafe { GLOBAL.assume_init_ref() }
```
Verify: All code paths initialize before access

**4.4: Alignment Calculations**
```rust
// SAFETY: Address is 16-byte aligned (rounded up)
let aligned = (addr + 15) & !15;
unsafe { read_aligned(aligned as *const u128) }
```
Verify: Math is correct (test with examples)

**Example Verification**:
```
Checking: src/allocator.rs:89 - Heap alignment

Code:
  let heap_start = __heap_start as usize;
  let aligned_start = (heap_start + 15) & !15;  // Round up to 16-byte
  // SAFETY: aligned_start is 16-byte aligned
  unsafe { ... }

Logic Verification:
  Test: heap_start = 0x80000 (aligned)
    → (0x80000 + 15) & !15 = 0x80000 ✓

  Test: heap_start = 0x80001 (misaligned by 1)
    → (0x80001 + 15) & !15 = 0x80010 ✓

  Test: heap_start = 0x8000F (misaligned by 15)
    → (0x8000F + 15) & !15 = 0x80010 ✓

  Test: heap_start = 0x80010 (aligned)
    → (0x80010 + 15) & !15 = 0x80010 ✓

  Formula verified: Always rounds up to next 16-byte boundary

Status: SOUND
```

**Output**: Logic verification for each unsafe block

---

## Step 5: Assumption Validation

**Input**: All assumptions stated in safety comments

**Actions**:
For each assumption, validate it holds in all contexts.

### Common Assumptions to Validate:

**5.1: "Only called once"**
- Search entire codebase for all call sites
- Verify no loops or conditions could cause multiple calls
- Check for indirect calls (function pointers, trait objects)

**5.2: "Runs before X" or "Runs after Y"**
- Trace execution flow from boot/entry point
- Verify ordering is enforced by code structure
- Check for early returns or error paths that could violate order

**5.3: "No other code accesses this"**
- Search for all access to the resource (global, MMIO, etc.)
- Verify mutual exclusion (locks, single-threaded, etc.)
- Consider interrupt handlers

**5.4: "Value is always X"**
- Trace all assignments to the variable
- Check for arithmetic overflow
- Verify external inputs are validated

**Example Validation**:
```
Assumption: "init() called exactly once before any driver use"

Validation:
  1. Find init() call sites:
     Grep(pattern="init\(\)", output_mode="content")
     → Found: main.rs:45 (single call in main)

  2. Verify execution path:
     Entry: boot.s → rust_main → main
     → init() is first function called in main()

  3. Check for early exits:
     → No returns before init()
     → No panic paths before init()

  4. Verify drivers used after:
     → All driver construction after init() ✓

Status: VALIDATED
```

**Output**: Validation results for each assumption

---

## Step 6: Document Findings

**Input**: Verification results from Steps 3-5

**Actions**:

### If All Verifications PASS:

1. Generate verification report:
   ```markdown
   # Safety Verification Report

   **Date**: YYYY-MM-DD
   **Verifier**: Claude Code (Sonnet 4.5)
   **Scope**: All unsafe blocks in codebase

   ## Summary
   - Total unsafe blocks: 12
   - Verified: 12
   - Failed: 0

   ## Verification Details

   ### src/drivers/uart.rs:67 - Uart::new()
   ✓ Address verified: BCM2711 Peripherals, Section 2.1
   ✓ Alignment verified: PL011 TRM (4-byte registers)
   ✓ Singleton verified: Code inspection (single instance)
   ✓ Logic sound: Pointer checks present

   [... repeat for all blocks ...]
   ```

2. Add verification metadata to code (optional):
   ```rust
   // SAFETY: UART base address (0xFE201000) is valid per BCM2711 ARM
   //         Peripherals datasheet Section 2.1. Pointer is aligned (4-byte),
   //         non-null, and points to valid MMIO region.
   //         No aliasing: only one Uart instance exists (see main.rs:67).
   // VERIFIED: 2025-01-15 against BCM2711 datasheet v1.0
   unsafe { &mut *(UART_BASE as *mut UartRegisters) }
   ```

### If Any Verifications FAIL:

1. Document the failure:
   ```markdown
   ## FAILED VERIFICATION

   **Location**: src/drivers/foo.rs:123
   **Issue**: Address mismatch

   Safety Comment Claims:
     "Base address 0xFE300000 per BCM2711 datasheet"

   Actual Value (BCM2711 Peripherals, Section 2.1):
     Foo peripheral base: 0xFE200000

   **Impact**: CRITICAL - Wrong MMIO region access
   **Action Required**:
     1. Update FOO_BASE constant to 0xFE200000
     2. Re-verify all register offsets
     3. Test on hardware
   ```

2. Create actionable TODO list for user:
   ```
   Required Actions:
   □ Fix address constant in src/drivers/foo.rs:15
   □ Update safety comment with correct source
   □ Re-run verification workflow
   □ Test on hardware before deployment
   ```

**Output**: Comprehensive verification report + action items (if needed)

---

## Step 7: Cross-Reference Check

**Input**: All safety comments

**Actions**:
Additional verification passes:

### 7.1: Consistency Check
- Ensure same constant has same value everywhere
- Verify safety comments cite same source for same invariant
- Check for conflicting statements

### 7.2: Completeness Check
- Verify every unsafe block has a safety comment
- Check that safety comment addresses all operations in block
- Ensure all invariants are explicitly stated (not implicit)

### 7.3: Source Currency Check
- Verify datasheet/TRM versions are current
- Flag if using outdated specifications
- Note if newer versions available

**Example**:
```
Consistency Check: UART_BASE constant

Found in:
  1. src/drivers/uart.rs:12 → 0xFE201000 ✓
  2. docs/src/hardware/memory-map.md → 0xFE201000 ✓
  3. docs/src/hardware/uart-pl011.md → 0xFE201000 ✓

Safety comments cite:
  1. "BCM2711 ARM Peripherals Section 2.1" ✓
  2. "BCM2711 datasheet" (same source, less specific) ⚠️

Recommendation: Standardize citation format to include section numbers

Source Currency:
  BCM2711 ARM Peripherals: 2024-10-15 (latest) ✓
```

**Output**: Cross-reference verification results

---

## Workflow Execution Example

**User request**: "Verify all safety comments"

**Agent execution**:
```
Step 1: Inventory unsafe blocks and verify comment presence
  1.1: Find all unsafe keywords
    → Found 12 unsafe occurrences across 11 files

  1.2: Verify safety comment presence
    → All 12 blocks have // SAFETY: comments ✓
    → No violations found ✓

  1.3: Extract contexts and operations
    → Complete ✓

Step 2: Categorize invariants
  → Hardware addresses: 6 blocks
  → Register operations: 3 blocks
  → Memory layout: 2 blocks
  → Assembly: 1 block

Step 3: Verify against sources

  3.1: UART_BASE (0xFE201000)
    Source: BCM2711 ARM Peripherals, Section 2.1
    WebFetch: https://datasheets.raspberrypi.com/...
    Result: ✓ VERIFIED (exact match)

  3.2: SCTLR_EL1.M bit
    Source: ARM ARM, Section D13.2.118
    WebFetch: https://developer.arm.com/...
    Result: ✓ VERIFIED (bit 0, valid at EL1)

  [... continue for all invariants ...]

  → All 12 blocks verified ✓

Step 4: Logic soundness
  → Alignment calculations: SOUND ✓
  → Null checks: SOUND ✓
  → Bounds checks: SOUND ✓

Step 5: Assumption validation
  → "init() called once": VALIDATED ✓
  → "Single instance": VALIDATED ✓
  → "No aliasing": VALIDATED ✓

Step 6: Document findings
  → Generated verification report
  → All verifications PASSED
  → No action items

Step 7: Cross-reference
  → Constants consistent across files ✓
  → All unsafe blocks have safety comments ✓
  → Sources are current ✓

Report to user:
  "✓ All 12 unsafe blocks verified against authoritative sources.
   All safety invariants are correct, logic is sound, and assumptions
   are validated. Full report available in verification_report.md"
```

---

## Authoritative Source Priority

When verifying invariants, use sources in this priority order:

**Tier 1: Official Hardware Documentation**
1. BCM2711 ARM Peripherals (Raspberry Pi)
2. ARM Cortex-A72 Technical Reference Manual
3. ARM Architecture Reference Manual (ARM ARM)
4. PL011 UART TRM
5. GIC-400 Generic Interrupt Controller TRM
6. BCM54213PE PHY Datasheet
7. Broadcom GENET v5 Documentation

**Tier 2: Standards Documents**
1. IETF RFCs (ARP, IP, TCP, UDP, ICMP, DNS)
2. IEEE Standards (802.3 Ethernet, 802.11 WiFi)
3. ARM AAPCS (calling convention, alignment)
4. USB specifications
5. PCI Express specifications

**Tier 3: High-Quality Secondary Sources**
1. Linux kernel source code (drivers/net/ethernet/broadcom/genet/)
2. Authoritative blog posts (Philipp Oppermann's OS blog, Writing an OS in Rust)
3. Academic papers
4. Reference implementations (FreeBSD, NetBSD)
5. Manufacturer application notes

**Tier 4: Community Sources** (use with caution, cross-verify)
1. OSDev wiki
2. Stack Overflow (ARM tags, embedded-rust tags)
3. Raspberry Pi Forums (official)
4. Embedded Rust community documentation

**NEVER trust without verification**:
- Random blog posts
- Unattributed code snippets
- Forum posts without citations
- AI-generated content (including Claude's own output!)

**Always cross-reference**: If using Tier 3/4 sources, verify against Tier 1/2

---

## Common Issues and Solutions

### Issue: Missing `// SAFETY:` comment

**This is a CRITICAL violation** - Rust std library requires all `unsafe` code to document why it's safe.

**Solution**:
1. **Stop verification immediately** - cannot proceed without safety comments
2. Report violation to user with exact location:
   ```
   ❌ CRITICAL: Missing // SAFETY: comment
   Location: src/foo.rs:42
   Unsafe operation: Dereferencing raw pointer
   ```
3. User must add appropriate safety comment before verification can continue
4. Safety comment must explain ALL of:
   - Which invariants make the operation safe
   - How those invariants are established (checks, initialization, etc.)
   - Why alternative safe code cannot be used

**Example fix**:
```rust
// Before (WRONG):
unsafe { ptr::write_volatile(addr, value) }

// After (CORRECT):
// SAFETY: Address 0xFE201000 is valid UART register per BCM2711
//         datasheet Section 2.1. Pointer is aligned (4-byte registers),
//         non-null, and points to device MMIO (no caching issues).
//         Volatile write prevents compiler reordering.
unsafe { ptr::write_volatile(addr, value) }
```

### Issue: Datasheet not available online

**Solution**:
1. Check manufacturer's official site (ARM, Broadcom, Raspberry Pi)
2. Search for archived versions (Wayback Machine)
3. Use reference implementation source code (Linux kernel)
4. Document assumption clearly: "Unable to verify against datasheet; based on Linux driver v6.1"

### Issue: Source citation is vague

**Example**: "Per ARM documentation"

**Solution**:
1. Find the specific document and section
2. Update safety comment with precise citation:
   ```rust
   // SAFETY: Register accessible from EL1 per ARM ARM DDI0487
   //         Section D13.2.118 (SCTLR_EL1 definition)
   ```

### Issue: Multiple sources conflict

**Example**: Different address in BCM2711 vs. BCM2837 datasheet

**Solution**:
1. Use the correct datasheet for target hardware (Pi 4 = BCM2711)
2. Document the discrepancy:
   ```rust
   // NOTE: BCM2837 (Pi 3) uses 0x3F200000, but BCM2711 (Pi 4)
   //       uses 0xFE200000 per BCM2711 Peripherals Section 1.2
   const GPIO_BASE: usize = 0xFE200000;
   ```

### Issue: Logic is complex and hard to verify

**Example**: Convoluted pointer arithmetic

**Solution**:
1. Simplify the logic if possible
2. Add intermediate assertions:
   ```rust
   let aligned = (addr + 15) & !15;
   debug_assert_eq!(aligned % 16, 0, "Address must be 16-byte aligned");
   ```
3. Add unit tests for the calculation:
   ```rust
   #[test]
   fn test_alignment() {
       assert_eq!(align_up(0x80000), 0x80000);
       assert_eq!(align_up(0x80001), 0x80010);
       assert_eq!(align_up(0x8000F), 0x80010);
   }
   ```

### Issue: Assumption cannot be validated statically

**Example**: "Hardware is initialized by firmware"

**Solution**:
1. Document the assumption clearly
2. Add runtime checks if possible:
   ```rust
   // SAFETY: Assumes firmware initialized PL011 UART clock to 54MHz
   //         per Raspberry Pi 4 boot specification.
   // RUNTIME CHECK: Baud rate calculation verified by loopback test
   ```
3. Note in documentation that hardware testing is required

---

## Automation Potential

**Current**: Manual workflow execution by AI agent

**Future Automation**:
1. Parse all `// SAFETY:` comments → Extract claims
2. Match claims to source documents → Auto-fetch verification
3. Generate verification matrix → Pass/fail per invariant
4. CI integration → Block merges if verification fails

**Recommended**: Human review for:
- Complex logic soundness (not mechanically verifiable)
- Assumption validation (requires semantic understanding)
- Conflicting sources (requires judgment call)

---

## Success Criteria Checklist

Before marking verification complete:

- [ ] All unsafe blocks inventoried (all files scanned)
- [ ] **Every `unsafe` keyword has a `// SAFETY:` comment (Rust std library convention)**
- [ ] Safety comments use correct format (`//` not `/* */`)
- [ ] Safety comments immediately precede `unsafe` (no blank lines)
- [ ] Invariants categorized
- [ ] Hardware addresses verified against datasheets
- [ ] Register operations verified against ARM docs
- [ ] Memory layouts verified against linker script
- [ ] Protocol formats verified against RFCs/standards
- [ ] Singleton/aliasing patterns verified by code inspection
- [ ] Logic soundness checked (alignment, bounds, null checks)
- [ ] Assumptions validated (ordering, initialization, exclusivity)
- [ ] Cross-reference check passed (consistency, completeness)
- [ ] Verification report generated
- [ ] Action items created for any failures
- [ ] Sources are authoritative and current

---

## Output Format

**Report Structure**:
```markdown
# Safety Verification Report

**Date**: YYYY-MM-DD
**Verifier**: [Tool/Person]
**Scope**: [Files/Modules]
**Status**: [PASS / FAIL / PARTIAL]

## Summary
- Total unsafe blocks: N
- Verified: N
- Failed: N
- Warnings: N

## Verification Results

### [Category]: Hardware Addresses

#### ✓ src/drivers/uart.rs:67 - UART_BASE
- **Claimed**: 0xFE201000 per BCM2711 datasheet
- **Verified**: BCM2711 ARM Peripherals, Section 2.1, Page 5
- **Source URL**: https://datasheets.raspberrypi.com/bcm2711/bcm2711-peripherals.pdf
- **Status**: VERIFIED
- **Date**: YYYY-MM-DD

[... repeat for all blocks ...]

## Failures

[If any - detail each failure with action items]

## Warnings

[Non-critical issues - vague citations, missing source versions, etc.]

## Recommendations

[Improvements to safety comments, documentation, etc.]

---
**Verification complete**: [YYYY-MM-DD HH:MM]
```

---

## Notes

- **This workflow verifies safety invariants, not functional correctness**
- All `unsafe` blocks must pass verification before deployment
- Update verification when:
  - Hardware changes (new Pi model, new peripherals)
  - Specifications updated (new ARM ARM revision)
  - Code refactored (safety assumptions may change)
- Keep verification reports in version control for audit trail
- Safety verification is ongoing, not one-time
