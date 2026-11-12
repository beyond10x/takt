//! A run on a schedule (`takt.work.Schedule`, story:a-run-on-a-schedule): `takt run --every
//! <duration>` stays resident and starts one pass, a tick, per slot. This module holds the parts
//! of a schedule parsed from a flag or the run file's `schedule` member, the slots (a pure
//! function, [`Schedule::next_slot`], driven by fixed instants), the schedule's id, the files a
//! schedule and a run write (`takt.schedule-tick/1`, `takt.run-outcome/1`) and the signal count
//! the resident loop reads. The loop, which starts each tick as a child `takt run --once`, is
//! the CLI's.
//!
//! Zones are read from the tz database bundled in the binary (`jiff`, `tzdb-bundle-always`), so
//! a container needs no tzdata. The design is `docs/design/scheduled-runs-and-connector-actions.md`
//! § 3.

use std::path::{Path, PathBuf};

pub use jiff::Timestamp;
use jiff::civil::{Date, DateTime, Time};
use jiff::tz::{AmbiguousOffset, TimeZone, TimeZoneDatabase};
use sha2::Digest as _;

/// The format of one line of `ticks.jsonl`.
pub const TICK_FORMAT: &str = "takt.schedule-tick/1";
/// The format of `--outcome-file`'s file.
pub const OUTCOME_FORMAT: &str = "takt.run-outcome/1";
/// The format of `schedule.json`.
pub const SCHEDULE_FORMAT: &str = "takt.schedule/1";
/// The format of `end.json`.
pub const END_FORMAT: &str = "takt.schedule-end/1";

/// The rule an `every` breaks.
pub const EVERY_RULE: &str = "is a duration of at least one second, whole seconds in `d`, `h`, `m` and `s`, such as `30m` or `1h30m`";
/// The rule a `time_zone` breaks.
pub const ZONE_RULE: &str = "is an IANA time zone name, such as `Europe/Berlin` or `UTC`";
/// The rule a window breaks.
pub const WINDOW_RULE: &str = "is a list of windows `<days> <HH:MM>[-<HH:MM>]`, the days `Mon` to `Sun`, a range `Mon-Tue` or a list `Mon,Wed`, the end not before the start";
/// The rule a stop condition breaks.
pub const UNTIL_RULE: &str =
    "is a list of `nothing-ready`, `exit:<n>`, `outcome:<name>` and `deadline:<RFC 3339 instant>`";
/// The rule the `schedule` member itself breaks.
pub const SCHEDULE_RULE: &str =
    "is a mapping of `every`, `time_zone`, `windows` and `until`, and names `every`";

const DAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
const DAY_SECONDS: u64 = 24 * 60 * 60;
/// How far ahead the slots of a windowed schedule are looked for: past a year with no slot, the
/// schedule has none left.
const HORIZON_DAYS: i64 = 400;

/// The bundled tz database, read once.
fn database() -> &'static TimeZoneDatabase {
    static DATABASE: std::sync::OnceLock<TimeZoneDatabase> = std::sync::OnceLock::new();
    DATABASE.get_or_init(TimeZoneDatabase::bundled)
}

/// `text` as a duration of whole seconds: one or more `<n><unit>`, the unit `d`, `h`, `m` or
/// `s`; at least one second. The rule broken otherwise is [`EVERY_RULE`].
pub fn every(text: &str) -> Result<u64, String> {
    let refused = || EVERY_RULE.to_owned();
    let mut total: u64 = 0;
    let mut digits = String::new();
    if text.is_empty() {
        return Err(refused());
    }
    for c in text.chars() {
        if c.is_ascii_digit() {
            digits.push(c);
            continue;
        }
        let unit = match c {
            'd' => DAY_SECONDS,
            'h' => 3600,
            'm' => 60,
            's' => 1,
            _ => return Err(refused()),
        };
        let n: u64 = digits.parse().map_err(|_| refused())?;
        digits.clear();
        total = n
            .checked_mul(unit)
            .and_then(|seconds| total.checked_add(seconds))
            .ok_or_else(refused)?;
    }
    if !digits.is_empty() || total < 1 {
        return Err(refused());
    }
    Ok(total)
}

/// `seconds` written as [`every`] reads it, the largest units first: `1800` is `30m`.
#[must_use]
pub fn every_text(seconds: u64) -> String {
    let mut left = seconds;
    let mut text = String::new();
    for (unit, size) in [("d", DAY_SECONDS), ("h", 3600), ("m", 60), ("s", 1)] {
        if left >= size {
            text.push_str(&format!("{}{unit}", left / size));
            left %= size;
        }
    }
    if text.is_empty() {
        text.push_str("0s");
    }
    text
}

