mod app;
mod app_install;
mod drawing;
mod export;
mod mobile_server;
mod prompt;
mod skill;

use clap::{Parser, Subcommand, ValueEnum};
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
            let path = app_install::install_macos_app(&home_dir, &executable_path)?;
            println!("Installed macOS app at {}", path.display());
            Ok(())
        }
        None => run_app(cli.drawings_dir.unwrap_or_else(default_drawings_dir)),
    }
}

fn default_drawings_dir() -> PathBuf {
    let Some(home_dir) = home::home_dir() else {
        return PathBuf::from("drawings");
    };
    let Ok(executable_path) = std::env::current_exe() else {
        return PathBuf::from("drawings");
    };
    default_drawings_dir_for_executable(&executable_path, &home_dir)
}

fn default_drawings_dir_for_executable(
    executable_path: &std::path::Path,
    home_dir: &std::path::Path,
) -> PathBuf {
    if is_macos_app_bundle_executable(executable_path) {
        home_dir.join("Pictures").join("GoghMode").join("drawings")
    } else {
        PathBuf::from("drawings")
    }
}

fn is_macos_app_bundle_executable(executable_path: &std::path::Path) -> bool {
    let components: Vec<_> = executable_path
        .components()
        .map(|component| component.as_os_str())
        .collect();
    components.windows(4).any(|window| {
        window[0] == "GoghMode.app" && window[1] == "Contents" && window[2] == "MacOS"
    })
}

fn run_app(drawings_dir: PathBuf) -> anyhow::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 760.0])
            .with_min_inner_size([720.0, 480.0]),
        ..Default::default()
    };

    eframe::run_native(
        "GoghMode",
        native_options,
        Box::new(move |_creation_context| Ok(Box::new(app::GoghModeApp::new(drawings_dir)))),
    )
    .map_err(|error| anyhow::anyhow!("{error}"))?;
    Ok(())
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

    #[test]
    fn default_drawings_dir_uses_project_directory_for_terminal_binary() {
        let executable = Path::new("/tmp/goghmode/target/release/goghmode");
        let home = Path::new("/Users/example");

        assert_eq!(
            default_drawings_dir_for_executable(executable, home),
            PathBuf::from("drawings")
        );
    }

    #[test]
    fn default_drawings_dir_uses_pictures_for_macos_app_bundle() {
        let executable =
            Path::new("/Users/example/Applications/GoghMode.app/Contents/MacOS/GoghMode");
        let home = Path::new("/Users/example");

        assert_eq!(
            default_drawings_dir_for_executable(executable, home),
            PathBuf::from("/Users/example/Pictures/GoghMode/drawings")
        );
    }
}
