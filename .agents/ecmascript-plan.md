# r8 ECMAScript 实现计划

状态：待实施。当前切片：S00。

目标：用 Rust 自研 ECMAScript 引擎，先交付可运行 MVP，再通过纵向切片扩展到固定版本的标准。开发方式与源码参考遵循 [AGENTS.md](../AGENTS.md)；本计划定义范围、顺序和验收依据。

## 1. 标准基线与边界

- **目标版本**：ECMA-262 第 17 版，ECMAScript 2026。以[已发布版本][spec]为规范依据，用[固定年份的分章节版本][spec-pages]查阅；后续草案与提案通过独立的基线升级纳入。
- **兼容目标**：实现该版本要求的语法、类型、抽象操作、执行语义与内建对象，包括严格模式和非严格模式。阶段性产物称为“ECMAScript 子集引擎”，只有满足最终验收条件后才声明目标版本兼容。
- **可选条款**：初始定位为非浏览器宿主，Annex B 的浏览器兼容行为默认不启用。其他 Normative Optional 条款逐项记录选择；启用某项时实现其所属可选条款的全部行为。仅标记为 Legacy、未标记为 Normative Optional 的内容仍属于实现范围。[依据][conformance]
- **引擎与宿主**：引擎负责语言语义；CLI 与测试宿主提供输入输出、模块解析、时钟、Job 调度和必要的多 agent 支持。Promise、模块、SharedArrayBuffer、Atomics 属于标准范围，不能因为没有浏览器或 Node.js 就跳过。
- **独立范围**：ECMA-402 / `Intl`、DOM、Web APIs、Node.js APIs、npm、TypeScript 和 WebAssembly 不属于本轮目标。不提供 `Intl` 时，仍按 ECMA-262 实现其规定的相关方法与宿主行为。
- **性能范围**：先实现单一字节码解释执行路径。JIT、快照、并行 GC 和高级对象布局优化按测量结果另立计划；规范要求的 proper tail calls 是语言语义，不能当作可选性能优化省略。

标准正确性以规范为准，Test262 是验证证据，其他引擎是实现参考。Test262 官方明确说明其覆盖并不完整；通过测试集不能单独证明完整符合标准。[依据][test262]

## 2. 执行方式与验收门槛

路线表中的阶段是功能族，不是一次交付单位。进入一个阶段后，只展开当前能端到端交付的小切片，例如先让一个数组方法正确执行，再扩展下一个方法。

每个切片依次完成：

1. **定义行为**：给出 JS 输入、预期值或错误，关联规范条款，写明支持边界与前置能力。
2. **打通执行**：按该行为修改所需的解析、编译、VM、内建对象或宿主部分，保持同一条可运行链路。
3. **验证语义**：正常路径、适用的错误路径和相关边界均有端到端测试；涉及副作用时验证求值顺序，涉及堆或暂停执行时验证 GC 下的存活性。
4. **记录完成**：新增测试与已有回归全部通过，更新规范覆盖记录和当前切片，再进入下一切片。测试中的已知失败属于尚未完成的范围。

当前切片的规范行为必须正确；尚未支持的能力单独报告为未实现，不能用占位值冒充结果，也不能把未实现错误算成 Test262 预期的规范异常。

### 早期技术取舍

- 从一个 Cargo package 开始，包含引擎库和薄 CLI；随实际职责增长拆分模块。
- 初始使用最小解析器、编译器和栈式字节码 VM，不先实现完整前端，也不并行维护 AST 解释器等第二条执行路径。
- Number 从 IEEE 754 双精度值开始；引入 String 时即保留 UTF-16 code unit 语义，包括孤立代理项。值表示随切片扩展，NaN-boxing 不作为启动前提。
- 首次出现可逃逸闭包和循环引用时引入最小可用的追踪式 GC 与显式根管理；用实际执行用例驱动，不另开一个“先完成 GC”的阶段。
- Unicode、BigInt、日期和正则相关库在对应切片到来时评估。依赖能处理数据结构，不意味着其语义天然符合 ECMAScript。

### 拟定 CLI 契约

以下是实施目标，当前尚无可执行程序：

