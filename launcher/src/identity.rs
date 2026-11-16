//! Repository-selected identity. An upstream build retains its original namespaces.
macro_rules! identity {
    ($($name:ident: $upstream:literal => $fork:literal),* $(,)?) => {$ (
        pub const $name: &str = if cfg!(sunrise_fork_launcher) { $fork } else { $upstream };
    )*};
}
identity! {
    VERSION_PREFIX: "horizon-launcher " => "sunrise-launcher ",
    DESKTOP_ENTRY: "horizon-launcher.desktop" => "net.playsunrise.launcher.desktop",
    WINDOW_TITLE: "Horizon Launcher" => "Sunrise Launcher",
    QUIT_LABEL: "Quit Horizon Launcher" => "Quit Sunrise Launcher",
    DISPLAY_NAME: "Horizon Launcher" => "Sunrise Launcher",
    // The installed copy's folder, shortcuts and Add/Remove entry carry Sunrise's own name, so it
    // never takes a Horizon install's place.
    INSTALL_NAME: "Horizon Launcher" => "Sunrise Launcher",
    EXECUTABLE: "horizon-launcher" => "sunrise-launcher",
    WINDOWS_EXECUTABLE: "horizon-launcher.exe" => "sunrise-launcher.exe",
    WINDOWS_EXECUTABLE_PREFIX: "horizon-launcher.exe." => "sunrise-launcher.exe.",
    APP_ID: "horizon-launcher" => "net.playsunrise.launcher",
    // Sunrise's own data, game and location folders, all under `sunrise/`, and saved accounts.
    // On its first start they are copied from the launchers before it (`carry_over`).
    KEYCHAIN_SERVICE: "dev.playhorizon.launcher" => "net.playsunrise.launcher",
    LOCATION_FOLDER: "horizon-launcher-location" => "sunrise/location",
    LAUNCHER_FOLDER: "horizon-launcher" => "sunrise/launcher",
    GAME_FOLDER: "horizon" => "sunrise/game",
    PUBLISHER: "Horizon" => "Sunrise",
    UNINSTALL_KEY: "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\HorizonLauncher" => "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\SunriseLauncher",
    INSTALL_KEY: "Software\\Horizon\\Launcher" => "Software\\Sunrise\\Launcher",
    REGISTRY_PARENT: "Software\\Horizon" => "Software\\Sunrise",
    UNINSTALL_PREFIX: "horizon-launcher-uninstall-" => "sunrise-launcher-uninstall-",
    SETUP_PREFIX: "horizon-launcher-setup" => "sunrise-launcher-setup",
    PROCESS_FILTER: "IMAGENAME eq horizon-launcher.exe" => "IMAGENAME eq sunrise-launcher.exe",
}

/// The launchers before this one, newest first: Horizon's own, then the arismoko fork's. Each
/// is (launcher folder, game folder, location folder) under the same bases as this launcher's.
/// A Sunrise build copies its first data from the first of them that has any (`carry_over`);
/// it never writes to them, and the uninstaller never removes them.
pub const EARLIER: &[(&str, &str, &str)] = if cfg!(sunrise_fork_launcher) {
    &[
        ("horizon-launcher", "horizon", "horizon-launcher-location"),
        (
            "horizon-fork-arismoko/launcher",
            "horizon-fork-arismoko/game",
            "horizon-fork-arismoko-location",
        ),
    ]
} else {
    &[]
};

/// The keychain services of the launchers before this one ([`EARLIER`]): a sign-in saved there is
/// copied under [`KEYCHAIN_SERVICE`] the first time it is read.
pub const EARLIER_KEYCHAIN_SERVICES: &[&str] = if cfg!(sunrise_fork_launcher) {
    &["dev.playhorizon.launcher"]
} else {
    &[]
};

/// The launcher's data folder under `base`.
pub fn launcher_folder_in(base: &std::path::Path) -> std::path::PathBuf {
    base.join(LAUNCHER_FOLDER)
}

/// The game's folder under `base`.
pub fn game_folder_in(base: &std::path::Path) -> std::path::PathBuf {
    base.join(GAME_FOLDER)
}

/// Product home used by the Windows installer's About link.
pub const ABOUT_URL: &str = if cfg!(sunrise_fork_launcher) {
    sunrise_launcher_addresses::FORK_LAUNCHER_ABOUT
} else {
    sunrise_launcher_addresses::LAUNCHER_ABOUT
};

/// Whether the uninstaller may remove `path` as this launcher's own data: the complete owned
/// suffix, not just a generic leaf selected by an override. An earlier launcher's folder
/// ([`EARLIER`]) never is. Uninstallation keeps the existing named-folder guard for relocated
/// data as well.
pub fn owns_launcher_data_dir(path: &std::path::Path) -> bool {
    ends_with(path, LAUNCHER_FOLDER)
}

/// Whether `path` ends with every component of `suffix`, case aside.
fn ends_with(path: &std::path::Path, suffix: &str) -> bool {
    let mut actual = path.components().rev();
    std::path::Path::new(suffix)
        .components()
        .rev()
        .all(|expected| {
            actual.next().is_some_and(|component| {
                component
                    .as_os_str()
                    .eq_ignore_ascii_case(expected.as_os_str())
            })
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn data_removal_requires_the_complete_owned_namespace() {
        let profile = Path::new("profile");
        let own = profile.join(LAUNCHER_FOLDER);
        assert!(owns_launcher_data_dir(&own));
        assert!(owns_launcher_data_dir(
            &profile.join(LAUNCHER_FOLDER.to_ascii_uppercase())
        ));
        // An earlier launcher's folder is never this one's to remove.
        for (old, _, _) in EARLIER {
            assert!(!owns_launcher_data_dir(&profile.join(old)), "{old}");
        }
        for unowned in [
            "launcher",
            "unrelated/launcher",
            "not-horizon-fork-arismoko/launcher",
            "horizon-launcher-backup",
            "sunrise/launcher-backup",
            "not-sunrise/launcher-backup",
        ] {
            assert!(!owns_launcher_data_dir(&profile.join(unowned)), "{unowned}");
        }
    }
}
