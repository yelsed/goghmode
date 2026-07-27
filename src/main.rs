mod app;
mod app_install;
mod crypto;
mod drawing;
mod export;
mod host;
mod mobile_server;
mod pages;
mod prompt;
mod protocol;
mod skill;

use clap::{Parser, Subcommand, ValueEnum};
use mobile_server::StartOutcome;
use prompt::PromptTarget;
use skill::SkillTarget;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "goghmode")]
#[command(about = "Draw in a native Rust app and save files AI tools can inspect")]
struct Cli {
    #[arg(long)]
    drawings_dir: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    Prompt {
        #[arg(long, value_enum, default_value_t = PromptTargetArg::Generic)]
        target: PromptTargetArg,
    },
    InstallSkill {
        #[arg(long, value_enum, default_value_t = SkillTargetArg::Claude)]
        target: SkillTargetArg,
    },
    InstallApp,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum PromptTargetArg {
    Generic,
    Claude,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum SkillTargetArg {
    Claude,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Some(Command::Prompt { target }) => {
            println!("{}", prompt::prompt_text(target.into()));
            Ok(())
        }
        Some(Command::InstallSkill { target }) => {
            let home_dir =
                home::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;
            let path = skill::install_skill(target.into(), &home_dir)?;
            println!("Installed Claude skill at {}", path.display());
            Ok(())
        }
        Some(Command::InstallApp) => {
            let home_dir =
                home::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;
            let executable_path = std::env::current_exe()?;
            let path = app_install::install_app(&home_dir, &executable_path)?;
            println!("Installed GoghMode at {}", path.display());
            Ok(())
        }
        None => run_app(cli.drawings_dir.unwrap_or_else(default_drawings_dir)),
    }
}

/// One location, whatever launched the app. This used to depend on whether the
/// executable lived inside GoghMode.app, which meant a terminal launch wrote to
/// `drawings/` relative to the current directory: three shells in three
/// directories produced three unrelated drawing histories, and the agent reading
/// `latest.*` had no way to know which one was current.
///
/// Pass `--drawings-dir` to put a drawing somewhere else on purpose.
fn default_drawings_dir() -> PathBuf {
    let Some(home_dir) = home::home_dir() else {
        return PathBuf::from("drawings");
    };
    drawings_dir_in_home(&home_dir)
}

fn drawings_dir_in_home(home_dir: &std::path::Path) -> PathBuf {
    home_dir.join("Pictures").join("GoghMode").join("drawings")
}

fn run_app(drawings_dir: PathBuf) -> anyhow::Result<()> {
    let native_options = native_options();
    let goghmode_dir = host::goghmode_dir(&home::home_dir().unwrap_or_else(std::env::temp_dir));
    // Loading fails only when there is no secure random source. Starting anyway
    // would mean a host that cannot pair and cannot say why, so this is loud.
    let host = host::SharedHost::load(&goghmode_dir)?;

    let bridge = match mobile_server::MobileServer::start(&drawings_dir, host.clone()) {
        StartOutcome::Running(server) => app::Bridge::Serving(server),
        // A second window would mean a second server competing for the same
        // drawings directory, and every device has the first one's address
        // saved. Hand over instead.
        StartOutcome::AlreadyRunning => {
            println!("GoghMode is already running on port {}.", mobile_server::DEFAULT_PORT);
            return Ok(());
        }
        StartOutcome::PortHeldByAnother => app::Bridge::Unavailable(format!(
            "Port {} is held by another program, so no device can reach this host. Free it, then reopen GoghMode.",
            mobile_server::DEFAULT_PORT
        )),
    };

    eframe::run_native(
        "GoghMode",
        native_options,
        Box::new(move |creation_context| {
            app::install_theme(&creation_context.egui_ctx);
            Ok(Box::new(app::GoghModeApp::new(
                drawings_dir,
                host,
                goghmode_dir,
                bridge,
            )))
        }),
    )
    .map_err(|error| anyhow::anyhow!("{error}"))?;
    Ok(())
}

// There is deliberately no "bring the running window forward" step here.
//
// `tell application "GoghMode" to activate` goes through LaunchServices, which
// runs the bundle's launcher, which `nohup`s another `goghmode-bin`. That
// instance finds the port taken, tries to be helpful, and activates again:
// thirty-four processes inside ten seconds, measured. Exiting is the guarantee;
// raising the window is not worth a spawn loop to get.

fn native_options() -> eframe::NativeOptions {
    eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 760.0])
            .with_min_inner_size([720.0, 480.0]),
        run_and_return: false,
        ..Default::default()
    }
}

impl From<PromptTargetArg> for PromptTarget {
    fn from(value: PromptTargetArg) -> Self {
        match value {
            PromptTargetArg::Generic => PromptTarget::Generic,
            PromptTargetArg::Claude => PromptTarget::Claude,
        }
    }
}

impl From<SkillTargetArg> for SkillTarget {
    fn from(value: SkillTargetArg) -> Self {
        match value {
            SkillTargetArg::Claude => SkillTarget::Claude,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// Terminal and Spotlight launches must land in the same place. When they did
    /// not, `/goghmode` could read a drawing from a different session entirely.
    #[test]
    fn drawings_dir_is_the_same_place_however_the_app_was_started() {
        let home = Path::new("/Users/example");

        assert_eq!(
            drawings_dir_in_home(home),
            PathBuf::from("/Users/example/Pictures/GoghMode/drawings")
        );
    }

    #[test]
    fn drawings_dir_is_absolute_so_it_cannot_follow_the_working_directory() {
        let home = Path::new("/Users/example");

        assert!(drawings_dir_in_home(home).is_absolute());
    }

    #[test]
    fn native_app_uses_non_returning_event_loop() {
        let options = native_options();

        assert!(!options.run_and_return);
    }
}
