use crate::{cli, core::paths::AppPaths, gui};

pub fn run() -> anyhow::Result<()> {
    if std::env::args().any(|arg| arg == "--gui") {
        return gui::run_native(AppPaths::default_user_paths()?);
    }

    cli::handlers::run_cli()
}
