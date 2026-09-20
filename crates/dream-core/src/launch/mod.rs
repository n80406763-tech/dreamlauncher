//! Сборка и запуск команды игры: подстановка аргументов, classpath,
//! спавн процесса Java с построчным чтением вывода.

pub mod args;
pub mod classpath;
pub mod crash;
pub mod process;

pub use args::{AccountKind, LaunchContext, QuickPlay};
pub use classpath::build_classpath;
pub use crash::{find_recent_crashes, read_crash_report, CrashKind, CrashReport};
pub use process::{spawn, LaunchCommand, LaunchEvent};
