# axutils 工具类定位

本文档维护 `axutils` 中工具类和公共模块的职责边界，帮助贡献者在新增能力前找到合适的
归属位置，避免重复实现或职责交叉。涉及工具类、跨模块 API 或新增方法时，应先阅读本文档。

## 定位清单

| 工具类 | 源文件 | crate 根模块导出 | 可用条件与依赖 | 职责与主要使用场景 |
| --- | --- | --- | --- | --- |
| `PathUtils` | `src/utils/path_utils.rs` | `axutils::PathUtils`；模块为 `axutils::path_utils` | 默认可用；仅依赖 Rust 标准库 | 判断路径是否为绝对路径，获取当前工作目录和当前可执行文件路径，按平台规则拼接并词法规整多个路径；不负责文件系统存在性、权限、符号链接或真实路径解析 |
| `TimeUtils` | `src/utils/time_utils.rs` | `axutils::TimeUtils`；模块为 `axutils::time_utils` | 默认可用；仅依赖 Rust 标准库 | 获取当前 Unix 时间戳，支持秒、毫秒、微秒和纳秒；不负责日期格式化、时区转换或日历计算 |
| `FormatUtils` | `src/utils/format_utils.rs` | `axutils::FormatUtils`；模块为 `axutils::format_utils` | 默认可用；仅依赖 Rust 标准库 | 将秒数格式化为中文持续时间字符串（天/小时/分钟/秒，最大单位为天，不足一天不显示天）；不负责时区转换、周/月/年等更大单位、负数或小数秒，也不支持中文以外的语言 |
| `RegUtils` | `src/utils/reg_utils.rs` | 启用 `regex` feature 后提供 `axutils::RegUtils`；模块为 `axutils::reg_utils` | `regex` feature 提供模块、常见/严格邮箱和中国大陆手机号校验；可选的第三方 `regex` crate。`is_phone` 还要求独立的 `libphonenumber` feature，并通过依赖别名 `libphonenumber` 使用 crates.io 的 `phonenumber` crate | 校验常见和严格电子邮箱格式、中国大陆手机号码格式，以及启用两个 feature 后的国际 E.164 手机号码格式；只做本地格式、号段和号码类型校验，不验证地址或号码是否真实存在 |
| `RandomUtils` | `src/utils/random_utils.rs` | 启用 `rand` feature 后提供 `axutils::RandomUtils`、`axutils::LetterCase` 和 `axutils::RandomRangeError`；模块为 `axutils::random_utils` | 默认不可用；`rand` feature 提供能力，可选的第三方 `rand` crate | 生成数字、大小写字母、混合字母和数字字母 ASCII 字符串，从闭区间生成 `i64` 或可构造的有限 `f64` 随机数；字符串长度不设固定上限，调用方需限制不可信输入；不负责密码学安全随机数、密码、令牌、密钥或可复现随机序列 |

## 新增工具类时的定位要求

新增工具类或公共模块时，应在同一变更中补充以下信息：

1. 源文件路径和 crate 根模块的公共导出路径；
2. 是否默认可用、对应 feature 以及第三方依赖；
3. 单一且清晰的职责边界、主要使用场景和明确不负责的范围；
4. 与现有工具类的关系，尤其是可能重叠的 API 和复用方式。

如果工具类的职责、公共导出、feature、依赖或适用范围发生变化，也必须同步更新本清单。
