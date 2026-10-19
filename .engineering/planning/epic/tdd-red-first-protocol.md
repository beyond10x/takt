---
format: aep.planning-md/3
id: epic:tdd-red-first-protocol
kind: epic
status: draft
title: 'takt runs tdd.red_first/1: one red test before any code, the least code to green'
summary: owner handover 2026-10-19; five validators missing (test_delta, failure_report, implementation_change, diff_coverage, scope_judgement)
relations:
- serves: vision:safe-agentic-coding
- informed_by: story:story-delivery-tests-first
revision: 1
---
# Epic: tdd-red-first-protocol

## Outcome

takt runs `tdd.red_first/1`, a red-green-refactor protocol for one behaviour per case: one new
test, red on an assertion before any implementation, the least code that turns it green, refactor
under green. Every gate reads a validator record bound to the working revision. Once takt runs it,
the protocol moves to engineering-protocols (Part 1 of the protocols move).

## Context

Handed over by the owner on 2026-10-19 as a Canon `protocol/1` document, modelled on an existing
red-first coding-agent plugin that gates each edit with a model judge. `canon validate` passes;
`canon check` refused at 17,694,720 states against its 65,536 limit, so no full state check ran.
takt 0.1.0 loads it and ran one `tests.run` step.

It closes three bypasses of the plugin: test files never judged (test_delta covers test paths),
a turn classifier that leaves the guard off (every write is a frontier action), and a task-runner
test command not recognised (the case declares its command, run without a shell).

takt lacks the validators for five evidence kinds: `test_delta` (count new tests),
`failure_report` (read a JUnit failure report), `implementation_change`, `diff_coverage` and the
advisory `scope_judgement`. Related: story:story-delivery-tests-first (a story case writes its
tests first), active.

Sequencing: after the RSI rounds (wave 15), per the owner's order to run improvement rounds before
shipping new features.

## The protocol as handed over

