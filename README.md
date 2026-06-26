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
| 无大括号对象检测 (v0.1.6) | `"key": value` | `{"key": value}` |
| 多余逗号跳过 (v0.1.7) | `"x",,` / `[1,,2]` | `"x",` / `[1,2]` |
| 括号错序修复 (v0.1.8) | `[{"]"}]}` → `[{"..."}]` | 数组末对象 `]` 误放于 `}` 前时自动校正 |
| 数组 `}` 闭合校正 (v0.1.9) | `{"a":[1}}]}` → `{"a":[1]}` | 数组误用 `}` 关闭时自动替换为 `]` |
| 无引号值修复 (v0.1.9) | `{"name": John}` → `{"name": "John"}` | 无引号字符串值自动添加引号 |
| 混合引号边界修复 (v0.1.10) | `"文本','key":"值"` → `"文本","key":"值"` | 双引号内 `','key":"` 模式自动断开，单引号 key 不泄漏到文本值中 |
| 冒号后缺失值填充 (v0.1.10) | `{"text":` → `{"text": null}` | Key 后缺失的值自动填充 `null` |

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

## 注意

修复后的 JSON 在语法上一定合法，但不保证语义上满足你的需求（例如缺失值被补 `null`）。
**建议配合验证器使用**，将字符串解析为 `dict` 后检查其结构是否符合预期。

```python
from json_repair import repair_json

raw = '{"name": "Alice", "age":'
obj = repair_json(raw, return_object=True)
# obj == {"name": "Alice", "age": null}  ← 可能不是你想要的

# 自定义验证
def validate(data):
    return isinstance(data, dict) and "age" in data and data["age"] is not None

if validate(obj):
    print("OK:", obj)
else:
    print("unexpected shape, discard or retry")
```

## 设计

基于**单次遍历状态机**，核心启发式规则：

> 字符串内遇到 `"` 时，仅当紧随其后的非空白字符是 `,` `}` `]` `:` `\n` 或另一个 `"` 才视为闭合引号，其余全部转义。

此规则针对 LLM 自然语言输出中频繁内嵌引号的行为优化。

## 性能

| 场景 | 大小 | 耗时 |
|------|------|------|
| 空对象 `{}` | 2 B | 3 µs |
| 小型 JSON | 48 B | 20 µs |
| 中型 JSON | 2.4 KB | 0.74 ms |
| 大型 JSON | 9.2 KB | 4.6 ms |
| 真实 LLM 输出 | 0.3 KB | 64 µs |
| 无引号值修复 | 14 B | 6 µs |
| 括号错序 / `}` 闭包 | 0.2-0.5 KB | 76–143 µs |

损毁 JSON 的修复速度与合法 JSON 几乎相同，接近零额外开销。

## 版本

| 版本 | 说明 |
|------|------|
| v0.1.10 | 混合引号边界修复（`','word":"` 自动断开）；冒号后缺失值填充（`{"text":` → `{"text":null}`）；新增 `mixed_quotes.jsonl`；8/8 `json_failures.txt` 全修复 |
| v0.1.9 | 数组 `}` 闭合校正（`{"a":[1}}]}` → `{"a":[1]}`）；无引号字符串值修复（`{"name": John}` → `{"name": "John"}`）；测试拆分为独立文件；新增 `brace_as_array_close.jsonl`、`unquoted_values.jsonl` |
| v0.1.7 | 多余逗号跳过（`",,"` → `","`）；更新到 24 个 `.jsonl` 文件；34/34 `json_failures.txt` 全修复 |
| v0.1.6 | 单 `_Repairer` 类；无大括号对象检测；22 个 `.jsonl` 测试文件；Pylance 严格模式 0 警告 |
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
