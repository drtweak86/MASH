#[derive(Subcommand)]
pub enum Command {
    Flash {
        #[arg(long)]
        image: PathBuf,

        #[arg(long)]
        disk: String,

        #[arg(long)]
        uefi_dir: PathBuf,

        #[arg(long)]
        dry_run: bool,

        #[arg(long)]
        auto_unmount: bool,

        #[arg(long)]
        yes_i_know: bool,
    },
}
