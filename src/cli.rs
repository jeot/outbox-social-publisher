use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(name = "publo")]
#[command(about = "Local-first publishing CLI", long_about = None)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Commands,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Commands {
    Publish(PublishArgs),
    Auth(AuthArgs),
    Job(JobArgs),
    Worker(WorkerArgs),
    Init(InitArgs),
    Workspace(WorkspaceArgs),
    Paths,
    Serve(ServeArgs),
}

#[derive(Debug, Args)]
pub(crate) struct InitArgs {
    #[arg(long)]
    pub(crate) workspace_id: Option<String>,
    #[arg(long)]
    pub(crate) display_name: Option<String>,
}

#[derive(Debug, Args)]
pub(crate) struct ServeArgs {
    #[arg(long)]
    pub(crate) host: Option<String>,
    #[arg(long)]
    pub(crate) port: Option<u16>,
}

#[derive(Debug, Args)]
pub(crate) struct WorkspaceArgs {
    #[command(subcommand)]
    pub(crate) command: WorkspaceCommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum WorkspaceCommand {
    Switch(WorkspaceSwitchArgs),
}

#[derive(Debug, Args)]
pub(crate) struct WorkspaceSwitchArgs {
    #[arg(long)]
    pub(crate) workspace_id: String,
}

#[derive(Debug, Args)]
pub(crate) struct PublishArgs {
    #[command(subcommand)]
    pub(crate) platform: PublishPlatform,
}

#[derive(Debug, Subcommand)]
pub(crate) enum PublishPlatform {
    Linkedin(PublishLinkedinArgs),
    X(PublishXArgs),
    Substack(PublishSubstackArgs),
}

#[derive(Debug, Args)]
pub(crate) struct PublishLinkedinArgs {
    #[arg(long)]
    pub(crate) file: PathBuf,
    #[arg(long, required_unless_present = "debug")]
    pub(crate) pass: Option<String>,
    #[arg(long)]
    pub(crate) timeout_seconds: Option<u64>,
    #[arg(long, default_value_t = false)]
    pub(crate) allow_duplicate: bool,
    #[arg(long, default_value_t = false)]
    pub(crate) debug: bool,
    #[arg(long, default_value_t = false, conflicts_with = "no_signature")]
    pub(crate) add_signature: bool,
    #[arg(long, default_value_t = false, conflicts_with = "add_signature")]
    pub(crate) no_signature: bool,
}

#[derive(Debug, Args)]
pub(crate) struct PublishXArgs {
    #[arg(long)]
    pub(crate) file: PathBuf,
    #[arg(long, required_unless_present = "debug")]
    pub(crate) pass: Option<String>,
    #[arg(long)]
    pub(crate) timeout_seconds: Option<u64>,
    #[arg(long, default_value_t = false)]
    pub(crate) allow_duplicate: bool,
    #[arg(long, default_value_t = false)]
    pub(crate) allow_cashtag: bool,
    #[arg(long, default_value_t = false)]
    pub(crate) allow_length: bool,
    #[arg(long, default_value_t = false)]
    pub(crate) force: bool,
    #[arg(long, default_value_t = false)]
    pub(crate) debug: bool,
    #[arg(long, default_value_t = false, conflicts_with = "no_signature")]
    pub(crate) add_signature: bool,
    #[arg(long, default_value_t = false, conflicts_with = "add_signature")]
    pub(crate) no_signature: bool,
}

#[derive(Debug, Args)]
pub(crate) struct PublishSubstackArgs {
    #[arg(long)]
    pub(crate) file: PathBuf,
    #[arg(long, required_unless_present = "debug")]
    pub(crate) pass: Option<String>,
    #[arg(long)]
    pub(crate) timeout_seconds: Option<u64>,
    #[arg(long, default_value_t = false)]
    pub(crate) allow_duplicate: bool,
    #[arg(long, default_value_t = false)]
    pub(crate) debug: bool,
    #[arg(long, default_value_t = false, conflicts_with = "no_signature")]
    pub(crate) add_signature: bool,
    #[arg(long, default_value_t = false, conflicts_with = "add_signature")]
    pub(crate) no_signature: bool,
}

#[derive(Debug, Args)]
pub(crate) struct AuthArgs {
    #[command(subcommand)]
    pub(crate) platform: AuthPlatform,
}

#[derive(Debug, Subcommand)]
pub(crate) enum AuthPlatform {
    Linkedin(AuthLinkedinArgs),
    X(AuthXArgs),
    Substack(AuthSubstackArgs),
}

#[derive(Debug, Args)]
pub(crate) struct JobArgs {
    #[command(subcommand)]
    pub(crate) command: JobCommand,
}

#[derive(Debug, Args)]
pub(crate) struct WorkerArgs {
    #[command(subcommand)]
    pub(crate) command: WorkerCommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum WorkerCommand {
    Run(WorkerRunArgs),
}

#[derive(Debug, Args)]
pub(crate) struct WorkerRunArgs {
    #[arg(long, conflicts_with = "live")]
    pub(crate) dry_run: bool,
    #[arg(long, conflicts_with = "dry_run")]
    pub(crate) live: bool,
    #[arg(long, required_if_eq("live", "true"))]
    pub(crate) pass: Option<String>,
    #[arg(long, required = true)]
    pub(crate) once: bool,
}

#[derive(Debug, Subcommand)]
pub(crate) enum JobCommand {
    Ready(JobReadyArgs),
    ImportPublished(JobImportPublishedArgs),
    Unready(JobIdArgs),
    Schedule(JobScheduleArgs),
    Unschedule(JobUnscheduleArgs),
    AddSchedule(JobAddScheduleArgs),
    Cancel(JobCancelArgs),
    List(JobListArgs),
    Show(JobShowArgs),
    RunDebug(JobRunDebugArgs),
}

#[derive(Debug, Args)]
pub(crate) struct JobImportPublishedArgs {
    #[arg(long, value_enum, value_delimiter = ',', required = true)]
    pub(crate) platform: Vec<PlatformArg>,
    #[arg(long)]
    pub(crate) file: PathBuf,
    #[arg(long)]
    pub(crate) published_at: Option<String>,
    #[arg(long, requires = "published_at")]
    pub(crate) timezone: Option<String>,
    #[arg(long, value_enum, required = true)]
    pub(crate) by: OperatorArg,
    #[arg(long)]
    pub(crate) user_note: Option<String>,
    #[arg(long)]
    pub(crate) ai_note: Option<String>,
    #[arg(long)]
    pub(crate) ai_model: Option<String>,
}

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
pub(crate) enum PlatformArg {
    Linkedin,
    X,
}

impl PlatformArg {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            PlatformArg::Linkedin => "linkedin",
            PlatformArg::X => "x",
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum OperatorArg {
    User,
    Ai,
}

impl OperatorArg {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            OperatorArg::User => "user",
            OperatorArg::Ai => "ai",
        }
    }
}

#[derive(Debug, Args)]
pub(crate) struct JobReadyArgs {
    #[arg(long, value_enum, value_delimiter = ',')]
    pub(crate) platform: Vec<PlatformArg>,
    #[arg(long)]
    pub(crate) file: PathBuf,
    #[arg(long)]
    pub(crate) at: Option<String>,
    #[arg(long)]
    pub(crate) timezone: Option<String>,
    #[arg(long)]
    pub(crate) owner_user_id: Option<String>,
    #[arg(long, value_enum, default_value = "user")]
    pub(crate) by: OperatorArg,
    #[arg(long)]
    pub(crate) user_note: Option<String>,
    #[arg(long)]
    pub(crate) ai_note: Option<String>,
    #[arg(long)]
    pub(crate) ai_model: Option<String>,
}

#[derive(Debug, Args)]
pub(crate) struct JobIdArgs {
    #[arg(long)]
    pub(crate) id: String,
}

#[derive(Debug, Args)]
pub(crate) struct JobScheduleArgs {
    #[arg(long)]
    pub(crate) id: String,
    #[arg(long, value_enum)]
    pub(crate) platform: Option<PlatformArg>,
    #[arg(long)]
    pub(crate) at: String,
    #[arg(long)]
    pub(crate) timezone: Option<String>,
    #[arg(long, value_enum, default_value = "user")]
    pub(crate) by: OperatorArg,
    #[arg(long)]
    pub(crate) user_note: Option<String>,
    #[arg(long)]
    pub(crate) ai_note: Option<String>,
    #[arg(long)]
    pub(crate) ai_model: Option<String>,
}

#[derive(Debug, Args)]
pub(crate) struct JobUnscheduleArgs {
    #[arg(long)]
    pub(crate) id: String,
    #[arg(long)]
    pub(crate) reason: Option<String>,
}

#[derive(Debug, Args)]
pub(crate) struct JobAddScheduleArgs {
    #[arg(long, value_enum)]
    pub(crate) platform: PlatformArg,
    #[arg(long)]
    pub(crate) file: PathBuf,
    #[arg(long)]
    pub(crate) at: String,
    #[arg(long)]
    pub(crate) timezone: Option<String>,
    #[arg(long)]
    pub(crate) owner_user_id: Option<String>,
    #[arg(long, value_enum, default_value = "user")]
    pub(crate) by: OperatorArg,
    #[arg(long)]
    pub(crate) user_note: Option<String>,
    #[arg(long)]
    pub(crate) ai_note: Option<String>,
    #[arg(long)]
    pub(crate) ai_model: Option<String>,
}

#[derive(Debug, Args)]
pub(crate) struct JobCancelArgs {
    #[arg(long)]
    pub(crate) id: String,
    #[arg(long)]
    pub(crate) reason: Option<String>,
}

#[derive(Debug, Args)]
pub(crate) struct JobListArgs {
    #[arg(long)]
    pub(crate) status: Option<String>,
    #[arg(long, value_enum)]
    pub(crate) platform: Option<PlatformArg>,
    #[arg(long, default_value_t = 100)]
    pub(crate) limit: u32,
}

#[derive(Debug, Args)]
pub(crate) struct JobShowArgs {
    #[arg(long)]
    pub(crate) id: String,
}

#[derive(Debug, Args)]
pub(crate) struct JobRunDebugArgs {
    #[arg(long)]
    pub(crate) id: String,
}

#[derive(Debug, Args)]
pub(crate) struct AuthLinkedinArgs {
    #[command(subcommand)]
    pub(crate) command: AuthLinkedinCommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum AuthLinkedinCommand {
    Guide,
    Login,
    Exchange(AuthLinkedinExchangeArgs),
    Whoami,
    TokenStatus,
    TokenRefresh,
}

#[derive(Debug, Args)]
pub(crate) struct AuthXArgs {
    #[command(subcommand)]
    pub(crate) command: AuthXCommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum AuthXCommand {
    Login,
    Exchange(AuthXExchangeArgs),
    TokenStatus,
    TokenRefresh,
}

#[derive(Debug, Args)]
pub(crate) struct AuthSubstackArgs {
    #[command(subcommand)]
    pub(crate) command: AuthSubstackCommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum AuthSubstackCommand {
    Guide,
    SessionStatus,
    Whoami,
}

#[derive(Debug, Args)]
pub(crate) struct AuthLinkedinExchangeArgs {
    #[arg(long)]
    pub(crate) code: String,
    #[arg(long)]
    pub(crate) state: String,
}

#[derive(Debug, Args)]
pub(crate) struct AuthXExchangeArgs {
    #[arg(long)]
    pub(crate) code: String,
    #[arg(long)]
    pub(crate) state: String,
    #[arg(long)]
    pub(crate) code_verifier: Option<String>,
}
