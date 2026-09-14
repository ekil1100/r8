# r8

用 Rust 自研 ECMAScript 引擎，按可运行的纵向切片推进。

当前实现 S00 算术子集：源码经词法分析、优先级解析和栈式指令生成，再由 VM 执行。支持十进制整数、小数、指数、一元 `+/-`、二元 `+ - * / %`、括号、可选尾分号及空 Script。完整范围与开发路线见 [ECMAScript 实现计划](.agents/ecmascript-plan.md)。

## 运行

需要 Rust 稳定工具链，在仓库根目录执行：

```sh
cargo run --locked -- -e '1 + 2 * 3'
# Expected stdout: 7

cargo run --locked -- -e '(1 + 2) * 3'
# Expected stdout: 9

cargo run --locked -- -e '1 +'
# Expected: SyntaxError on stderr; exit code 1
```

结果按 JS 数字字符串格式显示，空 Script 不输出内容。成功退出 `0`；语法错误、未实现能力或资源限制退出 `1`，并在 stderr 中分别标明 SyntaxError、Unsupported、ResourceLimit；参数错误退出 `2`。

标准空白、换行和行/块注释已支持。数字分隔符、非十进制数字、旧式前导零数字、更新运算、strict 指令、变量、语句列表、其他值类型及宿主接口仍未实现；不忽略不支持的尾随源码。当前源码上限 1 MiB、解析递归上限 128 层。

## 引擎库

```rust
let result = r8::eval("1 + 2 * 3").unwrap();
assert_eq!(result, r8::Value::Number(7.0));

let script = r8::Script::parse("-0").unwrap();
let r8::Value::Number(number) = script.run() else { unreachable!() };
assert!(number.is_sign_negative());
```

`Script::parse` 只解析和编译，`run` 使用独立操作数栈执行内部指令。库保留 `-0` 位模式；CLI 按标准将它显示为 `0`。目前没有有状态执行上下文或堆。

## 测试

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo run --locked --bin r8-eval -- evals/cases/s00
```

Rust 集成测试验证真实引擎的完成值、错误路径、资源边界与 CLI。`r8-eval` 同样调用 r8 引擎库，不依赖外部引擎；`.js` 用例使用 JS 断言或 Test262 `negative` metadata 表达预期。

自有 S00 JS 集合目前有 22 个文件、44 个变体，其中 5 个非严格模式解析负例通过，39 个变体仍为 skip，故退出 `1`。依赖 strict 指令、更新运算或断言 harness 的测试尚不能执行；这不是完整 Test262 合规结果。细节见[测试说明](evals/README.md)。