- S00 提供 `r8 -e '<source>'`；S01 增加 Script 文件入口；S16 增加显式 Module 文件入口。
- `-e` 执行 Script，并在结果非 `undefined` 时输出其字符串表示。CLI 展示行为与引擎返回的语言值分开测试。
- 成功退出码为 `0`；语法错误、未捕获异常或执行失败为 `1`；命令行参数错误为 `2`。诊断写入 stderr，并区分标准异常、未实现和宿主资源限制。
- S04 引入宿主函数 `print`，供 CLI 和测试观察副作用；它不是 ECMAScript 标准内建对象。
- S14 起由宿主驱动 Promise Jobs。执行结果与 Job 完成情况分别处理，不能把队列为空等同于所有 Promise 已兑现。

## 3. S00：第一个可运行 MVP

### 可观察结果

```sh
cargo run -- -e '1 + 2 * 3'
# Expected stdout: 7

cargo run -- -e '(1 + 2) * 3'
# Expected stdout: 9

cargo run -- -e '1 +'
# Expected: SyntaxError on stderr; exit code 1
```

### 本切片范围

- 接收源码，识别十进制数值、空白、一元正负号、`+ - * / %`、括号以及表达式后的可选分号。
- 打通 `CLI → 解析 → 字节码 → VM → 结果/错误`，按 JS 的优先级和结合性求值。
- 支持空 Script 的完成结果，完整消费输入；首个表达式后的多余内容也必须被处理或明确拒绝。
- 保留 `NaN`、正负零、无穷大的 Number 语义；除零不是整数除法异常。
- 工程初始化、必要模块、集成测试与这个可执行能力一起交付；对象、函数、GC、JIT 和完整 Test262 runner 留待对应能力出现。

### 完成条件

- 上述三条命令符合预期。
- 端到端测试覆盖：优先级、嵌套括号、左结合、负数、小数、余数、除零、`0 / 0`、`1 / -0`、空输入、缺失操作数、括号不匹配和尾随非法内容。
- 通过引擎值测试区分 `+0` 与 `-0`，而非仅比较 CLI 输出。
- Rust 格式、lint 与测试通过；有效输入和非法输入都不会引发 Rust panic。
- 产出可复现的构建与运行说明；本阶段完成后再展开 S01。

## 4. 后续纵向路线

按顺序推进；每行的例子只是首个验收入口，不代表该功能族已经全部覆盖。调整顺序时先核对前置能力，并更新本表。