/// A time zone by its IANA name, from the bundled database.
#[derive(Debug, Clone)]
pub struct Zone {
    /// The name as given.
    pub name: String,
    tz: TimeZone,
}

impl PartialEq for Zone {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

/// The zone `text` names in the bundled database; the rule broken otherwise is [`ZONE_RULE`].
pub fn zone(text: &str) -> Result<Zone, String> {
    if text.is_empty() || text.trim() != text {
        return Err(ZONE_RULE.to_owned());
    }
    database()
        .get(text)
        .map(|tz| Zone {
            name: text.to_owned(),
            tz,
        })
        .map_err(|_| ZONE_RULE.to_owned())
}

/// UTC, the zone of a schedule that names none.
#[must_use]
pub fn utc() -> Zone {
    Zone {
        name: "UTC".to_owned(),
        tz: TimeZone::UTC,
    }
}

/// A window: the days it falls on and a wall-clock start with an optional end, each in seconds
/// after midnight.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Window {
    /// The window as written.
    pub text: String,
    /// Monday first.
    days: [bool; 7],
    start: u64,
    end: Option<u64>,
}

fn day(text: &str) -> Option<usize> {
    DAYS.iter().position(|name| name.eq_ignore_ascii_case(text))
}

fn clock(text: &str) -> Option<u64> {
    let (hours, minutes) = text.split_once(':')?;
    if hours.len() != 2 || minutes.len() != 2 {
        return None;
    }
    let hours: u64 = hours.parse().ok().filter(|h| *h < 24)?;
    let minutes: u64 = minutes.parse().ok().filter(|m| *m < 60)?;
    Some(hours * 3600 + minutes * 60)
}

/// The window `text` writes, `<days> <HH:MM>[-<HH:MM>]`: `Mon-Tue 07:00-22:30`, `Wed 07:30`,
/// `Mon,Wed 09:00`; a range of days may wrap past Sunday. The rule broken otherwise is
/// [`WINDOW_RULE`].
pub fn window(text: &str) -> Result<Window, String> {
    let refused = || WINDOW_RULE.to_owned();
    let mut parts = text.split_whitespace();
    let (Some(days_text), Some(times), None) = (parts.next(), parts.next(), parts.next()) else {
        return Err(refused());
    };
    let mut days = [false; 7];
    for item in days_text.split(',') {
        match item.split_once('-') {
            Some((from, to)) => {
                let (from, to) = (day(from).ok_or_else(refused)?, day(to).ok_or_else(refused)?);
                let mut at = from;
                loop {
                    days[at] = true;
                    if at == to {
                        break;
                    }
                    at = (at + 1) % 7;
                }
            }
            None => days[day(item).ok_or_else(refused)?] = true,
        }
    }
    let (start, end) = match times.split_once('-') {
        Some((start, end)) => (
            clock(start).ok_or_else(refused)?,
            Some(clock(end).ok_or_else(refused)?),
        ),
        None => (clock(times).ok_or_else(refused)?, None),
    };
    if end.is_some_and(|end| end < start) {
        return Err(refused());
    }
    Ok(Window {
        text: text.to_owned(),
        days,
        start,
        end,
    })
}

/// A stop condition of `until`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Until {
    /// A tick's outcome says `nothing_ready`.
    NothingReady,
    /// A tick exited this code.
    Exit(i32),
    /// A tick's case reached this outcome of its protocol.
    Outcome(String),
    /// No slot falls at or after this instant.
    Deadline(Timestamp),
}

impl Until {
    /// The condition's kind, the reason `end.json` gives when it holds.
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            Until::NothingReady => "nothing-ready",
            Until::Exit(_) => "exit",
            Until::Outcome(_) => "outcome",
            Until::Deadline(_) => "deadline",
        }
    }
}

impl std::fmt::Display for Until {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Until::NothingReady => write!(f, "nothing-ready"),
            Until::Exit(code) => write!(f, "exit:{code}"),
            Until::Outcome(name) => write!(f, "outcome:{name}"),
            Until::Deadline(at) => write!(f, "deadline:{at}"),
        }
    }
}

