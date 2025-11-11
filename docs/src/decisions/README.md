# Architecture Decision Records (ADRs)

This directory contains Architecture Decision Records - documents that capture important architectural choices made during DaedalusOS development.

## What is an ADR?

An ADR documents **why** a significant technical decision was made. It captures:
- The problem or choice faced
- Alternatives considered
- The decision and rationale
- Consequences and trade-offs

ADRs are lightweight (typically 100-400 lines) and focus on **decision rationale**, not implementation details.

## When to Write an ADR

Write an ADR when:

✅ **"Would future-me wonder why this design exists?"**

Specific triggers:
- **One-way doors**: Hard-to-reverse decisions (e.g., target platform, no multi-arch)
- **Non-obvious trade-offs**: Choices where alternatives had merit (e.g., QEMU version requirement)
- **Future-facing design**: Adding abstraction/complexity now for future benefit (e.g., NetworkDevice trait)
- **Breaking conventions**: Deviating from common patterns (with good reason)
- **External dependencies**: Requiring specific versions/tools (e.g., QEMU 9.0+)

Don't write ADRs for:
- ❌ Implementation details (those go in module docs)
- ❌ Obvious choices (e.g., "use Rust for Rust project")
- ❌ Easily reversible decisions (refactorings, minor API changes)
- ❌ Temporary workarounds (comment in code is sufficient)

## ADR Template

```markdown
# ADR-XXX: Decision Title

**Status**: Accepted | Proposed | Deprecated | Superseded by ADR-YYY
**Date**: YYYY-MM-DD
**Decision**: One-sentence summary of the decision.

## Context

What problem are we solving? What constraints exist?
What alternatives were considered?

## Decision

What did we decide to do?
(Keep this section concise - 1-3 paragraphs)

## Rationale

Why this decision over alternatives?
- Reason 1
- Reason 2
- ...

### Alternatives Considered

**Alternative 1: [Name]**
- Pros: ...
- Cons: ...
- Why rejected: ...

**Alternative 2: [Name]**
- Pros: ...
- Cons: ...
- Why rejected: ...

## Consequences

### Positive
- Benefit 1
- Benefit 2

### Negative
- Cost 1
- Cost 2

### Neutral (optional)
- Side effect 1

## Reversal Plan

How would we undo this decision if needed?
- Step 1
- Step 2
- Estimated cost: X weeks/effort

**Triggers for reversal**: What would make us reconsider?

## Related Decisions

- [ADR-XXX: Related Decision](adr-xxx.md) - How it relates

## References

- [External source 1](https://...)
- [External source 2](https://...)
```

## Best Practices

### 1. Context Before Decision

Explain the problem and show alternatives **before** stating what you chose. This prevents "obvious in hindsight" bias.

**Good**:
```markdown
## Context
We need to support multiple network devices (Pi 4 GENET, future Pi 5, QEMU mock).

Three approaches:
- A) Direct GENET usage (no abstraction)
- B) Full trait abstraction now
- C) Minimal trait now, implement later

## Decision
Chose option C: Minimal trait now...
```

**Bad**:
```markdown
## Decision
We're using a trait for network devices.

## Context
This lets us support multiple devices...
```

### 2. Acknowledge Trade-offs

Good ADRs admit downsides. No decision is perfect.

**Good**:
```markdown
### Negative
- Setup complexity: Users must build QEMU from source
- CI build time: ~4 minutes on first run
```

**Bad**:
```markdown
### Consequences
- Better testing
- More accurate emulation
(No admission of downsides)
```

### 3. Include Reversal Plans

Show you've thought about "what if we're wrong?"

**Good**:
```markdown
## Reversal Plan

If multi-architecture support becomes necessary:
1. Create ADR-00X documenting new scope
2. Design HAL separating platform code
3. Restructure: src/platform/{rpi4,x86_64}
4. Test on both platforms

**Cost estimate**: 2-4 weeks of refactoring.
**Trigger**: Need to support x86 for CI, or Pi 5 requires different approach.
```

### 4. Status Lifecycle

```
Proposed → Accepted → [Deprecated | Superseded]
```

- **Proposed**: Under discussion, not yet implemented
- **Accepted**: Implemented and active
- **Deprecated**: No longer recommended, but code remains
- **Superseded by ADR-XXX**: Replaced by new decision

Update status when circumstances change.

### 5. Link Related ADRs

Decisions often build on or conflict with previous ones:

```markdown
## Related Decisions
- [ADR-001: Pi 4 Only](adr-001-pi-only.md) - Why we need raspi4b specifically
- [ADR-003: Network Abstraction](adr-003.md) - Plans for multi-device support
```

## Numbering Convention

ADRs are numbered sequentially with zero-padding:
- `adr-001-pi-only.md`
- `adr-002-qemu-9.md`
- `adr-003-network-device-trait.md`

Numbers are permanent. If ADR-002 is superseded, we create ADR-004 (not rename ADR-002).

## File Naming

Format: `adr-NNN-short-slug.md`

Examples:
- ✅ `adr-001-pi-only.md`
- ✅ `adr-002-qemu-9.md`
- ❌ `adr-1-raspberry-pi-4-only-target-platform.md` (too long, no zero-padding)

## Examples in This Project

### ADR-001: Raspberry Pi 4 Only
**Type**: Platform choice (one-way door)
**Demonstrates**: Clear rationale for rejecting multi-platform, detailed reversal plan

### ADR-002: QEMU 9.0+ Requirement
**Type**: External dependency requirement
**Demonstrates**: "Why Not" alternatives section, multiple implementation options

### ADR-003: Network Device Abstraction
**Type**: Future-facing design (abstraction for 1 implementation)
**Demonstrates**: Three options with honest pros/cons, migration path, design pattern comparisons

## Anti-Patterns to Avoid

❌ **"Implementation Masquerading as ADR"**
```markdown
# ADR-005: UART Driver Implementation
## Decision
The UART driver uses PL011 registers at 0xFE201000...
```
→ This is implementation detail, belongs in module docs.

❌ **"No Alternatives Shown"**
```markdown
## Decision
We use Rust.
```
→ If there's no real choice, don't write an ADR.

❌ **"Bias Toward Decision"**
```markdown
## Alternatives
1. Direct GENET usage - terrible, inflexible, bad
2. Trait abstraction - perfect, elegant, future-proof
```
→ Be honest about trade-offs.

❌ **"No Reversal Plan"**
```markdown
## Consequences
This is a permanent decision.
```
→ Few decisions are truly irreversible. Show you've thought about it.

## ADR Workflow

1. **Identify decision**: Recognize a significant architectural choice
2. **Draft ADR**: Use template, fill in context/alternatives
3. **Discuss if needed**: For team projects; solo projects can skip
4. **Implement**: Make the change
5. **Finalize ADR**: Update with actual implementation details
6. **Commit together**: ADR and implementation in same PR/commit

For DaedalusOS (solo project), ADRs can be written during or after implementation, as long as rationale is captured while fresh.

## References

- [Michael Nygard's ADR article](https://cognitect.com/blog/2011/11/15/documenting-architecture-decisions) - Original ADR concept
- [ADR GitHub organization](https://adr.github.io/) - Templates and tools
- [Documenting Architecture Decisions](https://www.thoughtworks.com/en-us/insights/blog/architecture/documenting-architecture-decisions) - ThoughtWorks guide