| 阶段 | 可运行能力与验收入口 | 逐步补齐的语义及关键验收 |
| --- | --- | --- |
| S01 | 运行有状态脚本：`let x = 3; x + 2` 得到 `5` | Script 文件入口、语句列表、标识符、块作用域、`let` / `const` / `var`、声明实例化、提升、TDZ、重声明和未绑定名称错误；按输入需要扩展注释、ASI、Unicode 标识符与转义。 |
| S02 | 运行多类型表达式：`'2' + 1` 得到 `'21'`，`'2' - 1` 得到 `1` | Boolean、Null、Undefined、String；原始值转换、其余一元运算（含 `typeof` / `void`）、算术/位运算、更新/赋值、比较、相等、逻辑短路与条件运算。字符串按 UTF-16 处理；对象参与的转换在 S05 后补齐。 |
| S03 | 执行实际计算：循环累加 `1..10` 得到 `55` | `if`、`while`、`do`、`for`、`switch`、标签、`break` / `continue`；测试嵌套跳转、短路副作用及循环词法绑定。 |
| S04 | 调用并保留闭包：计数器两次调用得到 `1`、`2` | 普通函数声明/表达式、调用帧、参数、`return`、递归、词法捕获、严格模式和非严格模式的基本调用行为；增加 `print`。闭包和环境进入可追踪堆，在强制 GC 下仍保持状态，不可达循环环境可回收。 |
| S05 | 操作数据与构造对象：`let o = {x: 1}; o.x = 3; o.x` 得到 `3` | 普通对象、数组字面量和基础索引、属性读写、原型链、`this`、`new`、函数对象、描述符和访问器、`in` / `instanceof`、删除与 `for...in` 枚举；按需增加 `Object` / `Array` 基础方法。测试属性顺序、`length`、对象转换副作用和循环对象回收。 |
| S06 | 捕获与传播失败：`try { throw 3; } catch (e) { e + 1; }` 得到 `4` | `throw`、`try/catch/finally`、标准 Error 类型及构造行为、`cause`、`Error.isError`；验证 `finally` 对正常完成、返回和跳转的覆盖。支持运行 Test262 基础 harness 后接入首批完整测试，区分错误类型与发生阶段。 |
| S07 | 执行符号与任意精度计算：`9007199254740993n + 2n` 得到 `9007199254740995n` | BigInt、Symbol、包装对象、基础 Number / Boolean / String / Math 方法、数值解析和 URI 全局函数；测试 Number/BigInt 混用错误、Symbol 身份、原始值装箱与接收者校验。 |
| S08 | 消费可迭代数据：`for...of` 累加数组得到预期总和 | well-known Symbols、同步迭代协议、自定义迭代器、迭代器关闭、`Iterator` 及 helpers，包括目标版本中的 `Iterator.concat`。验证惰性、副作用顺序，以及中断或异常时的关闭行为。 |
| S09 | 执行数据处理：`[1, 2, 3].map(function (x) { return x * 2; }).join(',')` 得到 `'2,4,6'` | 逐项实现 Array、Object 数据处理方法、JSON、Map、Set；包含稀疏数组、稳定排序、species、分组、集合运算、`getOrInsert` 系列和 `Math.sumPrecise`。验证修改集合时的迭代、键相等、JSON 循环错误和回调异常。 |
| S10 | 运行现代函数：`const add = (a, b = 1) => a + b; add(2)` 得到 `3` | 箭头函数、默认/rest 参数、解构、展开、模板字符串与 tagged templates、可选链、空值合并、逻辑赋值；补齐 `arguments`、`call/apply/bind`、`new.target` 和函数元数据。严格模式 proper tail calls 用有界调用帧深度验收。 |
| S11 | 运行类实例：构造计数器类并调用方法得到更新后的值 | 类声明/表达式、继承、`super`、字段、私有元素、静态块、私有品牌检查、内建对象子类化；验证初始化顺序、错误接收者、派生构造器与 GC 根。 |
| S12 | 执行文本匹配：`/(?<x>a+)b/u.exec('aab').groups.x` 得到 `'aa'` | 正则字面量/构造器、捕获、前后向断言、反向引用、全部目标 flags、Unicode 属性和集合、`lastIndex`、`RegExp.escape`、内联修饰符；完成 String 与正则协议。选库前验证其 ECMAScript 语义缺口，不能直接用 Rust `regex` 行为替代。 |
| S13 | 读写二进制：`new Uint8Array([255, 256])[1]` 得到 `0` | ArrayBuffer、DataView、全部 TypedArrays（含 Float16）、转换和排序、resize/transfer/detach、Uint8Array Base64/Hex 方法；测试越界、长度跟踪、字节序、buffer 失效和用户回调引发的状态变化。 |
| S14 | 执行 Promise 回调：`void Promise.resolve(3).then(x => print(x + 1))` 输出 `4` | Promise 构造、thenable 同化、反应链、组合器、`withResolvers`、`try`、拒绝追踪和 Job 队列；验证单次 settle、FIFO 次序、链式传播及异步完成/超时。 |
| S15 | 暂停并恢复执行：生成器依次产出 `1`、`2`；`async` 函数经 `await` 返回结果 | 先同步 generators，再 async functions、async generators、`for await...of` 和 `Array.fromAsync`；验证 `next/throw/return`、跨暂停点的 `finally`、关闭行为以及暂停帧和 Jobs 的 GC 根。 |
| S16 | 执行多文件模块：导入导出的绑定并计算得到预期值 | 模块入口、解析/链接/求值、live bindings、命名空间对象、循环依赖、重导出、import attributes、JSON 模块、`import()`、`import.meta` 和顶层 await。宿主先支持明确的本地解析规则；测试失败阶段、重复求值和异步依赖，未完成的求值不能报成功。 |
| S17 | 动态执行与拦截：`let x = 1; eval('x = 2'); x` 得到 `2`；Proxy 读取触发 trap | direct/indirect eval、动态 Function 构造、非严格模式 `with`、Proxy、Reflect、多 Realm 与跨 Realm 对象；测试环境选择、严格模式差异、代理不变量、撤销和跨 Realm 内建身份。 |
| S18 | 处理日期与宿主数据：`new Date(0).toISOString()` 得到 `'1970-01-01T00:00:00.000Z'` | 完整 Date 行为、UTC/本地时间、无效时间、时区转换与夏令时边界；补齐数值格式化和实现近似的 Math 行为。为时钟、随机数、时区数据及无 ECMA-402 时的方法行为记录宿主选择并分别测试。 |
| S19 | 运行弱引用数据结构：活跃对象可从 WeakMap 查询，WeakRef 在同一 Job 中保持读取一致性 | WeakMap、WeakSet、WeakRef、FinalizationRegistry、允许弱持有的 Symbol、弱键条件追踪及清理 Jobs。用测试宿主触发 GC 验证可达性；按规范允许的非确定性验收，不承诺固定回收或 finalizer 时刻。 |
| S20 | 运行共享内存程序：`Atomics.add` 返回旧值并更新共享 TypedArray；两个 agent 能交换数据 | SharedArrayBuffer、growable shared buffer、全部 Atomics 方法、wait/notify/waitAsync、Agent Records 与内存模型。先建立安全的共享访问规则，再实现多 agent 测试宿主；普通共享访问也必须避免 Rust 数据竞争 UB，不要求实现 Web Worker API。 |
| S21 | 运行目标版本的全部必需语法与内建行为 | 按覆盖记录逐项关闭剩余缺口：词法上下文、Unicode、ASI、early errors、strict/sloppy 差异、内建属性描述符与原型关系、继承/构造交互、宿主抽象操作和 Forbidden Extensions。每个缺口仍以独立可运行用例交付，不用一个“补齐标准”大任务替代。 |
| S22 | 验收目标版本兼容性 | 满足第 7 节全部条件，发布规范基线、宿主配置、测试 revision、覆盖结果与资源限制；明确与浏览器/Node.js 运行环境的差异。 |

