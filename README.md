# json_repair

修复大语言模型输出中格式异常的 JSON，**单次遍历**即可完成。

## 解决的问题

LLM 输出的 JSON 常见以下错误，`json_repair` 均可在一次遍历中修复：

| 错误类型 | 输入示例 | 修复结果 |
|----------|----------|----------|
| 字符串内未转义引号 | `"He said "hello""` | `"He said \"hello\""` |
| Python 三引号字符串 | `"""text"""` | `"text"` |
| CSV 风格 `""` 转义 | `"Col1""Data"` | `"Col1\"Data"` |
| 单引号字符串 | `{'key': 'val'}` | `{"key": "val"}` |
| 无引号 key | `{key: "val"}` | `{"key": "val"}` |
| 尾部逗号 | `{"a": 1,}` | `{"a": 1}` |
| 缺失逗号/冒号 | `{"a": 1 "b": 2}` | `{"a": 1, "b": 2}` |
| Python 字面量 | `True / False / None` | `true / false / null` |
| 注释 | `// comment` | 跳过 |
| 截断 JSON | `{"a": 1` | `{"a": 1}` |
| 字符串内控制字符 | 字面换行 / Tab | `\n` / `\t` |
| 前缀/后缀文本 | `Here is JSON: {...}` | `{...}` |
| 无效转义序列 (v0.1.1) | `"\*keeper, \(d_i\)"` | `"\\*keeper, \\(d_i\\)"` |
| JS 字面量 (v0.1.2) | `NaN, Infinity, undefined` | `null` |
| 隐含对象序列 (v0.1.3, ≥8KB) | `{...}, {...}, {...}` | `[{...}, {...}, {...}]` |
| 尾部垃圾数据 (v0.1.4) | `{"a":1}-lnd\nuser\n...` | `{"a":1}` |
| 前导逗号跳过 (v0.1.5) | `[,1]` | `[1]` |
| 点号数字归一化 (v0.1.5) | `.5` / `5.` | `0.5` / `5.0` |
| 相邻对象包裹 (v0.1.5) | `}{` (≥8KB, ≥3组) | `[{...},{...}]` |

## 安装

```bash
pip install git+https://gitee.com/mensui/json_repair.git
```

或使用 uv：

```bash
uv add git+https://gitee.com/mensui/json_repair.git
```

## 使用

```python
from json_repair import repair_json

# 修复 LLM 输出的异常 JSON
broken = '{"response": "He said "hello" to me"}'
fixed = repair_json(broken)
print(fixed)
# '{"response": "He said \"hello\" to me"}'

# 直接获取 Python 对象
obj = repair_json(broken, return_object=True)
print(obj)
# {'response': 'He said "hello" to me'}
```

## 设计

基于**单次遍历状态机**，核心启发式规则：

> 字符串内遇到 `"` 时，仅当紧随其后的非空白字符是 `,` `}` `]` `:` `\n` 或另一个 `"` 才视为闭合引号，其余全部转义。

此规则针对 LLM 自然语言输出中频繁内嵌引号的行为优化。

## 性能

| 场景 | 大小 | 耗时 |
|------|------|------|
| 空对象 `{}` | 2 B | 2 µs |
| 小型 JSON | 68 B | 11 µs |
| 中型 JSON | 2.4 KB | 0.52 ms |
| 大型 JSON | 9.2 KB | 3.7 ms |
| 真实 LLM 输出 | 0.3 KB | 51 µs |
| 无效转义序列 | 0.1 KB | 18 µs |

损毁 JSON 的修复速度与合法 JSON 几乎相同，接近零额外开销。

## 版本

| 版本 | 说明 |
|------|------|
| v0.1.6 | 所有代码合并为单个 `_Repairer` 类；测试用例移至 22 个 `.jsonl` 文件；Pylance 严格模式 0 警告 |
| v0.1.5 | 前导逗号跳过、`.` 数字归一化、相邻对象 `}{` 检测包裹为数组 |
| v0.1.4 | 尾部垃圾检测、隐式数组深度追踪、16/17 json_failures.txt 可修复 |
| v0.1.3 | 隐含对象序列自动包裹为数组，大规模隐式数组压力测试 |
| v0.1.2 | JS 字面量支持、Hypothesis 属性测试、防御性修复 |
| v0.1.1 | 修复无效 JSON 转义序列 (`\*`, `\(`, `\)` 等) |
| v0.1.0 | 初始版本，单次遍历状态机修复 LLM JSON |

## 开发

```bash
# 克隆
git clone https://gitee.com/mensui/json_repair.git
cd json_repair

# 安装依赖
uv sync

# 运行测试
uv run pytest tests/ -v

# 运行 pre-commit
uv run pre-commit run --all-files
```

## 许可

GNU General Public License v2.0
