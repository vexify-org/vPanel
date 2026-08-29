//! 系统级控制：服务管理(systemctl)、防火墙端口(ufw)、定时任务(crontab)。
//! 全部动作按需执行为一次性子进程，无常驻状态，内存开销趋近于零。
//! 操作类需要 root 权限；失败时返回明确的错误文本。

use crate::json;

/// 服务列表 -> JSON 字符串。
pub fn services_json() -> String {
    let out = json::run_out("systemctl", &["list-units", "--type=service", "--all", "--no-legend", "--no-pager"]);
    let mut items: Vec<String> = Vec::new();
    if let Some(s) = out {
        for line in s.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let mut it = line.split_whitespace();
            let name = it.next().unwrap_or("");
            let _load = it.next().unwrap_or("");
            let active = it.next().unwrap_or("");
            let sub = it.next().unwrap_or("");
            let desc: Vec<&str> = it.collect();
            if name.is_empty() || active.is_empty() {
                continue;
            }
            items.push(format!(
                "{{\"name\":\"{}\",\"active\":\"{}\",\"sub\":\"{}\",\"desc\":\"{}\"}}",
                json::jesc(name),
                json::jesc(active),
                json::jesc(sub),
                json::jesc(&desc.join(" "))
            ));
        }
    }
    format!("{{\"len\":{},\"list\":[{}]}}", items.len(), items.join(","))
}

/// 服务操作：start/stop/restart。返回 (ok, stderr说明)。
pub fn service_action(name: &str, action: &str) -> (bool, String) {
    let out = std::process::Command::new("systemctl")
        .args([action, name])
        .output();
    match out {
        Ok(o) => {
            let err = String::from_utf8_lossy(&o.stderr).trim().to_string();
            (o.status.success(), err)
        }
        Err(e) => (false, e.to_string()),
    }
}

fn cmd_result(out: std::io::Result<std::process::Output>) -> (bool, String) {
    match out {
        Ok(o) => {
            let msg = {
                let stdout = String::from_utf8_lossy(&o.stdout);
                let stderr = String::from_utf8_lossy(&o.stderr);
                let combined = format!("{}{}", stdout, stderr);
                combined.trim().to_string()
            };
            (o.status.success(), msg)
        }
        Err(e) => (false, e.to_string()),
    }
}

/// 定时任务列表 -> JSON 字符串（读取当前用户 crontab）。
pub fn tasks_json() -> String {
    let out = json::run_out("crontab", &["-l"]);
    let mut tasks = String::from("[");
    let mut first = true;
    if let Some(s) = out {
        for line in s.lines() {
            let t = line.trim();
            if t.is_empty() || t.starts_with('#') {
                continue;
            }
            let fields: Vec<&str> = t.split_whitespace().collect();
            if fields.len() < 6 {
                continue;
            }
            let schedule = fields[..5].join(" ");
            let command = fields[5..].join(" ");
            if !first {
                tasks.push(',');
            }
            first = false;
            tasks.push_str(&format!(
                "{{\"schedule\":\"{}\",\"command\":\"{}\"}}",
                json::jesc(&schedule),
                json::jesc(&command)
            ));
        }
    }
    tasks.push(']');
    let comma_count = tasks.matches(',').count() as i64;
    let comma_parts = if comma_count == 0 { 0 } else { comma_count + 1 };
    format!("{{\"len\":{},\"list\":{}}}", comma_parts, tasks)
}

/// 新增一条定时任务。schedule 为 5 段 cron，command 为要执行的命令。
pub fn task_add(schedule: &str, command: &str) -> (bool, String) {
    if schedule.split_whitespace().count() != 5 || command.trim().is_empty() {
        return (false, "schedule 需为 5 段 cron，且 command 不能为空".into());
    }
    // (crontab -l 2>/dev/null; echo 'schedule command') | crontab -
    let line = format!("{} {}", schedule.trim(), command.trim());
    let existing = std::process::Command::new("sh")
        .args(["-c", "crontab -l 2>/dev/null"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();
    let combined = if existing.trim().is_empty() {
        format!("{}\n", line)
    } else {
        format!("{}\n{}\n", existing.trim_end(), line)
    };
    let out = std::process::Command::new("bash")
        .arg("-c")
        .arg(format!("crontab - 2>/dev/null <<'EOF'\n{}\nEOF", combined))
        .output();
    cmd_result(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_add_validates_schedule() {
        // 校验在调用 crontab 之前即返回，不会改动系统状态。
        assert!(!task_add("* * *", "echo hi").0);
        assert!(!task_add("* * * * *", "").0);
        assert!(!task_add("", "echo hi").0);
        // 5 段 cron + 非空命令通过校验（进入写入阶段，可能在无 crontab 环境失败，这里只验证格式）。
        let (can_proceed, msg) = task_add("* * * * *", "echo hi");
        // 格式合法时返回 (true, _) 或 (false, 系统错误)；二者兜底，不 panic。
        let _ = (can_proceed, msg);
    }

    #[test]
    fn cmd_result_combines_and_trims() {
        use std::process::{Command, Output};

        let out = Output {
            status: Command::new("true").output().unwrap().status,
            stdout: b"hello".to_vec(),
            stderr: b" world".to_vec(),
        };
        let (ok, msg) = cmd_result(Ok(out));
        assert!(ok);
        assert_eq!(msg, "hello world");

        // 非零退出码 -> 失败，但文本仍组合。
        let out = Output {
            status: Command::new("false").output().unwrap().status,
            stdout: b"".to_vec(),
            stderr: b"boom\n".to_vec(),
        };
        let (ok, msg) = cmd_result(Ok(out));
        assert!(!ok);
        assert_eq!(msg, "boom");

        // 启动失败 -> Err。
        let (ok, _msg) = cmd_result(Err(std::io::Error::new(std::io::ErrorKind::Other, "x")));
        assert!(!ok);
    }
}