### 顺序中的硬依赖

- S04 的闭包存活性与 S05 的对象身份是后续 GC、调用与内建行为的基础；S06 后才能把依赖完整异常语义的 Test262 测试纳入通过集合。
- S08 的迭代协议支撑集合、展开与多种内建方法；涉及迭代器的后续特性必须复用同一套关闭及异常传播规则。
- S14 的 Jobs 和 Promise 支撑 S15 的异步执行与 S16 的异步模块求值。
- S19 依赖可验证的 GC 根、Jobs 与 Realm；S20 依赖 TypedArrays、共享访问策略及测试宿主。
- 后续特性可能要求修改早期实现。只调整当前需求涉及的部分，并用已有用例守住已交付能力。

## 5. Test262 接入与持续验证

### 分期接入

- **S00–S05**：先用 Rust 集成测试验证同一条源码执行链路。可选择不依赖未实现能力的原始 Test262 用例，但只有满足其全部运行约定才计入正式结果；完整 runner 不阻塞 MVP。
- **S06**：固定 Test262 commit 和解释规则版本，先让原始 `assert.js`、`sta.js` 及所选 `includes` 在引擎中正确执行，再提交第一批正式通过清单。runner 使用同一引擎库，不为测试另造一套语言实现。
- **S14–S20**：随语言能力补上异步完成协议、模块 fixtures、多 Realm、GC、buffer detach 和 agent 测试接口，每项接口均有 runner 自测。

### 执行约定

实施或修改 runner 前读取 [INTERPRETING.md][interpreting]，并遵循所固定 revision 的规则，尤其是：

- 每个测试变体使用独立 Realm；按 `onlyStrict`、`noStrict`、`module`、`raw` 等标记选择执行方式。默认 strict/non-strict 双跑不能漏计；`raw` 不注入 harness、不改写源码。
- 按顺序装载 harness；将测试体的语法/early errors、模块 resolution 错误与 runtime 异常分开判断。负例同时匹配阶段和错误类型，harness 失败不能满足测试体的预期异常。
- 异步测试遵守 `$DONE` / 完成信号及超时规则；`_FIXTURE` 文件只作依赖，不独立运行；并发测试遵守 `CanBlock` 与 agent 约定。
- 测试宿主 API 与 CLI 扩展不计入标准内建对象。未知 metadata、缺失接口和无效 fixtures 显式报告，不能静默算作通过。

### 统计与回归

- 依据固定标准逐项区分：目标版本必需、已选择的可选行为、范围外的 ECMA-402/提案/宿主测试。`features` 等 metadata 只是分类线索；`staging` 也可能包含范围内用例，不能按目录整批排除。
- 分开报告 `pass`、`fail`、`skip`、`timeout`、`crash`、`harness-error`；记录文件数和实际 strict/non-strict 等变体数。
- 每个 skip 有条款或能力依据；目标范围内的未实现测试仍计入总分母。同步公布“目标范围进度”和“当前已选切片通过情况”，避免选择性通过率。
- 每个切片运行新增用例与全部已通过回归；阶段完成或标准覆盖策略变化时运行固定基线的完整适用集合。升级 Test262 必须单独检查新增测试与判定变化。
- 差分测试用于发现与 V8 / Boa 等实现的分歧，最终按规范裁定。发现问题先保存最小 JS 回归，再修复。

