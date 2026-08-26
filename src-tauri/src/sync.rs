//! git 同步引擎：最后写入者胜（Last-Write-Wins）。
//! 流程：本地提交 → fetch → merge(按 mtime/提交时间逐文件裁决冲突) → push。
//! 依赖系统 git CLI（原型阶段）；生产可换 git2 vendored 或自研协议。

use serde::Serialize;
use std::collections::HashMap;
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

/// merge 前快照工作区所有文件的 mtime。git merge 在冲突时会 checkout
/// 冲突文件到工作区（覆盖 mtime），所以必须在 merge 之前采集。
fn snapshot_mtimes(root: &Path) -> HashMap<String, i64> {
    let mut map = HashMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with('.') {
                continue;
            }
            let Ok(ft) = entry.file_type() else { continue };
            if ft.is_dir() {
                stack.push(path);
            } else if ft.is_file() {
                if let Some(mtime) = file_mtime_secs(root, &rel_of(root, &path)) {
                    map.insert(rel_of(root, &path), mtime);
                }
            }
        }
    }
    map
}

fn rel_of(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// 解析同步应使用的 remote 名。
/// - origin 已指向 sync_url → 复用 origin（向后兼容）
/// - 否则确保独立 remote `sync` 指向 sync_url，**绝不改写用户已有的 origin**
pub fn resolve_sync_remote(root: &Path, sync_url: &str) -> Result<String, String> {
    if !root.join(".git").exists() {
        git(root, &["init"]).map_err(|e| format!("git init 失败: {e}"))?;
    }

    let (has_origin, origin_url) = git(root, &["remote", "get-url", "origin"])?;
    if has_origin && origin_url.trim() == sync_url {
        return Ok("origin".to_string());
    }

    let (has_sync, sync_url_existing) = git(root, &["remote", "get-url", "sync"])?;
    if has_sync {
        if sync_url_existing.trim() != sync_url {
            git(root, &["remote", "set-url", "sync", sync_url])
                .map_err(|e| format!("更新 sync remote 失败: {e}"))?;
        }
    } else {
        git(root, &["remote", "add", "sync", sync_url])
            .map_err(|e| format!("添加 sync remote 失败: {e}"))?;
    }
    Ok("sync".to_string())
}

pub fn sync_workspace(
    root: &Path,
    remote_hint: &str,
    remote_name: &str,
) -> Result<SyncStatus, String> {
    let mut status = SyncStatus::default();

    // 1. 确保是 git 仓库
    if !root.join(".git").exists() {
        git(root, &["init"]).map_err(|e| format!("git init 失败: {e}"))?;
    }

    // 2. 同步 remote 必须存在（resolve_sync_remote 已确保；此处双保险）
    let (ok_remote, _) = git(root, &["remote", "get-url", remote_name])?;
    if !ok_remote {
        return Err(format!(
            "工作区尚未配置同步远程（{remote_name}）。请先执行:\n  git remote add {remote_name} {remote_hint}"
        ));
    }

    // 3. 当前分支（unborn HEAD 也能读）
    let (okb, branch) = git(root, &["symbolic-ref", "--short", "HEAD"])?;
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
    let (okf, errf) = git(root, &["fetch", remote_name])?;
    if !okf {
        return Err(format!(
            "无法连接远程仓库（{remote_hint}）:\n{errf}\n\n请确认板子在线、SSH 密钥已配置。"
        ));
    }

    // 6. 远端分支是否有真实提交（空 ref 不算）+ merge-base
    let (has_remote_commit, _) = git(
        root,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{remote_name}/{branch}^{{commit}}"),
        ],
    )?;
    if has_remote_commit {
        let (okb2, base_out) = git(
            root,
            &["merge-base", "HEAD", &format!("{remote_name}/{branch}")],
        )?;
        if !okb2 || base_out.trim().is_empty() {
            return Err("本地与远程历史不相关，请手动处理首次合并".into());
        }
        let base = base_out.trim().to_string();

        // 7. merge（先统计远程带来多少文件变更）
        let (_, remote_files) = git(
            root,
            &[
                "diff",
                "--name-only",
                &base,
                &format!("{remote_name}/{branch}"),
            ],
        )?;
        let pulled_total = remote_files
            .lines()
            .filter(|l| !l.trim().is_empty())
            .count();

        // merge 前快照工作区 mtime（git merge 会覆盖冲突文件的 mtime）
        let mtimes = snapshot_mtimes(root);
        let (okm, _) = git(
            root,
            &[
                "merge",
                "--no-commit",
                "--no-ff",
                &format!("{remote_name}/{branch}"),
            ],
        )?;

        if !okm {
            // 有冲突：逐文件按 merge 前的 mtime / 远端提交时间裁决（最后写入者胜）
            let (_, unmerged) = git(root, &["diff", "--name-only", "--diff-filter=U"])?;
            let conflict_files: Vec<String> = unmerged
                .lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect();
            status.conflicts = conflict_files.len();

            for rel in &conflict_files {
                let local_mtime = mtimes.get(rel).copied();
                let (_, remote_ct) = git(
                    root,
                    &[
                        "log",
                        "-1",
                        "--format=%ct",
                        &format!("{remote_name}/{branch}"),
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

        // merge 遗留的暂存改动（无论有无冲突）统一提交；全按 ours 解决时
        // 结果与本地 HEAD 相同，用 --allow-empty 确保 merge 提交落地
        let merge_in_progress = root.join(".git").join("MERGE_HEAD").exists();
        if merge_in_progress {
            let _ = git(root, &["commit", "--allow-empty", "-m", "sync merge (LWW)"]);
        }

        // 8. 统计 pull 净变化
        let (_, after_merge) = git(root, &["diff", "--name-only", &base, "HEAD"])?;
        status.pulled = after_merge.lines().filter(|l| !l.trim().is_empty()).count();
        if pulled_total > status.pulled {
            status.pulled = pulled_total;
        }
    }

    // 9. push
    let (okp, errp) = git(root, &["push", remote_name, &branch])?;
    if !okp {
        return Err(format!("push 失败: {errp}"));
    }
    status.pushed = 1;

    status.ok = true;
    status.message = format!(
        "已同步（拉取 {} · 推送 · 冲突 {}）",
        status.pulled, status.conflicts
    );
    Ok(status)
}

fn unix_ts() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_else(|_| String::from("0"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::process::Command;

    pub(crate) fn git_in(dir: &Path, args: &[&str]) -> String {
        let out = Command::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .expect("git 执行失败");
        assert!(
            out.status.success(),
            "git {:?} in {:?} failed: {}",
            args,
            dir,
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    pub(crate) fn fresh_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("sw-sync-{}-{}", std::process::id(), name));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    pub(crate) fn setup_identity(dir: &Path) {
        git_in(dir, &["config", "user.name", "Test"]);
        git_in(dir, &["config", "user.email", "t@example.com"]);
    }

    #[test]
    fn pull_push_roundtrip_and_lww_conflict() {
        let base = fresh_dir("base1");
        let bare = base.join("repo.git");
        git_in(
            &base,
            &["init", "--bare", "-b", "main", bare.to_str().unwrap()],
        );

        // A 首次推送
        let a = base.join("a");
        git_in(
            &base,
            &["clone", bare.to_str().unwrap(), a.to_str().unwrap()],
        );
        setup_identity(&a);
        std::fs::write(a.join("hello.md"), "# Hello\n\nfrom A\n").unwrap();
        let st = sync_workspace(&a, "test-remote", "origin").expect("A 首次同步");
        assert!(st.ok && st.pushed == 1);

        // B 拉取
        let b = base.join("b");
        git_in(
            &base,
            &["clone", bare.to_str().unwrap(), b.to_str().unwrap()],
        );
        setup_identity(&b);
        let st2 = sync_workspace(&b, "test-remote", "origin").expect("B 拉取");
        assert!(st2.ok);
        assert_eq!(
            std::fs::read_to_string(b.join("hello.md")).unwrap(),
            "# Hello\n\nfrom A\n"
        );

        // 冲突：A、B 同时改 conflict.md，B 的 mtime 更新 → B 胜
        std::fs::write(a.join("conflict.md"), "# C\n\nversion A (older)\n").unwrap();
        sync_workspace(&a, "test-remote", "origin").expect("A 推送冲突版本");
        std::fs::write(b.join("conflict.md"), "# C\n\nversion B (newer)\n").unwrap();
        std::thread::sleep(std::time::Duration::from_secs(2));
        let st3 = sync_workspace(&b, "test-remote", "origin").expect("B LWW 同步");
        assert_eq!(st3.conflicts, 1);
        assert_eq!(st3.remote_wins, 0, "B 的 mtime 更新，应本地胜");
        let b_content = std::fs::read_to_string(b.join("conflict.md")).unwrap();
        assert!(
            b_content.contains("version B"),
            "B 的较新版本应保留: {b_content}"
        );

        // A 拉取到 B 胜出的版本
        let st4 = sync_workspace(&a, "test-remote", "origin").expect("A 拉取 B 胜出版本");
        let a_content = std::fs::read_to_string(a.join("conflict.md")).unwrap();
        assert!(
            a_content.contains("version B"),
            "A 应拿到 B 胜出的版本: {a_content}"
        );
        assert!(st4.ok);

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn resolve_sync_remote_never_touches_foreign_origin() {
        let base = fresh_dir("resolve");
        // 场景 1：origin 指向 GitHub（外来）→ 用 sync，origin 保持原样
        let ws = base.join("ws");
        git_in(&base, &["init", "-q", "-b", "main", ws.to_str().unwrap()]);
        git_in(
            &ws,
            &[
                "remote",
                "add",
                "origin",
                "https://example.invalid/stillwrite.git",
            ],
        );
        let name = resolve_sync_remote(&ws, "user@example.invalid:~/stillwrite.git").unwrap();
        assert_eq!(name, "sync");
        let (_, origin_url) = git(&ws, &["remote", "get-url", "origin"]).unwrap();
        assert_eq!(
            origin_url, "https://example.invalid/stillwrite.git",
            "外来 origin 不得被改写"
        );
        let (_, sync_url) = git(&ws, &["remote", "get-url", "sync"]).unwrap();
        assert_eq!(sync_url, "user@example.invalid:~/stillwrite.git");

        // 场景 2：origin 已指向默认远端 → 直接复用 origin
        let ws2 = base.join("ws2");
        git_in(&base, &["init", "-q", "-b", "main", ws2.to_str().unwrap()]);
        git_in(
            &ws2,
            &[
                "remote",
                "add",
                "origin",
                "user@example.invalid:~/stillwrite.git",
            ],
        );
        let name2 = resolve_sync_remote(&ws2, "user@example.invalid:~/stillwrite.git").unwrap();
        assert_eq!(name2, "origin");

        // 场景 3：无 origin → 创建 sync
        let ws3 = base.join("ws3");
        git_in(&base, &["init", "-q", "-b", "main", ws3.to_str().unwrap()]);
        let name3 = resolve_sync_remote(&ws3, "user@example.invalid:~/stillwrite.git").unwrap();
        assert_eq!(name3, "sync");
        let (_, sync3) = git(&ws3, &["remote", "get-url", "sync"]).unwrap();
        assert_eq!(sync3, "user@example.invalid:~/stillwrite.git");

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn remote_wins_when_local_mtime_stale() {
        let base = fresh_dir("base2");
        let bare = base.join("repo.git");
        git_in(
            &base,
            &["init", "--bare", "-b", "main", bare.to_str().unwrap()],
        );

        let a = base.join("a");
        git_in(
            &base,
            &["clone", bare.to_str().unwrap(), a.to_str().unwrap()],
        );
        setup_identity(&a);
        std::fs::write(a.join("doc.md"), "v1 from A\n").unwrap();
        sync_workspace(&a, "test-remote", "origin").expect("A 推送 v1");

        let b = base.join("b");
        git_in(
            &base,
            &["clone", bare.to_str().unwrap(), b.to_str().unwrap()],
        );
        setup_identity(&b);
        std::fs::write(b.join("doc.md"), "v2 from B\n").unwrap();
        sync_workspace(&b, "test-remote", "origin").expect("B 推送 v2");

        // A 把本地 mtime 伪造为旧时间后拉取 → 远端（B v2）胜
        let p = a.join("doc.md");
        std::fs::write(&p, "v3 from A (stale mtime)\n").unwrap();
        let f = std::fs::File::open(&p).unwrap();
        let past = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
        f.set_times(std::fs::FileTimes::new().set_modified(past))
            .unwrap();
        drop(f);

        let st = sync_workspace(&a, "test-remote", "origin").expect("A 陈旧 mtime 同步");
        assert_eq!(st.conflicts, 1);
        assert_eq!(st.remote_wins, 1, "本地 mtime 旧 → 远端应胜");
        let content = std::fs::read_to_string(&p).unwrap();
        assert!(content.contains("v2 from B"), "应保留 B 的 v2: {content}");

        let _ = std::fs::remove_dir_all(&base);
    }
}

#[cfg(test)]
mod live_tests {
    use super::tests::{fresh_dir, git_in, setup_identity};
    use super::*;

    const BOARD: &str = "user@example.invalid:~/stillwrite.git";

    #[test]
    #[ignore = "需要配置的远程设备在线（LAN）"]
    fn live_push_pull_and_conflict_against_remote() {
        // 0) 清空板子仓库引用，保证每次确定性执行（裸仓库无工作区，安全）
        let reset = Command::new("ssh")
            .args([
                "-o",
                "BatchMode=yes",
                "example.invalid",
                "git --git-dir=$HOME/stillwrite.git for-each-ref --format='%(refname)' | xargs -r -I{} git --git-dir=$HOME/stillwrite.git update-ref -d {}; git --git-dir=$HOME/stillwrite.git symbolic-ref HEAD refs/heads/main",
            ])
            .output()
            .expect("ssh 执行失败");
        assert!(reset.status.success(), "板子仓库重置失败");

        // 1) 本机工作区 → 推送到板子
        let base = fresh_dir("live");
        let ws = base.join("ws");
        std::fs::create_dir_all(&ws).unwrap();
        git_in(&ws, &["init", "-b", "main"]);
        setup_identity(&ws);
        git_in(&ws, &["remote", "add", "origin", BOARD]);
        let tag = format!("from-validator-{}", unix_ts());
        std::fs::write(
            ws.join("live.md"),
            format!("# Live Test\n\npush tag: {tag}\n"),
        )
        .unwrap();
        let st = sync_workspace(&ws, BOARD, "origin").expect("本机→板子 推送");
        assert!(st.ok, "推送失败: {}", st.message);

        // 2) 在板子上验证内容存在
        let check = Command::new("ssh")
            .args([
                "-o",
                "BatchMode=yes",
                "example.invalid",
                &format!("git --git-dir=$HOME/stillwrite.git show main:live.md | grep -c 'push tag: {tag}'"),
            ])
            .output()
            .expect("ssh 执行失败");
        assert!(
            check.status.success() && String::from_utf8_lossy(&check.stdout).trim() == "1",
            "板子上未找到推送的内容: {}",
            String::from_utf8_lossy(&check.stderr)
        );

        // 3) 在板子上建工作区，改同一文件并推送（模拟设备 B 编辑）
        let board_script = format!(
            "rm -rf /tmp/sw-board-ws && git clone -q ~/stillwrite.git /tmp/sw-board-ws 2>/dev/null; \
             cd /tmp/sw-board-ws && git checkout -q main && git config user.name Board && git config user.email board@example.invalid && \
             echo '# Live Test\n\nfrom board (newer)' > live.md && \
             git add -A && git commit -qm board-edit && git push -q origin main"
        );
        let board_edit = Command::new("ssh")
            .args(["-o", "BatchMode=yes", "example.invalid", &board_script])
            .output()
            .expect("ssh 执行失败");
        assert!(
            board_edit.status.success(),
            "板子侧编辑失败: {}",
            String::from_utf8_lossy(&board_edit.stderr)
        );

        // 4) 本机本地再编辑（mtime 更新），拉取 → 本机（较新）胜
        std::thread::sleep(std::time::Duration::from_secs(2));
        std::fs::write(
            ws.join("live.md"),
            "# Live Test\n\nfrom validator (newest local)\n",
        )
        .unwrap();
        let st2 = sync_workspace(&ws, BOARD, "origin").expect("本机 LWW 同步");
        assert!(st2.ok);
        let local_content = std::fs::read_to_string(ws.join("live.md")).unwrap();
        assert!(
            local_content.contains("newest local"),
            "本机 mtime 较新应本地胜: {local_content}"
        );

        // 5) 板子再拉取一次 → 拿到本机胜出的版本（LWW 收敛）
        let board_pull = Command::new("ssh")
            .args([
                "-o",
                "BatchMode=yes",
                "example.invalid",
                "cd /tmp/sw-board-ws && git pull -q --no-rebase && grep -c 'newest local' live.md",
            ])
            .output()
            .expect("ssh 执行失败");
        assert!(
            board_pull.status.success()
                && String::from_utf8_lossy(&board_pull.stdout).trim() == "1",
            "板子未收敛到本机胜出版本: {}",
            String::from_utf8_lossy(&board_pull.stderr)
        );

        let _ = std::fs::remove_dir_all(&base);
    }
}
