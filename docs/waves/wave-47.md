# Wave 47: takt carries no organisation's own route; git, the model and the forge through loopback relays

Stage: **review fixed and confirmed, PR next**. Shape: one coordinator and implementor sub-agents in one
tree, on `wave/47` from be894749 (wave 46's merge).

## Units

| unit | story | stage |
|---|---|---|
| U1 | story:takt-carries-no-organisations-own-route | committed (2c97598e, 28ef6621; review f730709e, f2715f57, 2ab5e9bf) |
| U2 | story:git-and-model-through-loopback-relays | committed (7adb6758); Acceptance 2 and 4 wait for a Loom release |
| U3 | story:connectors-forge-speaks-github-at-an-endpoint | committed (8839c7fb; review f730709e, 2ab5e9bf) |

Ride-alongs: engineering-protocols 0.15.2 and Loom 0.51.0 (868335d4); one loopback check for the
crate (98fe3396); the connectors forge on the Connectors 0.60.0 command line, in U3.

## Log

- U1 removes `--forge bot`, the bot forge, the `gates` scan adapter and every use of an
  organisation's private scan policy. `takt scenario run` cuts `trial/<n>` with the operator's
  own git on `origin`, with a lease that only creates the branch. U1 removed 49 adversary cases
  that attacked only the removed route and scan; each removal names the story in its hunk, and
  the seven emptied adversary files were deleted in 28ef6621.
- Loom 0.51.0's governor still pins engineering-protocols 0.14.0, so the lock holds 0.14.0 and
  0.15.2; `story.delivery/1` and `/2` differ between them only in `pr.open`'s description.
- U2 admits plain `http://` for git and a model endpoint with no credential only on a loopback
  host, and follows no redirect there. Loom 0.51.0's session executor refuses a b10x-harness
  session whose endpoint names no token file and no secret, so Acceptance 2 and 4 cannot pass
  on it; the story stays active until takt pins a Loom release that admits it.
- U3 drives a GitHub pull request through a served Connectors instance at `forge_endpoint`, with
  no credential, against a loopback stand-in of the served wire. A head with no check run and no
  status is pending, not pass. Connectors 0.60.0 refuses a write on its command line without
  `--approval-file`, which takt does not pass, so the command-line route cannot open or merge.
- Review at 7b85ed23: an adversarial pass and a security review wrote 15 red cases. The blocker
  was that on github the served `branch.get` answer alone named the merge target's tip, so an
  answer naming the case's own revision hid a judge-surface change from the guard. The tip now
  comes from `branch.get` only when `origin`'s branch, read through git, names the same commit.
  The warnings and notes, all fixed in f730709e:
  - relay lines went through a proxy;
  - the relay URL came from a parse other than the one that admitted it;
  - a `pushInsteadOf` rewrite, a `<helper>::<url>` origin, or git configuration in the
    environment could carry a relay line off loopback;
  - check-run paging ended on a short page;
  - a port above 65535 was accepted;
  - the prompt filter missed an `Authorization` header, AWS keys and a URL password, which only
    the removed scan had caught.

  The trial agent's profile, `AGENTS.md` and the design doc still named the removed route; that is
  fixed in f2715f57. Both reviewers approved at f2715f57 with one note: the connectors relay
  check read git's configuration without the part the environment hands git, which the push
  keeps. That is fixed in 2ab5e9bf, with a test that is red without the fix. The claim that
  checks may be judged on a head that moved between two reads is filed as
  story:ci-checks-judge-the-head-ci-read-compared. Both reviewers confirmed at 2ab5e9bf, and
  `task check-static` is green at f2715f57.
- After 0.24.0, wave 48 opens at once: the next Loom pin (U2's Acceptance 2 and 4), the
  connectors forge against a real served instance, and push and fetch through the Connectors git
  relay; 0.25.0 follows.