/// The condition `text` writes; the rule broken otherwise is [`UNTIL_RULE`].
pub fn until(text: &str) -> Result<Until, String> {
    let refused = || UNTIL_RULE.to_owned();
    if text == "nothing-ready" {
        return Ok(Until::NothingReady);
    }
    match text.split_once(':') {
        Some(("exit", code)) => code
            .parse::<u8>()
            .map(|code| Until::Exit(i32::from(code)))
            .map_err(|_| refused()),
        Some(("outcome", name))
            if !name.is_empty()
                && name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.')) =>
        {
            Ok(Until::Outcome(name.to_owned()))
        }
        Some(("deadline", at)) => at
            .parse::<Timestamp>()
            .map(Until::Deadline)
            .map_err(|_| refused()),
        _ => Err(refused()),
    }
}

/// What a run file's `schedule` member sets, each member optional but `every`; a flag of
/// `takt run` wins over each.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Setting {
    /// `every`, in seconds.
    pub every: Option<u64>,
    /// `time_zone`.
    pub time_zone: Option<Zone>,
    /// `windows`.
    pub windows: Option<Vec<Window>>,
    /// `until`.
    pub until: Option<Vec<Until>>,
}

/// Why a run file's `schedule` member is refused: the path of keys to the member at fault, for
/// its line, the member's name and the rule it breaks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingRefused {
    /// The keys from the `schedule` member down to the one at fault.
    pub keys: Vec<String>,
    /// The member, as a refusal names it.
    pub member: &'static str,
    /// The rule it breaks.
    pub rule: &'static str,
}

fn static_rule(rule: &str) -> &'static str {
    [EVERY_RULE, ZONE_RULE, WINDOW_RULE, UNTIL_RULE]
        .into_iter()
        .find(|known| *known == rule)
        .unwrap_or(SCHEDULE_RULE)
}

impl Setting {
    /// The `schedule` member `value` holds, as written: a mapping of `every`, `time_zone`,
    /// `windows` and `until`, `every` among them; each refused naming the member and its rule.
    pub fn read(value: &serde_yaml_ng::Value) -> Result<Self, SettingRefused> {
        let refusal = |keys: &[&str], member: &'static str, rule: &str| SettingRefused {
            keys: keys.iter().map(|key| (*key).to_owned()).collect(),
            member,
            rule: static_rule(rule),
        };
        let Some(mapping) = value.as_mapping() else {
            return Err(refusal(&["schedule"], "schedule", SCHEDULE_RULE));
        };
        let mut setting = Setting::default();
        for (key, member) in mapping {
            let Some(key) = key.as_str() else {
                return Err(refusal(&["schedule"], "schedule", SCHEDULE_RULE));
            };
            let strings = |member: &serde_yaml_ng::Value| -> Option<Vec<String>> {
                member
                    .as_sequence()?
                    .iter()
                    .map(|item| item.as_str().map(str::to_owned))
                    .collect()
            };
            match key {
                "every" => {
                    let seconds = member
                        .as_str()
                        .ok_or_else(|| EVERY_RULE.to_owned())
                        .and_then(every)
                        .map_err(|rule| refusal(&["schedule", key], "schedule.every", &rule))?;
                    setting.every = Some(seconds);
                }
                "time_zone" => {
                    let zone = member
                        .as_str()
                        .ok_or_else(|| ZONE_RULE.to_owned())
                        .and_then(zone)
                        .map_err(|rule| refusal(&["schedule", key], "schedule.time_zone", &rule))?;
                    setting.time_zone = Some(zone);
                }
                "windows" => {
                    let windows = strings(member)
                        .ok_or_else(|| WINDOW_RULE.to_owned())
                        .and_then(|texts| texts.iter().map(|text| window(text)).collect())
                        .map_err(|rule| refusal(&["schedule", key], "schedule.windows", &rule))?;
                    setting.windows = Some(windows);
                }
                "until" => {
                    let conditions = strings(member)
                        .ok_or_else(|| UNTIL_RULE.to_owned())
                        .and_then(|texts| texts.iter().map(|text| until(text)).collect())
                        .map_err(|rule| refusal(&["schedule", key], "schedule.until", &rule))?;
                    setting.until = Some(conditions);
                }
                other => {
                    return Err(refusal(&["schedule", other], "schedule", SCHEDULE_RULE));
                }
            }
        }
        if setting.every.is_none() {
            return Err(refusal(&["schedule"], "schedule", SCHEDULE_RULE));
        }
        Ok(setting)
    }
}

/// A schedule resolved from the flags and the run file: the slots it names and when it stops.
#[derive(Debug, Clone, PartialEq)]
pub struct Schedule {
    /// The seconds between two slots.
    pub every: u64,
    /// The zone the windows are wall-clock in.
    pub zone: Zone,
    /// The windows; none, a slot every `every` from [`Schedule::anchor`].
    pub windows: Vec<Window>,
    /// The stop conditions.
    pub until: Vec<Until>,
    /// The instant a schedule without windows counts its slots from: its first start, whole
    /// seconds.
    pub anchor: Timestamp,
}

