use camino::Utf8PathBuf;
use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Clone, Debug, Parser)]
#[command(
    name = "skill-kits",
    version,
    about = "Local-first AI Agent Skills manager"
)]
pub struct Cli {
    #[arg(long, help = "Launch the native GUI instead of the CLI")]
    pub gui: bool,
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Clone, Debug, Subcommand)]
pub enum Command {
    List {
        #[arg(long, value_enum, default_value_t = ListKind::Skill)]
        kind: ListKind,
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
    Enable {
        query: String,
    },
    Disable {
        query: String,
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
    Plugin {
        #[command(subcommand)]
        command: PluginCommand,
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

#[derive(Clone, Debug, Subcommand)]
pub enum PluginCommand {
    List {
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
    Status {
        query: String,
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
    Enable {
        query: String,
    },
    Disable {
        query: String,
    },
    Scan {
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
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
    pub skill: Option<String>,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum ListKind {
    Skill,
    Plugin,
    RuntimeCapability,
}
