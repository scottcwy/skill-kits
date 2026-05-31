use camino::Utf8PathBuf;
use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Clone, Debug, Parser)]
#[command(
    name = "skill-kits",
    version,
    about = "Local-first AI Agent Skills manager"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Clone, Debug, Subcommand)]
pub enum Command {
    List {
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
    Status {
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
    Install {
        #[command(subcommand)]
        command: InstallCommand,
    },
    Uninstall {
        skill: String,
    },
    Scan {
        skill: Option<String>,
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
    Doctor {
        #[arg(long)]
        fix: bool,
    },
    Adopt {
        #[arg(long = "global-agent")]
        global_agent: String,
    },
    Project {
        #[command(subcommand)]
        command: ProjectCommand,
    },
}

#[derive(Clone, Debug, Subcommand)]
pub enum InstallCommand {
    Local { path: Utf8PathBuf },
}

#[derive(Clone, Debug, Subcommand)]
pub enum ProjectCommand {
    Status(ProjectStatusArgs),
    Adopt(ProjectAgentArgs),
    Deploy(ProjectSkillAgentArgs),
    Enable(ProjectSkillAgentArgs),
    Disable(ProjectSkillAgentArgs),
    Redeploy(ProjectRedeployArgs),
    Remove(ProjectRemoveArgs),
}

#[derive(Clone, Debug, Args)]
pub struct ProjectStatusArgs {
    #[arg(long)]
    pub project: Option<Utf8PathBuf>,
    #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
    pub format: OutputFormat,
}

#[derive(Clone, Debug, Args)]
pub struct ProjectAgentArgs {
    #[arg(long)]
    pub agent: String,
    #[arg(long)]
    pub project: Option<Utf8PathBuf>,
}

#[derive(Clone, Debug, Args)]
pub struct ProjectSkillAgentArgs {
    pub skill: String,
    #[arg(long)]
    pub agent: String,
    #[arg(long)]
    pub project: Option<Utf8PathBuf>,
}

#[derive(Clone, Debug, Args)]
#[command(group(
    clap::ArgGroup::new("redeploy_resolution")
        .args(["overwrite", "promote"])
        .multiple(false)
))]
pub struct ProjectRedeployArgs {
    pub skill: String,
    #[arg(long)]
    pub agent: String,
    #[arg(long)]
    pub project: Option<Utf8PathBuf>,
    #[arg(long)]
    pub overwrite: bool,
    #[arg(long)]
    pub promote: bool,
}

#[derive(Clone, Debug, Args)]
pub struct ProjectRemoveArgs {
    pub skill: String,
    #[arg(long)]
    pub agent: String,
    #[arg(long)]
    pub project: Option<Utf8PathBuf>,
    #[arg(long)]
    pub force: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum OutputFormat {
    Table,
    Json,
}