impl Schedule {
    /// The schedule as `schedule.json`'s `resolved` member holds it, the same for the same
    /// schedule however it was given; the anchor is not part of it.
    #[must_use]
    pub fn resolved(&self) -> serde_json::Value {
        serde_json::json!({
            "every": every_text(self.every),
            "time_zone": self.zone.name,
            "windows": self.windows.iter().map(|window| window.text.clone()).collect::<Vec<_>>(),
            "until": self.until.iter().map(ToString::to_string).collect::<Vec<_>>(),
        })
    }

    /// The earliest deadline among the stop conditions.
    #[must_use]
    pub fn deadline(&self) -> Option<Timestamp> {
        self.until
            .iter()
            .filter_map(|until| match until {
                Until::Deadline(at) => Some(*at),
                _ => None,
            })
            .min()
    }

    /// The next slot to run at `now`, after the slot `after` last ran (none for a schedule that
    /// has run none): the first slot later than `after` that is not in the past, and how many
    /// slots later than `after` are in the past, which are skipped, never queued: a tick still
    /// running at its slot, or a suspend over several, runs the next slot once. `None` when no
    /// slot is left.
    #[must_use]
    pub fn next_slot(&self, after: Option<Timestamp>, now: Timestamp) -> Option<(Timestamp, u64)> {
        if self.windows.is_empty() {
            return self.next_plain_slot(after, now);
        }
        let from = after.map_or(now, |after| after.min(now));
        let mut date = from
            .to_zoned(self.zone.tz.clone())
            .date()
            .yesterday()
            .ok()?;
        let mut skipped = 0;
        for _ in 0..HORIZON_DAYS + (now.as_second() - from.as_second()) / 86_400 {
            for slot in self.day_slots(date) {
                if after.is_some_and(|after| slot <= after) {
                    continue;
                }
                if slot >= now {
                    return Some((slot, skipped));
                }
                if after.is_some() {
                    skipped += 1;
                }
            }
            date = date.tomorrow().ok()?;
        }
        None
    }

    /// [`Schedule::next_slot`] without windows: every `every` from the anchor.
    fn next_plain_slot(
        &self,
        after: Option<Timestamp>,
        now: Timestamp,
    ) -> Option<(Timestamp, u64)> {
        let every = i64::try_from(self.every).ok()?;
        let anchor = self.anchor.as_second();
        // The index of the first slot at or after `second`.
        let index_at = |second: i64| -> i64 {
            if second <= anchor {
                0
            } else {
                (second - anchor + every - 1) / every
            }
        };
        let first_not_past = index_at(now.as_second() + i64::from(now.subsec_nanosecond() > 0));
        let (index, skipped) = match after {
            Some(after) => {
                let next = index_at(after.as_second() + 1);
                let index = next.max(first_not_past);
                (index, u64::try_from(index - next).unwrap_or(0))
            }
            None => (first_not_past, 0),
        };
        let slot = Timestamp::from_second(anchor.checked_add(index.checked_mul(every)?)?).ok()?;
        Some((slot, skipped))
    }

    /// The slots of the windows on `date` in the schedule's zone, in order, each once.
    fn day_slots(&self, date: Date) -> Vec<Timestamp> {
        let weekday = usize::try_from(date.weekday().to_monday_zero_offset()).unwrap_or(0);
        let mut slots: Vec<Timestamp> = Vec::new();
        for window in self.windows.iter().filter(|window| window.days[weekday]) {
            let end = window.end.unwrap_or(window.start);
            let mut second = window.start;
            while second <= end && second < DAY_SECONDS {
                if let Some(slot) = self.instant(date, second) {
                    slots.push(slot);
                }
                second += self.every;
            }
        }
        slots.sort();
        slots.dedup();
        slots
    }