```yaml
format: protocol/1

protocol:
  id: tdd.red_first
  revision: 1
  description: >-
    One red-green-refactor cycle for one behaviour, modelled on the rules of an existing red-first
    coding-agent plugin that gates each edit with a model judge, restated as a takt protocol. A
    case is one cycle: pick one behaviour, write one new test, see it fail on an assertion, write
    the least code that passes it, see it green, refactor under green. Every gate reads a
    validator's record bound to the working revision, so any new commit returns the claims to
    unknown and the tests must be run again. The over-implementation judge is an advisory signal
    unless the case opens with judge_mode blocking; the blocking check that needs no model is diff
    coverage of the new test.

artifacts:
  behaviour:
    description: the one behaviour this cycle adds or the one defect it fixes
  implementation:
    description: >-
      the working revision of the repository, test files and production code together; every
      write step moves it

evidence_kinds:
  behaviour_choice:
    description: >-
      the harness chose one behaviour for the cycle, with no file changed; takt records the choice
      as it records a handover. It has no result
  test_delta:
    description: >-
      the test paths at the working revision diffed against the case's base: one (exactly one new
      test case, no existing test changed), several (two or more new test cases, at once or across
      writes), none, existing_changed (an existing test or assertion changed or deleted), or
      logic_in_tests (a helper, fixture or non-test function added under a test path). Closes the
      source plugin's gap that test files are never judged
  test_result:
    description: >-
      the case's declared test command run without a shell against the working revision: pass on
      exit 0, fail otherwise. The command is whatever the case declares (task test, cargo nextest,
      ./check_all.sh); nothing recognises a test run by pattern, so no runner is missed
  failure_report:
    description: >-
      the same run's JUnit report read for the new test: assertion when it is among the failures
      and every other test passed, error when it failed to compile, import or collect. Written
      only for a failing run
  implementation_change:
    description: >-
      the production paths (every path that is not a test path) at the working revision differ
      from the case's base; written only when they do, its result is changed
  diff_coverage:
    description: >-
      every production line changed since the case's base is executed by the new test, run alone
      with coverage: covered, or uncovered with the lines listed. The mechanical reading of "no
      more code than the test needs"
  scope_judgement:
    description: >-
      a model judge (the source plugin's judge prompt, second opinion included) read the diff
      since the base, test files included: minimal or over_implementation. A signal, not proof;
      its record names its producer, and only a producer independent of the author counts

independence_dimensions:
  not_author:
    description: the producer is not the harness session that wrote the change

claims:
  behaviour.chosen:
    description: one behaviour is chosen for this cycle
    true_when:
      evidence:
        kind: behaviour_choice
        subject: behaviour
  one.new.test:
    description: exactly one new test case since the base, no existing test changed, no logic in test files
    true_when:
      evidence:
        kind: test_delta
        result: one
        subject: implementation
  tests.pass:
    description: the declared test command passes at the working revision
    true_when:
      evidence:
        kind: test_result
        result: pass
        subject: implementation
  tests.fail:
    description: the declared test command fails at the working revision
    true_when:
      evidence:
        kind: test_result
        result: fail
        subject: implementation
  failure.assertion:
    description: the new test fails on an assertion at the working revision, every other test passing
    true_when:
      evidence:
        kind: failure_report
        result: assertion
        subject: implementation
  failure.error:
    description: the new test fails to compile, import or collect at the working revision
    true_when:
      evidence:
        kind: failure_report
        result: error
        subject: implementation
  red.seen:
    description: red for the right reason at the working revision, with one new test
    true_when:
      all:
        - claim: tests.fail
        - claim: failure.assertion
        - claim: one.new.test
  red.before.code:
    description: a failing run was recorded strictly before the first production change of the case
    true_when:
      before:
        first: failure_report
        then: implementation_change
        subject: implementation
  production.changed:
    description: the production paths differ from the case's base at the working revision
    true_when:
      evidence:
        kind: implementation_change
        result: changed
        subject: implementation
  implementation.minimal:
    description: every production line changed since the base is executed by the new test
    true_when:
      evidence:
        kind: diff_coverage
        result: covered
        subject: implementation
  scope.minimal:
    description: a judge independent of the author found no over-implementation at the working revision
    true_when:
      evidence:
        kind: scope_judgement
        result: minimal
        subject: implementation
        independent_on:
          - not_author

obligations:
  one_test.required:
    description: one new test, no more
    discharged_when:
      claim: one.new.test
  red.required:
    description: the test was seen failing before any production code
    discharged_when:
      claim: red.before.code
  green.required:
    description: the tests pass at the final revision
    discharged_when:
      claim: tests.pass
  minimal.required:
    description: no production line the new test does not execute
    discharged_when:
      claim: implementation.minimal
  judge.required:
    description: the judge found the change minimal; only when the case opened with judge_mode blocking
    applicable_when:
      input: judge_mode
      in:
        - blocking
    discharged_when:
      claim: scope.minimal

actions:
  repository.inspect:
    description: read the repository; change nothing
    effect: read
  behaviour.pick:
    description: >-
      name the one behaviour this cycle adds or the one defect it fixes, changing no file; takt
      records the choice. Taken once a cycle
    effect: read
    precondition:
      claim: behaviour.chosen
      is: unknown
    may_produce:
      - evidence: behaviour_choice
  tests.write:
    description: >-
      add or change test files only; takt refuses and reverts a step that touches a non-test
      path. There is no turn classifier: every write is an action the frontier offered, so
      "continue" after a non-coding prompt is one more step under the same rules
    effect: write
    precondition:
      claim: behaviour.chosen
  tests.delta:
    description: diff the test paths against the case's base and record the test delta
    effect: read
    may_produce:
      - evidence: test_delta
  tests.run:
    description: >-
      run the case's declared test command and read its JUnit report; records the result, and
      for a failing run the failure report
    effect: read
    may_produce:
      - evidence: test_result
      - evidence: failure_report
  implementation.stub:
    description: >-
      after a red by error, add only the declaration the new test needs, returning a value that
      contradicts its assertion, so the next run can fail for the right reason
    effect: write
    precondition:
      all:
        - claim: failure.error
        - claim: one.new.test
  implementation.edit:
    description: >-
      change production code, the least that makes the new test pass; offered only at a revision
      where the new test is red on an assertion. Taken when not offered, the step is refused and
      reverted
    effect: write
    precondition:
      claim: red.seen
  implementation.delta:
    description: diff the production paths against the case's base; record a change when there is one
    effect: read
    may_produce:
      - evidence: implementation_change
  coverage.run:
    description: run the new test alone with coverage over the production lines changed since the base
    effect: read
    precondition:
      all:
        - claim: tests.pass
        - claim: production.changed
    may_produce:
      - evidence: diff_coverage
  scope.judge:
    description: >-
      ask the over-implementation judge about the diff since the base, test files included; its
      verdict is advisory unless the case opened with judge_mode blocking
    effect: read
    may_produce:
      - evidence: scope_judgement
  implementation.refactor:
    description: >-
      restructure under green without adding behaviour or tests; the revision moves, so the
      tests, the coverage and the judge are read again before the cycle is done
    effect: write
    precondition:
      all:
        - claim: tests.pass
        - claim: implementation.minimal
        - claim: one.new.test

outcomes:
  done:
    description: >-
      the cycle closed: one new test, red before code, green at the final revision, every changed
      line executed by the new test, and the judge's minimal verdict when the case made it blocking
    requires:
      all:
        - claim: behaviour.chosen
        - claim: one.new.test
        - claim: red.before.code
        - claim: tests.pass
        - claim: implementation.minimal
        - any:
            - input: judge_mode
              in:
                - advisory
            - claim: scope.minimal
  abandoned:
    description: the cycle closed on an explicit decision to drop the behaviour
    requires:
      decision: explicitly_abandoned

case:
  inputs:
    judge_mode:
      description: whether the over-implementation judge's verdict gates the outcome
      values:
        - advisory
        - blocking
```
