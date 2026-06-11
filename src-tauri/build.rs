fn main() {
    // Tell cargo to rebuild the crate whenever any feedback-related env var
    // changes. option_env! is evaluated at compile time; without these hints
    // cargo will happily reuse a stale build where the vars were None.
    println!("cargo:rerun-if-env-changed=FEISHU_APP_ID");
    println!("cargo:rerun-if-env-changed=FEISHU_APP_SECRET");
    println!("cargo:rerun-if-env-changed=FEISHU_RECEIVE_ID");
    println!("cargo:rerun-if-env-changed=FEISHU_RECEIVE_ID_TYPE");

    // NOVA: 强制监听前端 dist 目录 — 修改 TypeScript 后必须重新嵌入资源
    println!("cargo:rerun-if-changed=../dist");
    println!("cargo:rerun-if-changed=../src");

    tauri_build::build()
}
