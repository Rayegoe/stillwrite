//! git 同步引擎：最后写入者胜（Last-Write-Wins）。
//! 流程：本地提交 → fetch → merge(按 mtime/提交时间逐文件裁决冲突) → push。
//! 依赖系统 git CLI（原型阶段）；生产可换 git2 vendored 或自研协议。

use serde::Serialize;
use std::path::Path;
use std::process::Command;

#[derive(Serialize, Default)]
pub struct SyncStatus {
    pub ok: bool,
    pub pulled: usize,
    pub pushed: usize,
    pub conflicts: usize,
    pub remote_wins: usize,
    pub message: String,
}

fn git(root: &Path, args: &[&str]) -> Result<(bool, String), String> {
    let out = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|e| format!("无法执行 git（系统未安装？）: {e}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
    let text = if stdout.is_empty() { stderr } else { stdout };
    Ok((out.status.success(), text))
}

fn file_mtime_secs(root: &Path, rel: &str) -> Option<i64> {
    let p = root.join(rel);
    let meta = std::fs::metadata(p).ok()?;
    let m = meta.modified().ok()?;
    let d = m.duration_since(std::time::UNIX_EPOCH).ok()?;
    Some(d.as_secs() as i64)
}

pub fn sync_workspace(root: &Path, remote_hint: &str) -> Result<SyncStatus, String> {
    let mut status = SyncStatus::default();

    // 1. 确保是 git 仓库
    if !root.join(".git").exists() {
        git(root, &["init"]).map_err(|e| format!("git init 失败: {e}"))?;
    }

    // 2. 远程
    let (_, remotes) = git(root, &["remote", "get-url", "origin"])?;
    if remotes.trim().is_empty() {
        return Err(format!(
            "工作区尚未配置远程仓库。请先执行:\n  git remote add origin {remote_hint}"
        ));
    }

    // 3. 当前分支
    let (okb, branch) = git(root, &["rev-parse", "--abbrev-ref", "HEAD"])?;
    if !okb || branch.trim().is_empty() {
        return Err("无法确定当前 git 分支".into());
    }
    let branch = branch.trim().to_string();

    // 4. 提交本地改动
    git(root, &["add", "-A"]).map_err(|e| format!("git add 失败: {e}"))?;
    let has_staged = !git(root, &["diff", "--cached", "--quiet"])
        .map(|(ok, _)| ok)
        .unwrap_or(false);
    if has_staged {
        let msg = format!("sync {}", unix_ts());
        let (okc, err) = git(root, &["commit", "-m", &msg])?;
        if !okc {
            if err.contains("Please tell me who you are") || err.contains("identity") {
                return Err(
                    "git 未配置身份。请先执行:\n  git config --global user.name \"你的名字\"\n  git config --global user.email \"you@example.com\"".into(),
                );
            }
            return Err(format!("git commit 失败: {err}"));
        }
    }

    // 5. fetch
    let (okf, errf) = git(root, &["fetch", "origin"])?;
    if !okf {
        return Err(format!("无法连接远程仓库（{remote_hint}）:\n{errf}\n\n请确认板子在线、SSH 密钥已配置。"));
    }

    // 6. 远端分支是否存在 + merge-base
    let (has_remote_branch, _) =
        git(root, &["rev-parse", "--verify", "--quiet", &format!("origin/{branch}")])?;
    let mut base = String::new();
    let mut pulled_total = 0usize;
    if has_remote_branch {
        let (okb2, base_out) =
            git(root, &["merge-base", "HEAD", &format!("origin/{branch}")])?;
        if !okb2 || base_out.trim().is_empty() {
            return Err("本地与远程历史不相关，请手动处理首次合并".into());
        }
        base = base_out.trim().to_string();

        // 7. merge（先统计远程带来多少文件变更）
        let (_, remote_files) =
            git(root, &["diff", "--name-only", &base, &format!("origin/{branch}")])?;
        pulled_total = remote_files.lines().filter(|l| !l.trim().is_empty()).count();

        let (okm, _) =
            git(root, &["merge", "--no-commit", "--no-ff", &format!("origin/{branch}")])?;

        if !okm {
            // 有冲突：逐文件按 mtime / 远端提交时间裁决（最后写入者胜）
            let (_, unmerged) = git(root, &["diff", "--name-only", "--diff-filter=U"])?;
            let conflict_files: Vec<String> = unmerged
                .lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect();
            status.conflicts = conflict_files.len();

            for rel in &conflict_files {
                let local_mtime = file_mtime_secs(root, rel);
                let (_, remote_ct) = git(
                    root,
                    &[
                        "log",
                        "-1",
                        "--format=%ct",
                        &format!("origin/{branch}"),
                        "--",
                        rel,
                    ],
                )?;
                let remote_ct = remote_ct.trim().parse::<i64>().ok();

                let remote_newer = match (local_mtime, remote_ct) {
                    (Some(l), Some(r)) => r > l,
                    (None, Some(_)) => true, // 本地已删除，远端有版本 → 取远端
                    _ => false,
                };

                if remote_newer {
                    git(root, &["checkout", "--theirs", "--", rel])?;
                    status.remote_wins += 1;
                } else {
                    git(root, &["checkout", "--ours", "--", rel])?;
                }
                git(root, &["add", "--", rel])?;
            }
        }

        // merge 遗留的暂存改动（无论有无冲突）统一提交
        let has_merge_staged = !git(root, &["diff", "--cached", "--quiet"])
            .map(|(ok, _)| ok)
            .unwrap_or(true);
        if has_merge_staged {
            let _ = git(root, &["commit", "-m", "sync merge (LWW)"]);
        }

        // 8. 统计 pull 净变化
        let (_, after_merge) = git(root, &["diff", "--name-only", &base, "HEAD"])?;
        status.pulled = after_merge.lines().filter(|l| !l.trim().is_empty()).count();
        if pulled_total > status.pulled {
            status.pulled = pulled_total;
        }
    }

    // 9. push
    let (okp, errp) = git(root, &["push", "origin", &branch])?;
    if !okp {
        return Err(format!("push 失败: {errp}"));
    }
    status.pushed = 1;

    status.ok = true;
    status.message = format!("已同步（拉取 {} · 推送 · 冲突 {}）", status.pulled, status.conflicts);
    Ok(status)
}

fn unix_ts() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_else(|_| String::from("0"))
}
