use std::sync::atomic::{AtomicBool, Ordering};

const PORTABLE_DIR_NAME: &str = ".Axolotl";

static PORTABLE_MODE: AtomicBool = AtomicBool::new(false);

/// 在启动时初始化便携模式
/// 检查 `.Axolotl` 文件夹是否存在且可写
/// 如果存在且可写，将 `THESEUS_CONFIG_DIR` 环境变量设置为该路径
/// 返回 `true` 如果便携模式已启用，否则返回 `false`
///
/// 必须在 main() 开头、任何其他线程（包括 tokio runtime）启动之前调用。
/// 此函数内部会调用 `std::env::set_var`，该函数在 Rust 中不是线程安全的。
pub unsafe fn init_portable_mode() -> bool {
    let exe_path = match std::env::current_exe() {
        Ok(p) => p,
        Err(_) => return false,
    };

    let Some(app_dir) = exe_path.parent() else {
        return false;
    };

    let portable_dir = app_dir.join(PORTABLE_DIR_NAME);

    if !portable_dir.is_dir() {
        return false;
    }
    if !try_write_test(&portable_dir) {
        return false;
    }

    // SAFETY: 调用者保证此时没有其他线程访问环境变量
    unsafe {
        std::env::set_var("THESEUS_CONFIG_DIR", &portable_dir);
    }
    PORTABLE_MODE.store(true, Ordering::Relaxed);

    tracing::info!(
        "Portable mode enabled: THESEUS_CONFIG_DIR={}",
        portable_dir.display()
    );

    true
}

/// 尝试在目标目录中创建并删除临时文件以验证可写性。
/// 先删除可能残留的 `.write_test`，避免已有的只读文件导致 `File::create` 失败。
fn try_write_test(dir: &std::path::Path) -> bool {
    let test_file = dir.join(".write_test");

    let _ = std::fs::remove_file(&test_file);

    match std::fs::File::create(&test_file) {
        Ok(_) => {
            let _ = std::fs::remove_file(&test_file);
            true
        }
        Err(e) => {
            tracing::warn!(
                "Portable directory {} exists but is not writable: {}",
                dir.display(),
                e
            );
            false
        }
    }
}

/// Tauri 命令：检查应用程序是否运行在便携模式下
#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub fn is_portable_mode() -> bool {
    PORTABLE_MODE.load(Ordering::Relaxed)
}
