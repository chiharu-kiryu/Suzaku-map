use std::path::Path;
use suzaku_map::data::{
    backup::{self, Backup},
    files::{self, DataLease},
    paths::DataPaths,
};

pub fn run(args: &[String]) -> i32 {
    match dispatch(args) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}

fn dispatch(args: &[String]) -> Result<(), String> {
    let action = args.first().map(String::as_str).unwrap_or("status");
    if matches!(action, "help" | "--help" | "-h") {
        println!(
            "suzaku_tool data status\nsuzaku_tool data backup [FILE]\nsuzaku_tool data validate FILE\nsuzaku_tool data restore FILE [--apply]\nsuzaku_tool data open [ime|panel|backups]\n\n恢复默认仅预览；--apply 要求退出面板并停止输入法宿主。备份不包含输入记录、缓存或模型权重。"
        );
        return Ok(());
    }
    // Validate/read does not require a configured home or write any local data.
    if action == "validate" && args.len() == 2 {
        Backup::read(Path::new(&args[1]))?;
        println!("备份校验通过（Suzaku settings v1）；未修改配置。");
        return Ok(());
    }
    let paths = DataPaths::current()?;
    match action {
        "status" if args.len() <= 1 => {
            for (label, path) in [("输入法配置", &paths.ime), ("面板配置", &paths.panel)] {
                let raw =
                    files::read_optional(path, files::SETTINGS_LIMIT).map_err(|e| e.to_string())?;
                println!(
                    "{label}: {} ({})",
                    path.display(),
                    raw.map(|s| format!("{} bytes", s.len()))
                        .unwrap_or("未创建，使用默认值".into())
                );
            }
            println!(
                "备份目录: {}\n词库: 内置于程序；暂无用户词库文件\n模型: 外部本地 / 云端服务管理，不复制或删除权重\n隐私: 不持久化输入历史、语音、手写草稿或模型密钥",
                paths.backups.display()
            );
            Backup::collect(&paths)?;
            println!("配置校验通过。");
        }
        "backup" if args.len() <= 2 => {
            let path = if let Some(destination) = args.get(1) {
                let _lease = DataLease::acquire(&paths.lock_path()?, false)?;
                let path = std::path::PathBuf::from(destination);
                Backup::collect(&paths)?.write_new(&path)?;
                path
            } else {
                backup::backup_now(&paths)?
            };
            println!("已创建配置备份：{}", path.display());
        }
        "restore" if args.len() == 2 || (args.len() == 3 && args[2] == "--apply") => {
            let snapshot = Backup::read(Path::new(&args[1]))?;
            for line in backup::preview(&paths, &snapshot)? {
                println!("{line}");
            }
            if args.len() == 2 {
                println!("仅预览，未修改配置。确认后使用 --apply；请先退出面板并停止输入法宿主。");
            } else {
                #[cfg(target_os = "linux")]
                match backup::restore(&paths, &snapshot)? {
                    Some(safety) => println!(
                        "恢复完成。恢复前备份：{}\n重新启动宿主和面板以加载配置。",
                        safety.display()
                    ),
                    None => println!("配置已一致，无需修改。"),
                }
                #[cfg(not(target_os = "linux"))]
                return Err("本轮仅支持在 Linux 应用恢复；此平台可以备份、校验和预览。".into());
            }
        }
        "open" if args.len() <= 2 => {
            let destination = match args.get(1).map(String::as_str).unwrap_or("backups") {
                "ime" => paths.ime.parent().ok_or("配置路径无效")?,
                "panel" => paths.panel.parent().ok_or("配置路径无效")?,
                "backups" => &paths.backups,
                _ => return Err("目录必须是 ime、panel 或 backups".into()),
            };
            #[cfg(target_os = "linux")]
            suzaku_map::data::open_directory(destination)?;
            #[cfg(not(target_os = "linux"))]
            return Err(format!("请在文件管理器中打开：{}", destination.display()));
        }
        _ => return Err("参数无效；使用 `suzaku_tool data --help` 查看用法".into()),
    }
    Ok(())
}
