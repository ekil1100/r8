# Test262 harness 来源

`assert.js` 与 `sta.js` 原样取自 [tc39/test262](https://github.com/tc39/test262/tree/72faf8ec1445c55149615e8b35187830783aba1a/harness)，固定 revision 为 `72faf8ec1445c55149615e8b35187830783aba1a`。

修改 runner 的解释规则或升级这些文件前，先阅读该 revision 的 [INTERPRETING.md](https://github.com/tc39/test262/blob/72faf8ec1445c55149615e8b35187830783aba1a/INTERPRETING.md)。升级时同时检查上游文件、执行规则与框架回归，更新本记录；不在副本中改写断言语义。

| 文件 | SHA-256 |
| --- | --- |
| `assert.js` | `206e274ca325eb8a652e3911c3fbd090e2480d11ed7579dc17a5d17a2360ed48` |
| `sta.js` | `1930c54af79455c484799f43e9a28e2b2f15c40d0917c9941ca54e26db243f35` |

保留上游版权声明与 [BSD 许可证](LICENSE)。许可证文本仅去除了行末空白。

这里只固定基础 harness，不包含完整 Test262 测试集。`../cases/` 下是 r8 自有用例；正式测试集的 revision、适用范围和通过清单在后续接入时单独记录。