    /// The instant of the wall-clock `second` after midnight of `date` in the schedule's zone:
    /// in a gap, the first valid instant after it; in a fold, the earlier.
    fn instant(&self, date: Date, second: u64) -> Option<Timestamp> {
        let time = Time::new(
            i8::try_from(second / 3600).ok()?,
            i8::try_from(second % 3600 / 60).ok()?,
            i8::try_from(second % 60).ok()?,
            0,
        )
        .ok()?;
        let wall: DateTime = date.to_datetime(time);
        let tz = &self.zone.tz;
        match tz.to_ambiguous_timestamp(wall).offset() {
            AmbiguousOffset::Unambiguous { offset } => offset.to_timestamp(wall).ok(),
            AmbiguousOffset::Fold { before, .. } => before.to_timestamp(wall).ok(),
            AmbiguousOffset::Gap { after, .. } => {
                let early = after.to_timestamp(wall).ok()?;
                tz.following(early)
                    .next()
                    .map(|transition| transition.timestamp())
            }
        }
    }
}

/// The id of a schedule: a digest of its resolved form and the argument list each tick is given.
#[must_use]
pub fn id_of(resolved: &serde_json::Value, args: &[String]) -> String {
    let mut hasher = sha2::Sha256::new();
    hasher.update(resolved.to_string().as_bytes());
    for arg in args {
        hasher.update([0]);
        hasher.update(arg.as_bytes());
    }
    let digest = hasher.finalize();
    digest
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// The directory schedules keep their files in, under the state directory `given` (a relative
/// one read against the current directory), else under `$XDG_STATE_HOME/takt`
/// (`~/.local/state/takt` without it): `<state dir>/schedule`.
pub fn root(given: Option<&Path>) -> anyhow::Result<PathBuf> {
    let state = match given {
        Some(dir) => std::path::absolute(dir)?,
        None => crate::promotion::state_dir()?.join("takt"),
    };
    Ok(state.join("schedule"))
}

/// One tick as `ticks.jsonl` records it, `takt.schedule-tick/1`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tick {
    /// The slot it ran for.
    pub slot: Timestamp,
    /// When its child started.
    pub started: Timestamp,
    /// When its child ended.
    pub ended: Timestamp,
    /// The exit its child ended with, 128 and the signal for one a signal ended; none when
    /// neither is known.
    pub exit: Option<i32>,
    /// The path of the `takt.run-outcome/1` it was asked to write.
    pub outcome: PathBuf,
    /// The slots skipped before it.
    pub skipped: u64,
}

/// The files of one schedule, under `<state dir>/schedule/<id>/`: `schedule.json`,
/// `ticks.jsonl`, `ticks/<slot>/` and `end.json`.
#[derive(Debug, Clone)]
pub struct Files {
    /// The schedule's id ([`id_of`]).
    pub id: String,
    /// `<state dir>/schedule/<id>`.
    pub dir: PathBuf,
}

impl Files {
    /// The files of the schedule `resolved` whose ticks are given `args`, under `root`
    /// ([`root`]); the directory is made.
    pub fn at(root: &Path, resolved: &serde_json::Value, args: &[String]) -> std::io::Result<Self> {
        let id = id_of(resolved, args);
        let dir = root.join(&id);
        std::fs::create_dir_all(dir.join("ticks"))?;
        Ok(Files { id, dir })
    }

    /// `schedule.json`.
    #[must_use]
    pub fn schedule_file(&self) -> PathBuf {
        self.dir.join("schedule.json")
    }

    fn earlier(&self) -> serde_json::Value {
        std::fs::read_to_string(self.schedule_file())
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    /// The pid an earlier start of the schedule runs as, while it lives: `schedule.json` names
    /// it, it is not this process, no `end.json` was written after it, and it is alive.
    #[must_use]
    pub fn running(&self) -> Option<u32> {
        let pid = self.earlier()["pid"]
            .as_u64()
            .and_then(|pid| u32::try_from(pid).ok())?;
        (pid != std::process::id() && !self.dir.join("end.json").exists() && alive(pid))
            .then_some(pid)
    }

    /// The anchor an earlier start of the schedule counted its slots from.
    #[must_use]
    pub fn anchor(&self) -> Option<Timestamp> {
        self.earlier()["anchor"].as_str()?.parse().ok()
    }

    /// The latest slot `ticks.jsonl` holds.
    #[must_use]
    pub fn last_slot(&self) -> Option<Timestamp> {
        std::fs::read_to_string(self.dir.join("ticks.jsonl"))
            .unwrap_or_default()
            .lines()
            .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .filter_map(|tick| tick["slot"].as_str()?.parse::<Timestamp>().ok())
            .max()
    }

    /// Starts the schedule: an earlier `end.json` removed, `schedule.json` written with the
    /// resolved schedule, its anchor, the ticks' arguments, this pid and the instant.
    pub fn start(&self, schedule: &Schedule, args: &[String]) -> std::io::Result<()> {
        match std::fs::remove_file(self.dir.join("end.json")) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => return Err(error),
            _ => {}
        }
        let document = serde_json::json!({
            "format": SCHEDULE_FORMAT,
            "id": self.id,
            "resolved": schedule.resolved(),
            "anchor": schedule.anchor.to_string(),
            "args": args,
            "pid": std::process::id(),
            "started": Timestamp::now().to_string(),
        });
        std::fs::write(self.schedule_file(), format!("{document:#}\n"))
    }

