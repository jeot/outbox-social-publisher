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
    Serve(ServeArgs),
}

#[derive(Debug, Args)]
pub(crate) struct ServeArgs {
    #[arg(long)]
    pub(crate) host: Option<String>,
    #[arg(long)]
    pub(crate) port: Option<u16>,
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
}

#[derive(Debug, Args)]
pub(crate) struct PublishLinkedinArgs {
    #[arg(long)]
    pub(crate) file: PathBuf,
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
pub(crate) struct AuthArgs {
    #[command(subcommand)]
    pub(crate) platform: AuthPlatform,
}

#[derive(Debug, Subcommand)]
pub(crate) enum AuthPlatform {
    Linkedin(AuthLinkedinArgs),
    X(AuthXArgs),
}

#[derive(Debug, Args)]
pub(crate) struct JobArgs {
    #[command(subcommand)]
    pub(crate) command: JobCommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum JobCommand {
    Ready(JobReadyArgs),
    Unready(JobIdArgs),
    Schedule(JobScheduleArgs),
    Unschedule(JobUnscheduleArgs),
    AddSchedule(JobAddScheduleArgs),
    Cancel(JobCancelArgs),
    List(JobListArgs),
    Show(JobShowArgs),
    RunDebug(JobRunDebugArgs),
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
    #[arg(long, value_enum)]
    pub(crate) platform: Option<PlatformArg>,
    #[arg(long)]
    pub(crate) file: PathBuf,
    #[arg(long, default_value = "default")]
    pub(crate) workspace_id: String,
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
    #[arg(long, default_value = "default")]
    pub(crate) workspace_id: String,
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
    pub(crate) limit: i64,
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