## 6. 规范覆盖记录

接入 Test262 时建立唯一的机器可读覆盖记录，后续报告从该记录和实际测试结果生成。该记录覆盖固定规范的目录，不只覆盖路线表中的示例。

每项至少包含：规范 anchor、可观察行为、所属切片、实现状态、验收用例、Test262 关联、适用宿主条件及缺口说明。状态使用“未实现 / 部分实现 / 已验证 / 规范不适用”；“测试还跑不了”属于前两者。

首次建表和版本升级时，按以下入口核对遗漏：

| 规范领域 | 覆盖要求 | 主要归属 |
| --- | --- | --- |
| 类型与抽象操作 | 原始值、对象、转换、相等、Completion、Reference、Property Descriptor；按使用场景实现并验证 | S00–S10，S21 |
| 源码与执行语义 | 全部目标语法、词法目标、静态语义、early errors、Environment Records、Execution Contexts、严格模式、proper tail calls | S00–S17，S21 |
| 对象与内建对象 | 按规范列出每个构造器、方法、属性、内部方法和 exotic object 行为；包含元数据及继承关系 | S04–S20 |
| Script、Module 与宿主 | Realm、声明实例化、模块状态机、Jobs、host-defined/implementation-defined/implementation-approximated 行为 | S01，S14–S18，S20 |
| 管理内存与并发 | 弱引用、清理、Agent/Agent Cluster、共享内存模型及原子操作 | S04–S05，S19–S20 |
| 可选与遗留内容 | 根据 Conformance 的规则逐项判定，记录所选条款及必须成组实现的行为；正文中的相关标记也要检查 | 基线建立、S21 |

某条款暂时没有 Test262 覆盖时，补充自有端到端测试与条款核对；不能因没有测试而把它标记为已验证。后续版本的新特性只有在基线升级后才进入必需集合。

## 7. 最终完成条件

以下条件全部满足，S22 才可完成：

- 固定版本所有必需的可观察语义都有实现与验证记录；已选 Normative Optional 条款完整实现，没有“未分类”“部分实现”或以宿主边界掩盖的语言缺口。
- 固定 Test262 适用集合全部通过；其中没有由未实现导致的 skip、fail、timeout、crash 或 harness-error。确有测试问题时先记录规范依据、修正测试基线，再验收。
- 自有端到端与回归测试通过，覆盖关键跨特性交互：闭包/GC、访问器/异常、迭代器关闭、Proxy/内建方法、跨 Realm、异步暂停/GC、循环模块与共享内存。
- 对解析、编译和执行路径做有资源上限的模糊测试；观察到的崩溃、失控资源使用与内存安全问题均有处理结果。安全限制和标准异常分别报告。
- 从干净环境可以复现构建、CLI 示例和测试报告；每个声明支持的平台均有验证记录。
- 发布说明列明标准版本、Test262 revision、Unicode/时区数据依据、宿主选择、可选条款、支持平台与资源限制。性能数据来自实际基准，不由测试通过率推断。

完成标准兼容后，再以实测瓶颈决定 inline cache、对象布局优化或 JIT 的后续切片；这些优化始终复用既有标准回归。

## 8. 主要依据

- [ECMA-262 已发布版本索引][editions]：确认年度版本。
- [ECMAScript 2026 固定版本][spec]及[分章节版本][spec-pages]：语言与内建对象的规范来源。
- [Conformance][conformance]与[Annex B][annex-b]：必需行为、Legacy 与 Normative Optional 的边界。
- [Test262 项目说明][test262]：测试范围与覆盖局限。
- [Test262 执行约定][interpreting]：runner、metadata、宿主接口和结果判定；实际执行以锁定 commit 中的文件为准。

[editions]: https://ecma-international.org/publications-and-standards/standards/ecma-262/
[spec]: https://262.ecma-international.org/17.0/
[spec-pages]: https://tc39.es/ecma262/2026/multipage/
[conformance]: https://tc39.es/ecma262/2026/multipage/conformance.html
[annex-b]: https://tc39.es/ecma262/2026/multipage/additional-ecmascript-features-for-web-browsers.html
[test262]: https://github.com/tc39/test262
[interpreting]: https://github.com/tc39/test262/blob/main/INTERPRETING.md
