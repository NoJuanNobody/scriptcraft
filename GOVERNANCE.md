# Governance

ScriptCraft is a small project. This document says how it is run, so contributors know what to
expect and how to take on more responsibility.

## Mission

An open, clean-room, Rust-native screenwriting app that works the way writers already work, that
people own, and that agents can drive. It supports a creator-focused tool chain, including comic and
script pipelines, and stays free of proprietary code and assets.

## Roles

- **Maintainer (currently Alejandro Londono, [@NoJuanNobody](https://github.com/NoJuanNobody)).**
  Reviews and merges pull requests, sets the roadmap, cuts releases, triages security reports and
  enforces the project rules.
- **Contributors.** Anyone who opens an issue, improves docs or sends a pull request.
- **Committers.** Contributors who have shown sound judgment and are given merge rights.

## How decisions are made

1. Most changes are decided in the pull request or issue by discussion and review.
2. Larger changes (new crates, file-format behavior, public command or protocol changes) are
   proposed in an issue first. The maintainer decides after discussion, aiming for consensus.
3. Anything that affects the project rules in [AGENTS.md](AGENTS.md) (never crash, clean room,
   everything is a command, layering) needs the maintainer's explicit approval.
4. When there is no consensus, the maintainer makes the call and records the reasoning in the issue.

## Becoming a committer

A contributor with several merged pull requests that follow the rules, who reviews others' work
helpfully, may be invited to become a committer. Nominate yourself or someone else by opening an
issue. The maintainer decides and records it here.

Current committers: none besides the maintainer.

## The rules apply to everyone

Every change passes `cargo xtask ci`, follows the rules in [CONTRIBUTING.md](CONTRIBUTING.md) and
keeps the clean-room guarantee: no Final Draft code, icons, images, templates, sample scripts or
fonts. Changes that risk this are reverted.

## Releases

The maintainer tags releases from `main` after the quality gate passes. Notes list user-visible
changes, fixes and security credits.

## Conduct and moderation

Be respectful and assume good intent. Harassment, personal attacks and discrimination are not
tolerated. The maintainer may edit or remove comments, close discussions and, for repeated or serious
violations, block accounts. To raise a concern privately, use a GitHub security advisory
(<https://github.com/NoJuanNobody/scriptcraft/security/advisories/new>) or email
<alondono11@gmail.com>.

## Security

See [SECURITY.md](SECURITY.md).

## Licensing and trademarks

Code is `MIT OR Apache-2.0`. Contributions are accepted under the same terms. Final Draft is a
trademark of its owner; ScriptCraft is not affiliated with it (see [NOTICE](NOTICE)).

## Changing this document

Open a pull request. The maintainer approves changes after allowing at least a week for comment,
except for typo fixes.
