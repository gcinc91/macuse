use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::sync::OnceLock;

/// Resuelve `~/Library/Logs/macuse/macuse.log`. Solo lo calcula una vez.
fn log_path() -> Option<&'static PathBuf> {
    static PATH: OnceLock<Option<PathBuf>> = OnceLock::new();
    PATH.get_or_init(|| {
        let home = dirs::home_dir()?;
        let dir = home.join("Library/Logs/macuse");
        fs::create_dir_all(&dir).ok()?;
        Some(dir.join("macuse.log"))
    })
    .as_ref()
}

pub fn log(msg: &str) {
    let Some(path) = log_path() else { return };
    // O_NOFOLLOW evita que un symlink (atacante u otro proceso) redirija
    // nuestro log a un archivo arbitrario. Si el path final es un symlink,
    // open falla y simplemente no logueamos esta linea.
    if let Ok(mut f) = OpenOptions::new()
        .create(true)
        .append(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
    {
        let _ = writeln!(f, "{}", msg);
    }
}

#[macro_export]
macro_rules! mlog {
    ($($arg:tt)*) => {
        $crate::log::log(&format!($($arg)*))
    };
}
