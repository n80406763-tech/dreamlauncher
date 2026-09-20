//! Запуск процесса игры (или, в тестах, любого другого процесса) с
//! построчным чтением stdout/stderr. Не знает про Tauri — вызывающий код
//! оборачивает `LaunchEvent` в `tauri::ipc::Channel`.

use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub enum LaunchEvent {
    Stdout(String),
    Stderr(String),
    Exited { code: Option<i32> },
}

#[derive(Debug, Clone)]
pub struct LaunchCommand {
    pub java_executable: PathBuf,
    pub jvm_args: Vec<String>,
    pub main_class: String,
    pub game_args: Vec<String>,
    pub working_directory: PathBuf,
}

impl LaunchCommand {
    pub fn to_args(&self) -> Vec<String> {
        let mut args = self.jvm_args.clone();
        args.push(self.main_class.clone());
        args.extend(self.game_args.iter().cloned());
        args
    }
}

/// Запускает команду и построчно шлёт события в канал, пока процесс не
/// завершится. Возвращает управление сразу после спавна; ждать выхода
/// нужно через отдельный `wait`-хендл, если он понадобится вызывающему коду.
pub async fn spawn(cmd: &LaunchCommand) -> std::io::Result<mpsc::UnboundedReceiver<LaunchEvent>> {
    let mut child = Command::new(&cmd.java_executable)
        .args(cmd.to_args())
        .current_dir(&cmd.working_directory)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;

    let stdout = child.stdout.take().expect("stdout must be piped");
    let stderr = child.stderr.take().expect("stderr must be piped");

    let (tx, rx) = mpsc::unbounded_channel();

    let tx_out = tx.clone();
    tokio::spawn(async move {
        let mut lines = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if tx_out.send(LaunchEvent::Stdout(line)).is_err() {
                break;
            }
        }
    });

    let tx_err = tx.clone();
    tokio::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if tx_err.send(LaunchEvent::Stderr(line)).is_err() {
                break;
            }
        }
    });

    tokio::spawn(async move {
        let status = child.wait().await.ok();
        let _ = tx.send(LaunchEvent::Exited { code: status.and_then(|s| s.code()) });
    });

    Ok(rx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn captures_stdout_and_exit_code() {
        let cmd = LaunchCommand {
            java_executable: PathBuf::from(if cfg!(windows) { "cmd" } else { "sh" }),
            jvm_args: if cfg!(windows) { vec!["/C".into(), "echo".into()] } else { vec!["-c".into(), "echo hello-from-dream-core".into()] },
            main_class: String::new(),
            game_args: if cfg!(windows) { vec!["hello-from-dream-core".into()] } else { vec![] },
            working_directory: std::env::temp_dir(),
        };

        let mut rx = spawn(&cmd).await.expect("spawn must succeed");
        let mut saw_output = false;
        let mut exit_code = None;
        while let Some(event) = rx.recv().await {
            match event {
                LaunchEvent::Stdout(line) => {
                    if line.contains("hello-from-dream-core") {
                        saw_output = true;
                    }
                }
                LaunchEvent::Exited { code } => exit_code = Some(code),
                LaunchEvent::Stderr(_) => {}
            }
        }

        assert!(saw_output, "должны получить строку из stdout дочернего процесса");
        assert_eq!(exit_code, Some(Some(0)));
    }
}
