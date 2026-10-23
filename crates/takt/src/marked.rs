//! What a child left behind once it ended, on macOS: there is no subreaper there, so a process
//! the child started and orphaned becomes `launchd`'s, and is found instead by the mark
//! ([`crate::forge`]'s `TAKT_CHILD`) it inherited in its environment. Every process of this
//! user is listed (`proc_listallpids`), and each one's environment is read from the kernel
//! (`sysctl` `KERN_PROCARGS2`), never from a file.

use std::time::{Duration, Instant};

/// The processes of this user, but this one.
fn pids() -> Vec<libc::pid_t> {
    let me = libc::pid_t::try_from(std::process::id()).unwrap_or(0);
    // SAFETY: `getuid` takes nothing and touches no memory of this process.
    let uid = unsafe { libc::getuid() };
    let mut room = 1024_usize;
    loop {
        let mut found: Vec<libc::pid_t> = vec![0; room];
        let bytes = libc::c_int::try_from(room * std::mem::size_of::<libc::pid_t>())
            .unwrap_or(libc::c_int::MAX);
        // SAFETY: `proc_listallpids` writes at most `bytes` bytes of pids into the buffer and
        // answers how many it wrote.
        let count = unsafe { libc::proc_listallpids(found.as_mut_ptr().cast(), bytes) };
        let Ok(count) = usize::try_from(count) else {
            return Vec::new();
        };
        if count >= room {
            room *= 2;
            continue;
        }
        found.truncate(count);
        found.retain(|pid| *pid > 0 && *pid != me && owner(*pid) == Some(uid));
        return found;
    }
}

/// The user a process runs as.
fn owner(pid: libc::pid_t) -> Option<libc::uid_t> {
    // SAFETY: `proc_bsdinfo` is plain integers and bytes, for which all zeroes is a value.
    let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
    let size = libc::c_int::try_from(std::mem::size_of::<libc::proc_bsdinfo>()).ok()?;
    // SAFETY: `proc_pidinfo` writes at most `size` bytes into `info` and answers how many.
    let wrote =
        unsafe { libc::proc_pidinfo(pid, libc::PROC_PIDTBSDINFO, 0, (&raw mut info).cast(), size) };
    (wrote == size).then_some(info.pbi_uid)
}

/// A process's parent and process group; none for a process that is gone.
fn parent_and_group(pid: libc::pid_t) -> Option<(libc::pid_t, libc::pid_t)> {
    bsd_info(pid).map(|(parent, group, _)| (parent, group))
}

/// A process's parent, process group and whether it has ended and waits to be reaped (a
/// zombie, `SZOMB`): the kernel answers for a zombie too. None for a process that is gone.
fn bsd_info(pid: libc::pid_t) -> Option<(libc::pid_t, libc::pid_t, bool)> {
    // SAFETY: `proc_bsdinfo` is plain integers and bytes, for which all zeroes is a value.
    let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
    let size = libc::c_int::try_from(std::mem::size_of::<libc::proc_bsdinfo>()).ok()?;
    // SAFETY: `proc_pidinfo` writes at most `size` bytes into `info` and answers how many.
    let wrote =
        unsafe { libc::proc_pidinfo(pid, libc::PROC_PIDTBSDINFO, 0, (&raw mut info).cast(), size) };
    if wrote != size {
        return None;
    }
    Some((
        libc::pid_t::try_from(info.pbi_ppid).ok()?,
        libc::pid_t::try_from(info.pbi_pgid).ok()?,
        info.pbi_status == libc::SZOMB,
    ))
}

