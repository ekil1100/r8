# r8

用 Rust 自研 ECMAScript 引擎，按可运行的纵向切片推进。

测试框架只面向 r8：用例以 `.js` 保存，通过 JS 断言或 Test262 `negative` metadata 表达预期。

**当前解析器和 VM 尚未实现。** 测试发现、准备、原生 worker 与报告链路可运行，但 JS 执行入口仍待接入，当前用例全部报告为未实现，不代表语言测试通过。开发路线见 [ECMAScript 实现计划](.agents/ecmascript-plan.md)。

## 运行测试集合

需要 Rust 稳定工具链，在仓库根目录运行：

```sh
cargo run --bin r8-eval -- evals/cases/s00
```

当前集合包含 22 个文件、44 个执行变体，均为 skip，命令退出码为 `1`。JSON 报告写入 stdout，摘要写入 stderr。

用例编写、执行接入点与判定约定见 [测试说明](evals/README.md)。

## 框架自检

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

这里检查框架自身的行为；在 r8 执行器接入之前，框架自检成功不表示 JS 用例通过。
