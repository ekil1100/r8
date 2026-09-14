# r8 测试框架

本框架的测试对象只有 r8。用例保存在 `.js` 中，以 JS 断言或 Test262 `negative` metadata 表达预期；Rust 负责发现、准备、隔离与报告。

`src/eval/executor.rs` 的 `execute_r8` 已接入 r8 的 S00 引擎库。raw 算术用例可以执行，范围内的 parse 负例由真实解析器判定；依赖 strict 指令、更新运算、其他值类型或官方断言 harness 的用例仍报告 unsupported/skip。

## 运行

在仓库根目录执行：

```sh
cargo run --bin r8-eval -- evals/cases/s00
```

也可传入单个 `.js` 文件。目录按路径排序递归发现测试，不展开目录中的符号链接；文件名包含 `_FIXTURE` 时不作为独立测试。命令只接收测试及框架选项，不接收外部引擎命令。

保存报告：

```sh
mkdir -p target/eval
cargo run --bin r8-eval -- evals/cases/s00 > target/eval/r8.json
```

默认使用仓库中的官方 harness 副本，[来源、版本与许可证](harness/README.md)单独记录。`--harness` 可指定其他 harness 目录。选项见 `cargo run --bin r8-eval -- --help`。

## 编写用例

普通用例使用官方 `assert.js` 提供的断言。在 r8 能够执行这些断言后，没有未捕获异常即通过。新增自有语言用例时，在文件头填写规范条款 `esid`：

```js
/*---
description: Multiplication binds tighter than addition.
esid: sec-multiplicative-operators
---*/
assert.sameValue(1 + 2 * 3, 7);
```

断言可以检查值、对象身份和副作用，不需要在 Rust 中另写 JS 值比较。避免实际值和预期走完全相同的表达式路径；例如用 `1 / -0` 与 `-Infinity` 检查负零。

预期异常同时声明阶段和类型：

```js
/*---
description: Addition requires a right operand.
esid: sec-syntactic-grammar
negative:
  phase: parse
  type: SyntaxError
---*/
1 +
```

需要观察独立 Script 的完成值时，用例可调用 `$262.evalScript`；该宿主接口也需要在 r8 中实现，目前不能执行。

`cases/s00/` 检查 S00 算术行为，但 JS 断言依赖函数、对象、异常等能力。目前 44 个变体中有 5 个非严格模式解析负例通过、39 个 skip；算术值、负零等由 `tests/engine.rs` 和 `tests/engine_cli.rs` 通过同一真实执行链路验证。阶段边界见[实现计划](../.agents/ecmascript-plan.md)。

## 当前已实现的准备流程

文件头支持 `/*--- ... ---*/` YAML 块。识别 `negative`、`flags`、`includes`、`features`、`locale`、`esid`，接受 `description`、`info`、`author`、`es5id`、`es6id` 等说明字段。未知字段、无效 YAML、互斥模式标记属于配置错误。

| 标记 | 变体准备规则 |
| --- | --- |
| 无模式标记 | non-strict / strict 两个变体 |
| `onlyStrict` | 只准备 strict，在测试体前添加 `"use strict";` 与换行 |
| `noStrict` | 只准备 non-strict，不改写源码 |
| `raw` | 单个变体，不改写源码，也不读取 harness 文件 |
| `generated`、`non-deterministic` | 信息标记，不改变模式 |
| `async`、`module`、CanBlock 标记及未知 flags | 暂不支持，明确 skip |

非 raw 用例按 `assert.js`、`sta.js`、`includes` 的顺序准备源码。includes 限于指定 harness 目录，拒绝越界路径及指向目录外的符号链接。IsHTMLDDA、locale 条件和 resolution 负例目前也明确 skip。其他 features 仅保留作分类信息，不据此猜测能力支持度。

读取 harness 文件成功，只说明输入已准备好，不代表 r8 已执行 harness 或测试体。

## r8 执行器

每个可准备的变体启动同一 Rust 二进制的内部 worker，不从 PATH 查找引擎。JSON 是父子进程之间的内部传输格式，不是外部引擎插件协议。

`executor.rs::execute_r8` 与 CLI 共用 `r8::Script::parse` 和 `Script::run`：

1. 单独解析并编译测试体，区分真实语法错误、未实现能力和宿主资源限制。`parse_only` 请求到此结束，不执行测试体或解析/执行 harness；解析成功不能满足 parse 负例。
2. 需要求值时，按准备顺序解析并执行 harness，再运行测试体。harness 的未实现能力报告 skip，其他 harness 故障报告 harness-error；均不能满足测试体的 negative 预期。输入文件存在性和路径安全检查仍在 worker 启动前完成。
3. 正常完成报告 completed；语法错误报告 parse 阶段与 SyntaxError。`negative` 同时匹配阶段与类型。宿主资源限制报告 harness-error，不伪装为语言异常。
4. 缺失能力保持 unsupported；更新运算等未支持语法中的负例也不能猜测为 SyntaxError。执行到未知语法时不采信已解析的前缀。

S00 只有无状态算术和独立操作数栈，暂无 Realm、全局绑定或 `$262` 接口。引入状态后，同一变体的 harness 和测试体必须共享执行上下文，不同变体保持隔离；引入语言异常后再补 runtime 类型映射。完整 Test262 集合、异步、Module、多 Realm、GC、buffer detach 和 agent 接口按实现计划逐项补齐。

## 报告与资源限制

报告固定标识测试对象为 `r8`，区分文件数 `files` 与变体数 `summary.total`，以文件 `id` 加 `variant` 唯一定位一次执行。保留源码、metadata、harness 目录、worker 结果、诊断、stderr 和耗时。准备阶段直接跳过的变体没有 `actual`。

| 状态 | 判定 |
| --- | --- |
| `pass` | r8 正常完成，或真实异常的阶段与类型符合预期 |
| `fail` | 断言失败、非预期异常或预期异常未发生 |
| `skip` | 未支持的执行条件或 r8 能力；仍计入总数 |
| `timeout` | worker 达到时间上限并被终止 |
| `crash` | worker 非零退出或被信号终止；不采信其语言结果 |
| `harness-error` | 输入准备、启动、I/O、内部协议、输出限制或 harness 故障 |

- 全部变体通过时退出 `0`，存在非通过结果退出 `1`；参数、文件读取或 metadata 校验错误退出 `2`。空集合属于配置错误。
- 一个变体失败后继续处理后续变体。worker 默认限时 2000 ms，可用 `--timeout-ms` 调整。
- stdin/stdout/stderr 使用临时文件，单个输出流最多采集 1 MiB，超限终止 worker。轮询检测不是严格磁盘配额。
- 只回收直接子进程；未来测试宿主派生的进程由宿主负责管理。耗时包含进程启动，不作为解释器吞吐量基准。
- 进程隔离不是安全沙箱；只运行可信用例，不隔离宿主文件、网络或凭据。

框架自检通过公开 CLI 验证发现、metadata、输入边界、无外部程序依赖、raw 执行、parse 负例、harness 失败与资源限制分类。完整断言、运行期异常和宿主行为仍待对应能力实现后验收。