    /// Appends `tick` to `ticks.jsonl`.
    pub fn record(&self, tick: &Tick) -> std::io::Result<()> {
        use std::io::Write as _;
        let line = serde_json::json!({
            "format": TICK_FORMAT,
            "slot": tick.slot.to_string(),
            "started": tick.started.to_string(),
            "ended": tick.ended.to_string(),
            "exit": tick.exit,
            "outcome": tick.outcome.display().to_string(),
            "skipped": tick.skipped,
        });
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.dir.join("ticks.jsonl"))?;
        writeln!(file, "{line}")
    }

    /// Writes `end.json`: the reason, the condition that held when one did, the exit.
    pub fn end(&self, reason: &str, until: Option<&Until>, exit: u8) -> std::io::Result<()> {
        let document = serde_json::json!({
            "format": END_FORMAT,
            "reason": reason,
            "until": until.map(ToString::to_string),
            "exit": exit,
            "ended": Timestamp::now().to_string(),
        });
        std::fs::write(self.dir.join("end.json"), format!("{document}\n"))
    }
}

/// The first of `until` that a tick which exited `exit` and wrote the outcome file at
/// `outcome` makes hold; a deadline is not read here ([`Schedule::next_slot`]).
#[must_use]
pub fn held<'a>(until: &'a [Until], exit: Option<i32>, outcome: &Path) -> Option<&'a Until> {
    let observed: serde_json::Value = std::fs::read_to_string(outcome)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    until.iter().find(|until| match until {
        Until::NothingReady => observed["frontier"] == "nothing_ready",
        Until::Exit(code) => exit == Some(*code),
        Until::Outcome(name) => observed["outcome"].as_str() == Some(name.as_str()),
        Until::Deadline(_) => false,
    })
}

/// The directory of a tick's files under the schedule's directory: `ticks/<slot>`, the slot in
/// UTC as `YYYYMMDDTHHMMSSZ`.
#[must_use]
pub fn tick_dir(schedule_dir: &Path, slot: Timestamp) -> PathBuf {
    schedule_dir
        .join("ticks")
        .join(slot.strftime("%Y%m%dT%H%M%SZ").to_string())
}

/// What a run ended with, for `--outcome-file`: the stop's kind, the case, the outcome the case
/// reached and whether the frontier had nothing ready. The exit and the instants are given when
/// it is written.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Observed {
    /// The kind of the stop (`Completed`, `Iterations`, `error`), or none.
    pub stop: Option<String>,
    /// The case id, or none.
    pub case: Option<String>,
    /// The outcome the case reached, or none.
    pub outcome: Option<String>,
    /// Whether the frontier had nothing ready.
    pub nothing_ready: bool,
}

impl Observed {
    /// The `takt.run-outcome/1` document for a run that exited `exit`.
    #[must_use]
    pub fn document(&self, exit: u8, started: Timestamp, ended: Timestamp) -> serde_json::Value {
        serde_json::json!({
            "format": OUTCOME_FORMAT,
            "exit": exit,
            "stop": self.stop,
            "case": self.case,
            "outcome": self.outcome,
            "frontier": self.nothing_ready.then_some("nothing_ready"),
            "started": started.to_string(),
            "ended": ended.to_string(),
        })
    }
}

/// Whether the process `pid` is alive: it exists, whoever owns it.
#[must_use]
pub fn alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        let Ok(pid) = libc::pid_t::try_from(pid) else {
            return false;
        };
        // SAFETY: signal 0 sends nothing; it only asks whether the process exists.
        let answer = unsafe { libc::kill(pid, 0) };
        answer == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        false
    }
}

/// SIGTERM and SIGINT, counted: the resident schedule reads the count at its boundaries and
/// forwards a second signal to its tick.
pub mod signals {
    use std::sync::atomic::{AtomicUsize, Ordering};

    static RECEIVED: AtomicUsize = AtomicUsize::new(0);

    #[cfg(unix)]
    extern "C" fn count(_: libc::c_int) {
        RECEIVED.fetch_add(1, Ordering::SeqCst);
    }

