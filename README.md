# r8

用 Rust 自研 ECMAScript 引擎，按可运行的纵向切片推进。

当前完成 S00 算术与 S01 有状态 Script 子集：源码经词法分析、优先级解析和栈式指令生成，再由 VM 执行。支持脚本文件、语句列表、块作用域、`let` / `const` / `var`、变量读取、十进制数值及一元 `+/-`、二元 `+ - * / %`。完整范围与后续路线见 [ECMAScript 实现计划](.agents/ecmascript-plan.md)。

## 运行

需要 Rust 稳定工具链，在仓库根目录执行：

```sh
cargo run --locked -- -e 'let x = 3; x + 2'
# Expected stdout: 5

cargo run --locked -- evals/cases/s01/declarations.js
# Expected stdout: 5

cargo run --locked -- -e 'let x = 1; { const x = 3; x + 2; }'
# Expected stdout: 5

cargo run --locked -- -e 'x; let x = 3;'
# Expected: ReferenceError on stderr; exit code 1
```

CLI 接收一个 UTF-8 Script 文件路径，或 `-e` / `--eval` 后的源码字符串；二者互斥。位置参数只作文件名，不猜测为源码，不提供 REPL 或 stdin 模式。以 `-` 开头的文件名放在 `--` 后。

两种输入模式均按 JS 数字字符串格式输出非 `undefined` 的 Script 完成值。空 Script、只有声明的 Script 不输出；声明和空语句不覆盖前一条表达式的完成值。成功退出 `0`；语法错误、运行期错误、未实现能力、资源限制或文件读取失败退出 `1`；参数错误退出 `2`。诊断写入 stderr，语言错误区分 SyntaxError、ReferenceError，能力与宿主限制分别标明 Unsupported、ResourceLimit。

### 支持边界

- 标准空白、换行、行/块注释及当前语句语法所需的 ASI；换行不会拆开仍可继续的算术或调用表达式。
- Unicode 17.0 的 `ID_Start` / `ID_Continue` 标识符，补充 `$`、`_`、ZWNJ、ZWJ 和 `\uXXXX` / `\u{...}` 转义；不做 Unicode 规范化。
- 声明列表、块级词法绑定、`var` 提升、TDZ、同作用域重声明冲突；未绑定读取报 ReferenceError。声明初始化不是普通赋值表达式。
- 保留 Number 的 NaN、±0、Infinity 行为。可读取全局 `undefined`、`NaN`、`Infinity`；Undefined 参与当前算术运算转换为 NaN。
- 源码上限 1 MiB、块与表达式合计解析递归上限 128 层；文件读取在超出源码上限后停止。

普通赋值及更新运算留在 S02；strict 指令、其他值类型、解构、控制流、函数、对象、其他内建对象和宿主接口仍未实现。数字分隔符、非十进制数字、旧式前导零数字与 hashbang 也未实现。遇到不支持的语法不执行已解析的前缀。

## 引擎库

```rust
let result = r8::eval("let x = 3; x + 2").unwrap();
assert_eq!(result, r8::Value::Number(5.0));

let script = r8::Script::parse("-0").unwrap();
let r8::Value::Number(number) = script.run().unwrap() else { unreachable!() };
assert!(number.is_sign_negative());

let mut context = r8::Context::default();
context.eval("let x = 3;").unwrap();
assert_eq!(context.eval("x + 2").unwrap(), r8::Value::Number(5.0));
```

`Script::parse` 只解析、检查 early errors 并编译。`Script::run` 和顶层 `eval` 每次使用新上下文；`Context::run` / `Context::eval` 在多个 Script 间保留全局绑定。引入运行期错误后，`Script::run` 返回 `Result<Value, Error>`，不保留旧的无错误返回接口。

每次执行先实例化声明：`var` 提前初始化为 `undefined`，词法绑定在声明执行前保持 TDZ。跨 Script 的全局声明冲突发生于执行期；该阶段的冲突检查完成前不修改已有上下文。运行期错误不回滚已建立的全局绑定，但会释放本次执行的块环境。当前没有完整 Realm、全局对象或 GC；与未实现全局内建对象有关的 `var` 声明明确报告 Unsupported。

## 测试

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo run --locked --bin r8-eval -- evals/cases/s01
```

Rust 集成测试通过真实引擎与 CLI 验证数值位模式、作用域、完成值、错误阶段、跨 Script 上下文和资源边界。`r8-eval` 调用同一引擎库；每个变体使用独立 `Context`，同一变体的 harness 和测试体共享该上下文。

自有 S01 JS 集合包含 12 个 raw 变体，全部可运行：4 个正常完成用例、4 个解析负例、4 个运行期负例；具体完成值由 Rust 集成测试验证，不使用宿主端猜测结果。

自有 S00 JS 集合仍为 22 个文件、44 个变体：5 个非严格模式解析负例通过，39 个变体因 strict 指令、更新运算或完整断言 harness 等缺失能力而 skip，运行该集合的退出码为 `1`。这不是完整 Test262 合规结果。详见[测试说明](evals/README.md)。
