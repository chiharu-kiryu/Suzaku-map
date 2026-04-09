use std::env;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() -> Result<(), Box<dyn Error>> {
    let mode = env::args().nth(1).unwrap_or_else(|| "build".to_string());
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let app_dir = build_bundle(&manifest_dir)?;

    match mode.as_str() {
        "open" => {
            open_app(&app_dir)?;
        }
        "install" => {
            let installed = install_app(&app_dir)?;
            println!("{}", installed.display());
            return Ok(());
        }
        "install-open" => {
            let installed = install_app(&app_dir)?;
            open_app(&installed)?;
            println!("{}", installed.display());
            return Ok(());
        }
        _ => {}
    }

    println!("{}", app_dir.display());
    Ok(())
}

fn build_bundle(root: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let status = Command::new("cargo")
        .arg("build")
        .arg("--features")
        .arg("gpu")
        .arg("--bin")
        .arg("panel")
        .current_dir(root)
        .status()?;
    if !status.success() {
        return Err("failed to build panel binary".into());
    }

    let target_dir = root.join("target/debug");
    let app_dir = target_dir.join("Suzaku Panel.app");
    let contents_dir = app_dir.join("Contents");
    let macos_dir = contents_dir.join("MacOS");
    let resources_dir = contents_dir.join("Resources");
    let plist_path = root.join("src/macos/SuzakuPanel-Info.plist");
    let icns_source_path = root.join("src/assets/icons/SuzakuPanel.icns");
    let bin_path = target_dir.join("panel");
    let app_bin_path = macos_dir.join("Suzaku Panel");
    let icns_path = resources_dir.join("SuzakuPanel.icns");

    fs::create_dir_all(&macos_dir)?;
    fs::create_dir_all(&resources_dir)?;
    fs::copy(plist_path, contents_dir.join("Info.plist"))?;
    fs::copy(bin_path, &app_bin_path)?;
    fs::copy(&icns_source_path, &icns_path)?;

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

fn install_app(app_dir: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let home = env::var("HOME")?;
    let applications_dir = PathBuf::from(home).join("Applications");
    let installed_dir = applications_dir.join("Suzaku Panel.app");
    fs::create_dir_all(&applications_dir)?;
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
