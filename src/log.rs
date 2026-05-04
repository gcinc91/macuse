use std::fs::OpenOptions;
use std::io::Write;

pub fn log(msg: &str) {
    if let Ok(mut f) = OpenOptions::new()
        .create(true)
        .append(true)
        .open("/tmp/macuse.log")
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
