//! Starting the sync service with the user's session, so it syncs while Pochette is
//! closed: an XDG autostart entry, which the desktops that follow the standard start
//! (GNOME, KDE Plasma, Xfce, Cinnamon, MATE, LXQt…). The entry being there is the setting.

use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

use relm4::gtk::glib;

fn path() -> PathBuf {
    glib::user_config_dir()
        .join("autostart")
        .join(format!("{}.Sync.desktop", crate::app_id()))
}

pub fn is_enabled() -> bool {
    path().exists()
}

pub fn set_enabled(enabled: bool) -> io::Result<()> {
    let path = path();
    if !enabled {
        return match fs::remove_file(&path) {
            Err(err) if err.kind() != io::ErrorKind::NotFound => Err(err),
            _ => Ok(()),
        };
    }
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(
        &path,
        format!(
            "[Desktop Entry]\n\
             Type=Application\n\
             Name=Pochette Sync\n\
             Comment=Shares your listening history with your other devices\n\
             Exec={} --sync\n\
             Icon={}\n\
             NoDisplay=true\n\
             Terminal=false\n\
             X-GNOME-Autostart-enabled=true\n",
            command(),
            crate::APP_ID,
        ),
    )
}

/// How to start this Pochette: by its name, if that's what the user's `PATH` finds,
/// which keeps working once it's updated, else by its path.
fn command() -> String {
    let Ok(exe) = env::current_exe() else {
        return "pochette".to_owned();
    };
    // Packages may wrap it, as Nix does, in a script next to it.
    let found_on_path = env::var_os("PATH").is_some_and(|paths| {
        env::split_paths(&paths).any(|dir| {
            fs::canonicalize(dir.join("pochette"))
                .ok()
                .and_then(|found| found.parent().map(Path::to_path_buf))
                .is_some_and(|found| Some(found.as_path()) == exe.parent())
        })
    });
    if found_on_path {
        "pochette".to_owned()
    } else {
        quote(&exe.to_string_lossy())
    }
}

/// `path`, as an `Exec` key takes it.
fn quote(path: &str) -> String {
    if path
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "/._-+".contains(c))
    {
        return path.to_owned();
    }
    let mut quoted = String::from("\"");
    for c in path.chars() {
        if matches!(c, '"' | '`' | '$' | '\\') {
            // Once for the quotes, once for the desktop file's strings.
            quoted.push_str("\\\\");
        }
        quoted.push(c);
    }
    quoted.push('"');
    quoted
}

#[cfg(test)]
mod tests {
    use super::quote;

    #[test]
    fn quotes_paths_for_exec() {
        assert_eq!(quote("/usr/bin/pochette"), "/usr/bin/pochette");
        assert_eq!(quote("/home/a b/pochette"), "\"/home/a b/pochette\"");
        assert_eq!(quote("/x$y/pochette"), "\"/x\\\\$y/pochette\"");
    }
}
