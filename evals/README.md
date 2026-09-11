# r8 测试框架

本框架的测试对象只有 r8。用例保存在 `.js` 中，以 JS 断言或 Test262 `negative` metadata 表达预期；Rust 负责发现、准备、隔离与报告。

**当前只有框架，尚无 r8 解析器和 VM。** `src/eval/executor.rs` 的 `execute_r8` 是执行接入点，目前明确返回 unsupported。已有用例不能执行，因此报告 skip，而不是通过。

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

`cases/s00/` 检查 S00 算术行为，但 JS 断言依赖函数、对象、异常等能力。引擎尚不能执行 harness 时，按[实现计划](../.agents/ecmascript-plan.md)用调用真实 r8 执行链路的 Rust 集成测试建立早期保障。

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

## 接入 r8 执行器

每个可准备的变体启动同一 Rust 二进制的内部 worker，不从 PATH 查找引擎。JSON 是父子进程之间的内部传输格式，不是外部引擎插件协议。

r8 引擎库建立后，在 `executor.rs::execute_r8` 中接入同一套解析器和 VM，按以下约定逐项实现并验收：

1. 为每个变体创建独立的 r8 执行上下文，提供该切片所需的测试宿主接口。
2. 单独解析测试体，区分语法/early errors 与未实现能力。`parse_only` 请求只解析，不执行测试体或 harness；解析成功不能满足 parse 负例。
3. 需要求值时，先在测试上下文全局作用域中执行准备好的 harness，再执行测试体。harness 初始化或执行失败单独报告为 harness-error，不能满足测试体的 negative 预期。
4. 正常完成报告 completed；未捕获异常报告真实阶段、错误类型和诊断。`negative` 必须同时匹配阶段与类型。
5. 缺失的引擎能力和宿主接口保持 unsupported。只有真实执行完成才能报告通过，不根据源码内容猜测结果，也不调用其他引擎代跑。

目前上述 JS 执行能力均待接入。完整 Test262 集合、异步、Module、多 Realm、GC、buffer detach 和 agent 接口按实现计划逐项补齐。

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

框架自检通过公开 CLI 验证发现、metadata、输入边界、无外部程序依赖以及未实现状态的报告。r8 接入后，还需用真实执行回归验证断言、异常、宿主行为及资源限制；当前自检成功不代表这些语言行为已通过。
