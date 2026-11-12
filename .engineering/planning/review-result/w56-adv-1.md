---
format: aep.planning-md/3
id: review-result:w56-adv-1
kind: review-result
status: active
title: Wave 56 units a, b, c and d, adversary pass and security review
summary: nine findings fixed by the correction and the link refusal, four escalated as drafts
relations:
- reviews: story:work-items-in-the-ess-model
- reviews: story:work-flag-and-the-aep-source
- reviews: story:the-command-work-source
- reviews: story:git-excluded-files-are-not-step-changes
- reviews: story:a-step-may-not-commit-a-link-at-a-deployment-path
revision: 1
---
# Adversary and security review, wave 56 units a, b, c and d

```
unit: story:work-items-in-the-ess-model + story:work-flag-and-the-aep-source + story:the-command-work-source + story:git-excluded-files-are-not-step-changes
verdict: needs change
cases: adversary 5, red 4; security 7, red 6, reviewed at 9a67088ee
```

The cases are `crates/takt/tests/work_key_w56_adv.rs`, `crates/takt-cli/tests/run_work_command_w56_adv.rs` and
`crates/takt/tests/work_and_deployment_w56_sec.rs`. Both required mutation checks went red: reverting the finished-item
guard, and passing an empty deployment list at case open.

| finding | unit | case | outcome |
|---|---|---|---|
| a link a step put at a deployment path was exempt, so the deployment's next write landed in `src/` and was committed | d | `a_link_the_step_puts_at_a_deployment_path_is_a_change` | fixed (a19b058f2, 242ea0649) |
| an edit step could commit such a link | d | `an_edit_step_that_puts_a_link_at_the_deployments_directory_is_refused_and_put_back` | fixed (242ea0649), story:a-step-may-not-commit-a-link-at-a-deployment-path |
| an entry without `/` covered an ignored directory of that name | d | `an_entry_without_a_slash_does_not_cover_what_a_step_writes_below_it` | fixed |
| the command source's `select` and `read` ran on a finished case's branch, on the model's commits | c | `select_after_a_finished_case_does_not_run_on_the_cases_branch` | fixed |
| a declined command case was resumed for ever | c | `an_argument_less_run_passes_a_declined_command_case_over` | fixed |
| `Key::parse` took zero-width and bidi characters, and an id starting with `-` | b | `a_key_whose_id_does_not_show_as_itself_is_refused`, `a_key_whose_id_holds_an_invisible_or_format_character_is_refused`, `an_id_starting_with_a_dash_never_reaches_the_work_command_as_an_option` | fixed |
| an item title took U+2028 and U+2029 | c | `an_item_title_holding_a_unicode_line_break_is_refused` | fixed |
| `--work command:<id>` opened a second case on an item finished at its revision | c | `work_naming_an_item_whose_case_is_finished_at_its_revision_is_refused` | fixed |
| work-command output past the runner's 8 MiB was read as cut | c | `output_past_the_runners_limit_is_refused_naming_the_command_and_the_limit` | fixed |
| files the run file names may lie under `deployment_paths` | d | removed from the file, kept with its draft | escalated: story:files-the-run-file-names-are-not-deployment-paths |
| takt's git still reads `.git/info/exclude` and a global excludes file | d | removed from the file, kept with its story | escalated: story:git-reads-no-user-excludes |
| `takt approve` takes no `deployment_paths` | d | none | escalated: story:takt-approve-keeps-the-deployments-paths |
| item fields have no bounds of their own | c | none | escalated: story:work-item-fields-have-their-own-bounds |

Found while correcting and filed as a draft: story:the-command-forge-and-the-work-source-refuse-alike.

Attacked and held: the work command starts through the confinement seam with the forge's credential scrub; `{id}` is
substituted per argument with no shell; minted case ids pass `check_id` and never collide for 23 adversarial keys and
4000 generated ones; `--work` and `--story` take, resume and refuse alike; the trailer is right for every source; the
judge surface still sees a deployment path's files; `.git` and the run file are refused in the list.

The staged gate's CLI stage found one more defect, outside both reviews: the run's work sources defaulted to none when
the command line was not built through the run file reader, so the binary's own unit tests found nothing ready; fixed in
58723b3e3.
