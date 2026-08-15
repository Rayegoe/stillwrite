//! 批注功能端到端流程测试：工作区任意文档 → 写批注（标注来源/时间）→ 汇总到 批注汇总.md。
//! 模拟前端完整调用序列（read_annotation → save_annotation → aggregate_annotations），
//! 覆盖拆解章节文档、子目录文档、无标题散记。

use stillwrite_lib::annotate::{self, ANNOTATE_DIR, AGGREGATE_NAME};
use std::fs;
use std::path::PathBuf;

fn tmp_workspace(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("sw-e2e-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn full_annotation_flow_any_doc() {
    let root = tmp_workspace("flow");
    // 拆解后的章节文档
    fs::write(
        root.join("ch01-llm-wiki-是什么.md"),
        "# 第一章：LLM Wiki 是什么\n\n## 核心理念\n",
    )
    .unwrap();
    // 子目录里的普通文档
    fs::create_dir_all(root.join("笔记")).unwrap();
    fs::write(root.join("笔记/随手记.md"), "没有标题结构的散记\n").unwrap();

    // 1. 初始：批注为空
    for rel in ["ch01-llm-wiki-是什么.md", "笔记/随手记.md"] {
        let data = annotate::read_annotation_data(&root, &root.join(rel)).unwrap();
        assert_eq!(data.body, "", "{rel} 初始应为空批注");
        assert_eq!(data.title, if rel.contains('/') { "随手记" } else { "ch01-llm-wiki-是什么" });
    }

    // 2. 写批注（不按章节，整篇一篇）
    annotate::save_annotation(&root, &root.join("ch01-llm-wiki-是什么.md"), "wiki 把知识编译一次，比 RAG 每次现查更省。").unwrap();
    annotate::save_annotation(&root, &root.join("笔记/随手记.md"), "这段想法值得展开。").unwrap();

    // 3. 侧车按原文路径镜像 + 元信息标注来源和时间
    let sidecar = root.join(ANNOTATE_DIR).join("ch01-llm-wiki-是什么.md");
    assert!(sidecar.is_file());
    let content = fs::read_to_string(&sidecar).unwrap();
    assert!(content.contains("> 来源：`ch01-llm-wiki-是什么.md`"));
    assert!(content.contains("> 时间：20"), "侧车应标注时间: {content}");
    assert!(root.join(ANNOTATE_DIR).join("笔记").join("随手记.md").is_file());

    // 4. 读取回来
    let data = annotate::read_annotation_data(&root, &root.join("ch01-llm-wiki-是什么.md")).unwrap();
    assert_eq!(data.body, "wiki 把知识编译一次，比 RAG 每次现查更省。");
    assert!(!data.updated_at.is_empty());

    // 5. 汇总 → 批注汇总.md 在工作区根目录
    let result = annotate::aggregate(&root).unwrap();
    assert_eq!(result.count, 2);
    assert_eq!(result.path, root.join(AGGREGATE_NAME).to_string_lossy().to_string());

    let aggregate_md = fs::read_to_string(root.join(AGGREGATE_NAME)).unwrap();
    assert!(aggregate_md.contains("## ch01-llm-wiki-是什么"));
    assert!(aggregate_md.contains("来源：`ch01-llm-wiki-是什么.md`"));
    assert!(aggregate_md.contains("批注于 "));
    assert!(aggregate_md.contains("## 随手记"));
    assert!(aggregate_md.contains("来源：`笔记/随手记.md`"));

    // 6. 幂等：汇总文件自身不被当作源文档
    let again = annotate::aggregate(&root).unwrap();
    assert_eq!(again.count, 2);
    assert_eq!(fs::read_to_string(root.join(AGGREGATE_NAME)).unwrap(), aggregate_md);

    // 7. 清空一篇批注后重汇总 → 只剩一篇；侧车被删除
    annotate::save_annotation(&root, &root.join("笔记/随手记.md"), "  ").unwrap();
    assert!(!root.join(ANNOTATE_DIR).join("笔记").join("随手记.md").exists(), "空正文应删除侧车");
    let after_clear = annotate::aggregate(&root).unwrap();
    assert_eq!(after_clear.count, 1);
    let md2 = fs::read_to_string(root.join(AGGREGATE_NAME)).unwrap();
    assert!(!md2.contains("这段想法值得展开"));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn updated_time_refreshes_on_resave() {
    let root = tmp_workspace("retime");
    fs::write(root.join("a.md"), "# A\n").unwrap();
    annotate::save_annotation(&root, &root.join("a.md"), "第一版").unwrap();
    std::thread::sleep(std::time::Duration::from_secs(1));
    annotate::save_annotation(&root, &root.join("a.md"), "第二版").unwrap();

    let data = annotate::read_annotation_data(&root, &root.join("a.md")).unwrap();
    assert_eq!(data.body, "第二版");
    assert!(!data.updated_at.is_empty());
    let _ = fs::remove_dir_all(&root);
}
