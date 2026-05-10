use std::env;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

struct MacOsBundleSpec {
    app_name: &'static str,
    executable_name: &'static str,
    binary_name: &'static str,
    plist_path: &'static str,
    icon_name: Option<&'static str>,
    install_dir_kind: InstallDirKind,
}

enum InstallDirKind {
    Applications,
    InputMethods,
}

fn main() -> Result<(), Box<dyn Error>> {
    let target = env::args().nth(1).unwrap_or_else(|| "panel".to_string());
    let mode = env::args().nth(2).unwrap_or_else(|| "build".to_string());
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let spec = match target.as_str() {
        "ime" => ime_bundle_spec(),
        _ => panel_bundle_spec(),
    };
    let app_dir = build_bundle(&manifest_dir, &spec)?;

    match mode.as_str() {
        "open" => {
            open_app(&app_dir)?;
        }
        "install" => {
            let installed = install_app(&app_dir, &spec)?;
            println!("{}", installed.display());
            return Ok(());
        }
        "install-open" => {
            let installed = install_app(&app_dir, &spec)?;
            open_app(&installed)?;
            println!("{}", installed.display());
            return Ok(());
        }
        _ => {}
    }

    println!("{}", app_dir.display());
    Ok(())
}

fn panel_bundle_spec() -> MacOsBundleSpec {
    MacOsBundleSpec {
        app_name: "Suzaku Panel",
        executable_name: "Suzaku Panel",
        binary_name: "panel",
        plist_path: "src/macos/SuzakuPanel-Info.plist",
        icon_name: Some("SuzakuPanel.icns"),
        install_dir_kind: InstallDirKind::Applications,
    }
}

fn ime_bundle_spec() -> MacOsBundleSpec {
    MacOsBundleSpec {
        app_name: "Suzaku Input Method",
        executable_name: "Suzaku Input Method",
        binary_name: "macos_ime_host",
        plist_path: "src/macos/SuzakuInputMethod-Info.plist",
        icon_name: Some("SuzakuPanel.icns"),
        install_dir_kind: InstallDirKind::InputMethods,
    }
}

fn build_bundle(root: &Path, spec: &MacOsBundleSpec) -> Result<PathBuf, Box<dyn Error>> {
    let status = Command::new("cargo")
        .arg("build")
        .arg("--bin")
        .arg(spec.binary_name)
        .args(if spec.binary_name == "panel" {
            vec!["--features", "gpu"]
        } else {
            vec![]
        })
        .current_dir(root)
        .status()?;
    if !status.success() {
        return Err(format!("failed to build {} binary", spec.binary_name).into());
    }

    let target_dir = root.join("target/debug");
    let app_dir = target_dir.join(format!("{}.app", spec.app_name));
    let contents_dir = app_dir.join("Contents");
    let macos_dir = contents_dir.join("MacOS");
    let resources_dir = contents_dir.join("Resources");
    let plist_path = root.join(spec.plist_path);
    let bin_path = target_dir.join(spec.binary_name);
    let app_bin_path = macos_dir.join(spec.executable_name);

    fs::create_dir_all(&macos_dir)?;
    fs::create_dir_all(&resources_dir)?;
    fs::copy(plist_path, contents_dir.join("Info.plist"))?;
    fs::copy(bin_path, &app_bin_path)?;
    if let Some(icon_name) = spec.icon_name {
        let icns_source_path = root.join("src/assets/icons").join(icon_name);
        let icns_path = resources_dir.join(icon_name);
        fs::copy(&icns_source_path, &icns_path)?;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&app_bin_path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&app_bin_path, perms)?;
    }

    let sign_status = Command::new("codesign")
        .arg("--force")
        .arg("--deep")
        .arg("--sign")
        .arg("-")
        .arg(&app_dir)
        .status()?;
    if !sign_status.success() {
        return Err("failed to ad-hoc sign macOS app bundle".into());
    }

    Ok(app_dir)
}

fn install_app(app_dir: &Path, spec: &MacOsBundleSpec) -> Result<PathBuf, Box<dyn Error>> {
    let home = env::var("HOME")?;
    let base_dir = match spec.install_dir_kind {
        InstallDirKind::Applications => PathBuf::from(home).join("Applications"),
        InstallDirKind::InputMethods => PathBuf::from(home).join("Library/Input Methods"),
    };
    let installed_dir = base_dir.join(format!("{}.app", spec.app_name));
    fs::create_dir_all(&base_dir)?;
    if installed_dir.exists() {
        fs::remove_dir_all(&installed_dir)?;
    }
    copy_dir_all(app_dir, &installed_dir)?;
    Ok(installed_dir)
}

fn open_app(app_dir: &Path) -> Result<(), Box<dyn Error>> {
    let status = Command::new("open").arg(app_dir).status()?;
    if !status.success() {
        return Err("failed to open macOS app bundle".into());
    }
    Ok(())
}

fn copy_dir_all(src: &Path, dst: &Path) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let target = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}
