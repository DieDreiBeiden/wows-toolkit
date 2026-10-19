//! Asking the game client to play a replay.
//!
//! On Windows the client takes a replay path on its command line, so the
//! toolkit runs `WorldOfWarships.exe <replay>` directly. Elsewhere the client
//! is a Windows binary that only runs under Proton, and Proton is started by
//! Steam: running the `.exe` from a Linux process does nothing useful. A game
//! directory inside a Steam library is therefore launched through Steam, as
//! `steam -applaunch 552990 <replay>`. The client accepts the host path
//! unchanged; a `Z:\...` form is rejected. Both front ends build the command
//! here so the rule has one implementation.

use std::ffi::OsString;
use std::path::Path;
use std::process::Command;

/// A Steam application id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SteamAppId(pub u32);

/// World of Warships' Steam app id.
pub const STEAM_APP_ID: SteamAppId = SteamAppId(552990);

/// The executable inside the game directory.
pub const GAME_EXE: &str = "WorldOfWarships.exe";

/// The Flatpak id of the Steam client, whose libraries live under
/// `~/.var/app/<id>` and whose `steam` is not on the host's PATH.
const STEAM_FLATPAK_ID: &str = "com.valvesoftware.Steam";

/// A program and its arguments, before it becomes a [`Command`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchPlan {
    pub program: OsString,
    pub args: Vec<OsString>,
}

impl LaunchPlan {
    pub fn into_command(self) -> Command {
        let mut command = Command::new(self.program);
        command.args(self.args);
        command
    }
}

/// The command that opens `replay` in the client installed at `wows_dir`.
pub fn replay_launch_command(wows_dir: &Path, replay: &Path) -> Command {
    replay_launch_plan(wows_dir, replay, cfg!(windows)).into_command()
}

/// What [`replay_launch_command`] runs, with the platform passed in so both
/// branches can be tested anywhere.
pub fn replay_launch_plan(wows_dir: &Path, replay: &Path, windows: bool) -> LaunchPlan {
    if !windows && let Some(steam) = steam_install(wows_dir) {
        let (program, mut args) = match steam {
            SteamInstall::Native => (OsString::from("steam"), Vec::new()),
            SteamInstall::Flatpak => (OsString::from("flatpak"), vec!["run".into(), STEAM_FLATPAK_ID.into()]),
        };
        args.push("-applaunch".into());
        args.push(STEAM_APP_ID.0.to_string().into());
        args.push(replay.as_os_str().to_owned());
        return LaunchPlan { program, args };
    }

    LaunchPlan { program: wows_dir.join(GAME_EXE).into_os_string(), args: vec![replay.as_os_str().to_owned()] }
}

/// How the Steam client that owns a game directory was installed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SteamInstall {
    Native,
    Flatpak,
}

/// Which Steam client `wows_dir` belongs to, if it sits in a Steam library
/// (`.../steamapps/common/<game>`).
fn steam_install(wows_dir: &Path) -> Option<SteamInstall> {
    let components: Vec<_> = wows_dir.components().map(|c| c.as_os_str().to_string_lossy().to_lowercase()).collect();
    let in_library = components.windows(2).any(|pair| pair[0] == "steamapps" && pair[1] == "common");
    if !in_library {
        return None;
    }
    let flatpak = components.windows(2).any(|pair| pair[0] == "app" && pair[1] == STEAM_FLATPAK_ID.to_lowercase());
    Some(if flatpak { SteamInstall::Flatpak } else { SteamInstall::Native })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(plan: &LaunchPlan) -> Vec<&str> {
        plan.args.iter().map(|a| a.to_str().unwrap()).collect()
    }

    #[test]
    fn windows_runs_the_client_directly() {
        let plan = replay_launch_plan(
            Path::new(r"C:\Games\World_of_Warships"),
            Path::new(r"C:\Games\World_of_Warships\replays\a.wowsreplay"),
            true,
        );
        assert_eq!(plan.program, Path::new(r"C:\Games\World_of_Warships").join(GAME_EXE).into_os_string());
        assert_eq!(args(&plan), [r"C:\Games\World_of_Warships\replays\a.wowsreplay"]);
    }

    #[test]
    fn a_steam_library_on_windows_still_runs_the_client_directly() {
        // Forward slashes, so the path also splits into components on the
        // host running the test and only the platform flag keeps Steam out.
        let dir = Path::new("C:/Program Files (x86)/Steam/steamapps/common/World of Warships");
        let replay = "C:/Program Files (x86)/Steam/steamapps/common/World of Warships/replays/a.wowsreplay";
        let plan = replay_launch_plan(dir, Path::new(replay), true);
        assert_eq!(plan.program, dir.join(GAME_EXE).into_os_string());
        assert_eq!(args(&plan), [replay]);
    }

    #[test]
    fn a_steam_library_on_linux_launches_through_steam() {
        let dir = Path::new("/home/me/.local/share/Steam/steamapps/common/World of Warships");
        let plan = replay_launch_plan(dir, &dir.join("replays/a b.wowsreplay"), false);
        assert_eq!(plan.program, "steam");
        assert_eq!(
            args(&plan),
            [
                "-applaunch",
                "552990",
                "/home/me/.local/share/Steam/steamapps/common/World of Warships/replays/a b.wowsreplay"
            ]
        );
    }

    #[test]
    fn a_flatpak_steam_library_launches_through_flatpak() {
        let dir = Path::new("/home/me/.var/app/com.valvesoftware.Steam/data/Steam/steamapps/common/World of Warships");
        let plan = replay_launch_plan(dir, &dir.join("replays/a.wowsreplay"), false);
        assert_eq!(plan.program, "flatpak");
        assert_eq!(&args(&plan)[..4], ["run", STEAM_FLATPAK_ID, "-applaunch", "552990"]);
    }

    #[test]
    fn a_game_outside_steam_keeps_the_direct_launch() {
        let dir = Path::new("/home/me/Games/wows");
        let plan = replay_launch_plan(dir, &dir.join("replays/a.wowsreplay"), false);
        assert_eq!(plan.program, dir.join(GAME_EXE).into_os_string());
        assert_eq!(args(&plan), ["/home/me/Games/wows/replays/a.wowsreplay"]);
    }
}
