match cli.command {
    Command::Flash {
        image,
        disk,
        uefi_dir,
        dry_run,
        auto_unmount,
        yes_i_know,
    } => {
        flash::run(&image, &disk, &uefi_dir, dry_run, auto_unmount, yes_i_know)?;
    }
}