/// Kills and reaps, round after round, the children of this process that left its process
/// group for one no pid of `running` leads, or left its session: what a judgement step's
/// harness, or a validator that ran a program by itself, started in the background and left
/// once it returned, which carries no mark when it was not started through
/// [`crate::forge::run_child`]. A child another thread started in this process's own group and
/// session stays. Never taken, so never reaped: a child of `running`, which its
/// [`crate::forge::run_child`] call waits on, and a child that has already ended (a zombie),
/// whose exit status belongs to whoever waits on it, and whose session `getsid` no longer
/// answers. A child whose session is not answered is judged by its group alone. An error when
/// one is still there after the last round.
pub fn sweep_children(running: &std::collections::BTreeSet<u32>) -> anyhow::Result<()> {
    let me = libc::pid_t::try_from(std::process::id()).unwrap_or(0);
    // SAFETY: `getsid` and `getpgrp` take integers and touch no memory of this process.
    let (session, own) = unsafe { (libc::getsid(0), libc::getpgrp()) };
    let left = || -> Vec<libc::pid_t> {
        pids()
            .into_iter()
            .filter(|pid| !u32::try_from(*pid).is_ok_and(|pid| running.contains(&pid)))
            .filter(|pid| {
                bsd_info(*pid).is_some_and(|(parent, group, ended)| {
                    if parent != me || ended {
                        return false;
                    }
                    // SAFETY: `getsid` takes an integer and touches no memory of this process.
                    let sid = unsafe { libc::getsid(*pid) };
                    (sid != -1 && sid != session)
                        || (group != own
                            && !u32::try_from(group).is_ok_and(|g| running.contains(&g)))
                })
            })
            .collect()
    };
    for _ in 0..64 {
        let found = left();
        if found.is_empty() {
            return Ok(());
        }
        for pid in &found {
            // SAFETY: `kill` takes two integers and touches no memory of this process.
            unsafe {
                libc::kill(*pid, libc::SIGKILL);
            }
        }
        let until = Instant::now() + Duration::from_secs(2);
        let mut waiting = found;
        while !waiting.is_empty() && Instant::now() < until {
            waiting.retain(|pid| {
                let mut status = 0;
                // SAFETY: `waitpid` writes the status into the integer it is given.
                let reaped = unsafe { libc::waitpid(*pid, &raw mut status, libc::WNOHANG) };
                reaped == 0
            });
            if !waiting.is_empty() {
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    }
    anyhow::bail!("children this process left were still running after 64 rounds of killing")
}

/// When a process started, to the microsecond since the epoch; none for one that has ended.
fn started_at(pid: libc::pid_t) -> Option<std::time::SystemTime> {
    // SAFETY: `proc_bsdinfo` is plain integers and bytes, for which all zeroes is a value.
    let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
    let size = libc::c_int::try_from(std::mem::size_of::<libc::proc_bsdinfo>()).ok()?;
    // SAFETY: `proc_pidinfo` writes at most `size` bytes into `info` and answers how many.
    let wrote =
        unsafe { libc::proc_pidinfo(pid, libc::PROC_PIDTBSDINFO, 0, (&raw mut info).cast(), size) };
    (wrote == size).then(|| {
        std::time::UNIX_EPOCH
            + Duration::from_secs(info.pbi_start_tvsec)
            + Duration::from_micros(info.pbi_start_tvusec)
    })
}

/// Kills, round after round, the orphans in this process's own process group and session that
/// started at `since` or later: what a child started there, in that group, left running when
/// it ended, which `launchd` adopted (its parent is 1). A harness session runs its program in
/// this group on macOS, where its executor finds no `setsid`, so this finds what the program
/// left once the session ended. It takes this process's sweeps first (`forge::sweeping`), so
/// it waits for every child another thread holds them for. An error when one is still there
/// after the last round.
pub fn sweep_orphans_since(since: std::time::SystemTime) -> anyhow::Result<()> {
    let _sweeping = crate::forge::sweeping();
    // SAFETY: `getsid` and `getpgrp` take integers and touch no memory of this process.
    let (session, own) = unsafe { (libc::getsid(0), libc::getpgrp()) };
    let left = || -> Vec<libc::pid_t> {
        pids()
            .into_iter()
            .filter(|pid| {
                // SAFETY: `getsid` takes an integer and touches no memory of this process.
                let sid = unsafe { libc::getsid(*pid) };
                sid == session
                    && parent_and_group(*pid) == Some((1, own))
                    && started_at(*pid).is_some_and(|started| started >= since)
            })
            .collect()
    };
    for _ in 0..64 {
        let found = left();
        if found.is_empty() {
            return Ok(());
        }
        for pid in &found {
            // SAFETY: `kill` takes two integers and touches no memory of this process.
            unsafe {
                libc::kill(*pid, libc::SIGKILL);
            }
        }
        let until = Instant::now() + Duration::from_secs(2);
        let mut waiting = found;
        while !waiting.is_empty() && Instant::now() < until {
            waiting.retain(|pid| parent_and_group(*pid).is_some());
            if !waiting.is_empty() {
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    }
    anyhow::bail!("processes a harness session left were still running after 64 rounds of killing")
}

/// The environment a process was started with, each entry `NAME=value`; none for a process
/// that has ended (a zombie included) or that is not this user's.
fn environment(pid: libc::pid_t) -> Option<Vec<Vec<u8>>> {
    let mut most: libc::c_int = 0;
    let mut size = std::mem::size_of::<libc::c_int>();
    let mut name = [libc::CTL_KERN, libc::KERN_ARGMAX];
    // SAFETY: `sysctl` writes one integer into `most`, `size` bytes long.
    let read = unsafe {
        libc::sysctl(
            name.as_mut_ptr(),
            2,
            (&raw mut most).cast(),
            &raw mut size,
            std::ptr::null_mut(),
            0,
        )
    };
    if read != 0 {
        return None;
    }
    let mut buffer = vec![0_u8; usize::try_from(most).ok()?];
    let mut size = buffer.len();
    let mut name = [libc::CTL_KERN, libc::KERN_PROCARGS2, pid];
    // SAFETY: `sysctl` writes at most `size` bytes into the buffer and sets `size` to how many.
    let read = unsafe {
        libc::sysctl(
            name.as_mut_ptr(),
            3,
            buffer.as_mut_ptr().cast(),
            &raw mut size,
            std::ptr::null_mut(),
            0,
        )
    };
    if read != 0 {
        return None;
    }
    buffer.truncate(size);
    parse(&buffer)
}

/// The environment in a `KERN_PROCARGS2` answer: the argument count, the executable's path and
/// its padding of NULs, that many arguments, then the environment up to the first empty
/// string; what follows that is the loader's, not the environment.
fn parse(bytes: &[u8]) -> Option<Vec<Vec<u8>>> {
    let count = usize::try_from(i32::from_ne_bytes(bytes.get(..4)?.try_into().ok()?)).ok()?;
    let rest = bytes.get(4..)?;
    let path_end = rest.iter().position(|byte| *byte == 0)?;
    let mut rest = rest.get(path_end..)?;
    let start = rest.iter().position(|byte| *byte != 0)?;
    rest = rest.get(start..)?;
    let mut strings = rest.split(|byte| *byte == 0);
    for _ in 0..count {
        strings.next()?;
    }
    Some(
        strings
            .take_while(|entry| !entry.is_empty())
            .map(<[u8]>::to_vec)
            .collect(),
    )
}

/// The value of `name` in a process's environment, when it was started with it.
fn mark_of(pid: libc::pid_t, name: &str) -> Option<Vec<u8>> {
    let prefix = format!("{name}=");
    environment(pid)?
        .into_iter()
        .find_map(|entry| entry.strip_prefix(prefix.as_bytes()).map(<[u8]>::to_vec))
}

/// Kills, round after round, every process of this user whose `name` in its environment
/// `pick` takes: killing one can hand what it started on to `launchd` before it dies, which the
/// next round finds by the same mark. A killed process that is this one's child is reaped. An
/// error when one is still there after the last round.
pub fn sweep(name: &str, pick: impl Fn(&[u8]) -> bool) -> anyhow::Result<()> {
    for _ in 0..64 {
        let found: Vec<libc::pid_t> = pids()
            .into_iter()
            .filter(|pid| mark_of(*pid, name).is_some_and(|mark| pick(&mark)))
            .collect();
        if found.is_empty() {
            return Ok(());
        }
        for pid in &found {
            // SAFETY: `kill` takes two integers and touches no memory of this process.
            unsafe {
                libc::kill(*pid, libc::SIGKILL);
            }
        }
        let until = Instant::now() + Duration::from_secs(2);
        let mut left = found;
        while !left.is_empty() && Instant::now() < until {
            left.retain(|pid| {
                let mut status = 0;
                // SAFETY: `waitpid` writes the status into the integer it is given; for a
                // process that is not this one's child it answers an error and changes nothing.
                unsafe {
                    libc::waitpid(*pid, &raw mut status, libc::WNOHANG);
                }
                mark_of(*pid, name).is_some_and(|mark| pick(&mark))
            });
            if !left.is_empty() {
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    }
    anyhow::bail!("processes the child started were still running after 64 rounds of killing")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_environment_follows_the_arguments_and_ends_at_the_first_empty_string() {
        let mut bytes = 2_i32.to_ne_bytes().to_vec();
        bytes.extend_from_slice(b"/bin/sh\0\0\0\0sh\0-c\0A=1\0TAKT_CHILD=9-0\0\0ptr_munge=x\0");
        assert_eq!(
            parse(&bytes),
            Some(vec![b"A=1".to_vec(), b"TAKT_CHILD=9-0".to_vec()])
        );
    }

    #[test]
    fn this_process_reads_its_own_environment() {
        let me = libc::pid_t::try_from(std::process::id()).expect("a pid");
        let found = environment(me).expect("this process's environment");
        assert!(
            found.iter().any(|entry| entry.starts_with(b"PATH=")),
            "{found:?}"
        );
    }

    #[test]
    fn a_sweep_of_orphans_waits_for_the_sweeps_held() {
        let held = crate::forge::hold_sweeps();
        let (sent, swept) = std::sync::mpsc::channel();
        let since = std::time::SystemTime::now();
        let sweeper = std::thread::spawn(move || {
            let _ = sent.send(sweep_orphans_since(since).is_ok());
        });
        assert!(
            swept.recv_timeout(Duration::from_millis(300)).is_err(),
            "the sweep ran while the sweeps were held"
        );
        drop(held);
        assert_eq!(swept.recv_timeout(Duration::from_secs(30)), Ok(true));
        sweeper.join().expect("the sweeper");
    }
}