    /// Installs the counter for SIGTERM and SIGINT, in place of their default, which ends the
    /// process.
    pub fn install() {
        #[cfg(unix)]
        for signal in [libc::SIGTERM, libc::SIGINT] {
            // SAFETY: an all-zero `sigaction` is a valid value of the C struct.
            let mut action: libc::sigaction = unsafe { std::mem::zeroed() };
            action.sa_sigaction = count as extern "C" fn(libc::c_int) as libc::sighandler_t;
            action.sa_flags = libc::SA_RESTART;
            // SAFETY: the handler only touches an atomic, which is async-signal-safe.
            unsafe { libc::sigaction(signal, &raw const action, std::ptr::null_mut()) };
        }
    }

    /// The signals received since [`install`].
    #[must_use]
    pub fn received() -> usize {
        RECEIVED.load(Ordering::SeqCst)
    }

    /// Sends SIGTERM to the process `pid`.
    pub fn terminate(pid: u32) {
        #[cfg(unix)]
        if let Ok(pid) = libc::pid_t::try_from(pid) {
            // SAFETY: sending a signal to a process the caller started.
            unsafe { libc::kill(pid, libc::SIGTERM) };
        }
        #[cfg(not(unix))]
        let _ = pid;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn berlin() -> Schedule {
        Schedule {
            every: every("30m").expect("30m"),
            zone: zone("Europe/Berlin").expect("a zone"),
            windows: vec![
                window("Mon-Tue 07:00-22:30").expect("a window"),
                window("Wed 07:30").expect("a window"),
            ],
            until: Vec::new(),
            anchor: "2026-11-16T00:00:00Z".parse().expect("an instant"),
        }
    }

    fn at(text: &str) -> Timestamp {
        text.parse().expect("an instant")
    }

    /// Every slot later than `from` and before `to`, each as `next_slot` gives it.
    fn slots(schedule: &Schedule, from: Timestamp, to: Timestamp) -> Vec<Timestamp> {
        let mut found = Vec::new();
        let mut after = Some(from);
        while let Some((slot, _)) = schedule.next_slot(after, from) {
            if slot >= to {
                break;
            }
            found.push(slot);
            after = Some(slot);
        }
        found
    }

    #[test]
    fn run_schedule_slots_berlin() {
        let schedule = berlin();
        // 2026-11-16 is a Monday; Berlin is at +01:00 in November.
        let days = [
            ("2026-11-15T23:00:00Z", "2026-11-16T23:00:00Z", 32),
            ("2026-11-16T23:00:00Z", "2026-11-17T23:00:00Z", 32),
            ("2026-11-17T23:00:00Z", "2026-11-18T23:00:00Z", 1),
            ("2026-11-18T23:00:00Z", "2026-11-19T23:00:00Z", 0),
        ];
        for (from, to, count) in days {
            // `from` itself is midnight, never a slot here, so "later than" loses nothing.
            let found = slots(&schedule, at(from), at(to));
            assert_eq!(found.len(), count, "{from}: {found:?}");
        }
        let monday = slots(
            &schedule,
            at("2026-11-15T23:00:00Z"),
            at("2026-11-16T23:00:00Z"),
        );
        assert_eq!(monday[0], at("2026-11-16T06:00:00Z"), "07:00 in Berlin");
        assert_eq!(monday[31], at("2026-11-16T21:30:00Z"), "22:30 in Berlin");
        let wednesday = slots(
            &schedule,
            at("2026-11-17T23:00:00Z"),
            at("2026-11-18T23:00:00Z"),
        );
        assert_eq!(
            wednesday,
            vec![at("2026-11-18T06:30:00Z")],
            "07:30 in Berlin"
        );
    }

    #[test]
    fn run_schedule_slots_dst() {
        // 2026-03-29, a Sunday: Berlin's clocks go from 02:00 to 03:00 (01:00Z).
        let spring = Schedule {
            every: every("30m").expect("30m"),
            zone: zone("Europe/Berlin").expect("a zone"),
            windows: vec![window("Sun 01:30-03:30").expect("a window")],
            until: Vec::new(),
            anchor: at("2026-03-01T00:00:00Z"),
        };
        let found = slots(
            &spring,
            at("2026-03-28T23:00:00Z"),
            at("2026-03-29T22:00:00Z"),
        );
        assert_eq!(
            found,
            vec![
                at("2026-03-29T00:30:00Z"),
                at("2026-03-29T01:00:00Z"),
                at("2026-03-29T01:30:00Z"),
            ],
            "01:30, then 02:00 and 02:30 in the gap moved to 03:00 and taken once with it, 03:30"
        );
        // 2026-10-25, a Sunday: Berlin's clocks go from 03:00 back to 02:00 (01:00Z).
        let autumn = Schedule {
            windows: vec![window("Sun 02:00-02:30").expect("a window")],
            ..spring.clone()
        };
        let found = slots(
            &autumn,
            at("2026-10-24T22:00:00Z"),
            at("2026-10-25T23:00:00Z"),
        );
        assert_eq!(
            found,
            vec![at("2026-10-25T00:00:00Z"), at("2026-10-25T00:30:00Z")],
            "02:00 and 02:30 each once, at the earlier instant (+02:00)"
        );
    }

    #[test]
    fn run_schedule_slots_without_window() {
        let schedule = Schedule {
            every: every("10m").expect("10m"),
            zone: utc(),
            windows: Vec::new(),
            until: Vec::new(),
            anchor: at("2026-11-12T12:00:00Z"),
        };
        let noon = at("2026-11-12T12:00:00Z");
        assert_eq!(
            schedule.next_slot(None, noon),
            Some((noon, 0)),
            "the start is a slot"
        );
        let found = slots(&schedule, noon, at("2026-11-12T12:31:00Z"));
        assert_eq!(
            found,
            vec![
                at("2026-11-12T12:10:00Z"),
                at("2026-11-12T12:20:00Z"),
                at("2026-11-12T12:30:00Z"),
            ]
        );
    }

    #[test]
    fn run_schedule_missed_slots_run_once() {
        let schedule = Schedule {
            every: every("10m").expect("10m"),
            zone: utc(),
            windows: Vec::new(),
            until: Vec::new(),
            anchor: at("2026-11-12T12:00:00Z"),
        };
        // The slot 12:00 ran; the next, 12:10, is due, and the process wakes at 13:00, five
        // slots late: 12:10 to 12:50 are skipped, and 13:00, the first not in the past, runs.
        let late = at("2026-11-12T13:00:00Z");
        assert_eq!(
            schedule.next_slot(Some(at("2026-11-12T12:00:00Z")), late),
            Some((late, 5))
        );
        // Woken between two slots, the next one ahead runs; the five before it are skipped.
        assert_eq!(
            schedule.next_slot(Some(at("2026-11-12T12:00:00Z")), at("2026-11-12T12:55:00Z")),
            Some((at("2026-11-12T13:00:00Z"), 5))
        );
        // A windowed schedule counts its skipped slots the same way.
        let berlin = berlin();
        assert_eq!(
            berlin.next_slot(Some(at("2026-11-16T06:00:00Z")), at("2026-11-16T08:30:00Z")),
            Some((at("2026-11-16T08:30:00Z"), 4))
        );
    }

    #[test]
    fn a_deadline_with_windows_ends_the_slots_and_without_one_they_repeat_weekly() {
        let schedule = berlin();
        let thursday = at("2026-11-19T12:00:00Z");
        assert_eq!(
            schedule.next_slot(Some(thursday), thursday),
            Some((at("2026-11-23T06:00:00Z"), 0)),
            "the next Monday's 07:00"
        );
    }

    #[test]
    fn the_parts_parse_and_refuse() {
        assert_eq!(every("1h30m"), Ok(5400));
        assert_eq!(every_text(5400), "1h30m");
        assert_eq!(every_text(2), "2s");
        for bad in ["", "0s", "500ms", "30", "m", "-1s", "1.5s"] {
            assert!(every(bad).is_err(), "{bad}");
        }
        assert!(zone("Europe/Berlin").is_ok());
        assert!(zone("UTC").is_ok());
        assert!(zone("Mars/Olympus").is_err());
        assert!(window("Mon,Wed 09:00").is_ok());
        assert!(window("Sat-Mon 09:00-10:00").is_ok());
        for bad in [
            "Mon",
            "Monday 07:00",
            "Mon 7:00",
            "Mon 24:00",
            "Mon 10:00-09:00",
            "Mon 07:00 x",
        ] {
            assert!(window(bad).is_err(), "{bad}");
        }
        assert_eq!(until("exit:3"), Ok(Until::Exit(3)));
        assert_eq!(
            until("outcome:released"),
            Ok(Until::Outcome("released".into()))
        );
        assert!(until("deadline:2026-11-19T00:00:00+01:00").is_ok());
        for bad in ["nothing", "exit:x", "outcome:", "deadline:tomorrow"] {
            assert!(until(bad).is_err(), "{bad}");
        }
    }
}
