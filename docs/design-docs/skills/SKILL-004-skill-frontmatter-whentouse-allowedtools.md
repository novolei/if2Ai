# SKILL-004 Skill Frontmatter / WhenToUse / AllowedTools

## Problem
`SKILL.md` content alone is too blunt. Stable agent behavior needs structured
metadata that explains when a skill should activate, which tools it expects,
and what evidence caused activation.

## Design
Skill auto-load now parses a minimal frontmatter subset:

- `whenToUse` / `when_to_use`
- `allowedTools` / `allowed_tools`
- `modelHint` / `model_hint` / `model`

The data is projected onto `SkillResolutionCandidate` and rendered into the
auto-loaded skill prompt block as runtime metadata.

## Trust Boundary
SKILL-004 does not relax AWL-003. Only trusted builtin/workspace/user skills
that pass review may enter provider context. Untrusted skills can appear only as
blocked candidates/proposals.
