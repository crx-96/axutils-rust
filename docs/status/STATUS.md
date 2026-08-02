# 当前任务状态

## 目标

按 `docs/plans/config-file-loading.md` 实现统一的配置文件读取能力：支持 JSON、YAML、TOML、
INI 和 `.env` 五种格式，解析实现放在 `src/config/`，便捷入口 `ConfigUtils` 放在
`src/utils/config_utils.rs`。实现完成后需认真审查直到没有遗留问题。

## 范围

- 新增 `toml`、`serde-saphyr`、`rust-ini` 三个可选依赖与同名 feature；`config` 模块由 `serde`
  feature 守卫；`rust-version` 从 1.85 提升到 1.88（见下）。
- `src/config/{mod,error,format,value,de,source,json,yaml,toml,ini,env}.rs`、
  `src/utils/config_utils.rs`。
- `src/lib.rs`、`src/utils/mod.rs` 的 feature 守卫与重导出。
- `tests/config.rs`、`tests/fixtures/config/`、`tests/fixtures/config_feature_matrix/`、
  `tests/feature_matrix.rs` 的正/负向断言。
- `README.md`、`develop.md`、`docs/module-map.md`、`CHANGELOG.md`、`Cargo.toml`。

## 阶段（全部完成）

1. [完成] 核对基线：确认 `toml 1.1.4`、`serde-saphyr 1.0.0`、`rust-ini 0.21.3` 为当前最新兼容
   版本；实测发现并按用户确认修正了文档假设的 MSRV 风险（见下）。
2. [完成] Cargo feature、依赖与模块骨架。
3. [完成] `ConfigError`（脱敏 `Display`）、`ConfigFormat` 扩展名映射、`src/config/source.rs`
   受限读取（`Read::take` 流式截断、BOM、UTF-8 校验）。
4. [完成] `ConfigValue`、深度受限的 `DeserializeSeed`/`Visitor`（`ConfigValueSeed`）、
   `serde_json` 后端，`ConfigLoader`/`ConfigUtils` 完整方法集。
5. [完成] `.env` 自实现词法/插值解析器（`src/config/env.rs`）、`ConfigValue -> Deserializer`
   （`src/config/de.rs`，供 INI/`.env` 类型化读取复用）。
6. [完成] TOML（`toml` crate，含 `$__toml_private_datetime` 伪表识别）、YAML（`serde-saphyr`，
   `Budget.max_depth` 精确深度限制 + 别名预算 + 关闭 snippet）、INI（`rust-ini`，section 映射
   +重复键检测）后端。
7. [完成] API doc 与 `# Examples` doctest（全部 26 个公共方法逐一核对补齐）、`README.md`、
   `src/lib.rs` crate 文档、`docs/module-map.md`、`CHANGELOG.md`；第 10 节全部验证命令。
8. [完成] 最终审查：补充空文件/仅注释/YAML 非有限浮点等边界测试，确认无 `unwrap`/`expect`
   处理不可信输入，确认错误脱敏、feature 矩阵正负向断言、依赖树隔离均符合预期。
9. [完成] 复核当前工作区新增内容：修正 MSRV/feature 文档遗漏、消除 rustdoc 私有链接警告，显式固定
   YAML 别名回放上限，严格检查 JSON 尾随内容与 INI section 深度，并把各格式有类型路径的深度边界
   同步到 API doc、README 和工具类定位文档。

## 已确认问题（偏离文档假设，已获用户确认）

`serde-saphyr 1.0.0` 的 `Cargo.toml` 声明 `edition = "2024"` 并在源码中使用 let-chains 语法。
实测（`rustup` 安装 1.85.0 与 1.88.0-x86_64-pc-windows-msvc 工具链，在独立 scratch 项目中编译）
确认：

- Rust 1.85（项目原 MSRV）编译 `serde-saphyr 1.0.0` 报 27 个 `E0658` 错误（let-chains 与
  `is_multiple_of` 均未稳定）。
- Rust 1.88.0（let-chains 稳定化版本，2025-06-26 发布）及以上正常编译通过，并验证了
  `from_str_with_options`、`Options`/`Budget`/`AliasLimits`、`Error::DuplicateMappingKey`、
  `Error::location()` line/column 提取、`toml::de::Deserializer::parse`、
  `ini::Ini::load_from_str_opt`、`toml_datetime` 伪表协议的实际 API 行为。
- `toml 1.1.4`（自身 MSRV 1.85）与 `rust-ini 0.21.3`（自身 MSRV 1.64）在 1.85 下均正常。
- 用户已确认处理方式：**将项目 `rust-version` 从 1.85 提升到 1.88**，保留 `serde-saphyr`
  作为 YAML 后端；已同步更新 `Cargo.toml`、`README.md`、`src/lib.rs` crate 文档、
  `CHANGELOG.md` 的 `Changed` 条目。

## 风险与阻塞（任务结束时的状态）

- 本机默认工具链仍为 stable（未改动）；另安装了非默认的 `1.85.0-x86_64-pc-windows-msvc` 与
  `1.88.0-x86_64-pc-windows-msvc` 工具链用于 MSRV 验证（`1.94.0` 为会话开始前已存在，非本任务
  安装）。是否卸载 1.85.0（其验证目的已完成）由用户决定，本任务未擅自卸载。
- YAML 后端 `serde-saphyr` 1.0.0 于 2026-07-31 才发布，1.x 稳定性未经长期检验；已知限制（JSON
  超 `u64::MAX` 纯整数字面量退化为浮点、YAML 默认拒绝 `.inf`/`.nan`）已在 API doc/README 中
  如实记录。
- 已知功能边界（非缺陷，均按执行文档设计）：有类型读取路径对 JSON/TOML 的深度限制依赖各自
  后端内置的递归保护，而非本 crate 精确配置的 `max_depth`；YAML 由 `Budget.max_depth`、INI
  由 `ConfigValue` 构建阶段精确生效。TOML 重复键返回通用 `Parse` 而非 `DuplicateKey`（底层
  语法层拒绝，不经过本 crate 的检测逻辑）。

## 验证记录（全部通过）

```
cargo fmt --all -- --check                                            # 通过
cargo test --all-features                                             # 通过：142 lib + 8 集成(config.rs)
                                                                        #      + 2 集成(feature_matrix.rs)
                                                                        #      + 1 集成(email_live.rs) + 80 doctest
cargo test --doc --all-features                                       # 通过：80 doctest
cargo clippy --all-targets --all-features -- -D warnings              # 无警告
cargo doc --no-deps --all-features                                    # 通过，无 rustdoc 警告
cargo test --no-default-features                                      # 通过：26 lib + 13 doctest
cargo check --no-default-features [无/toml/serde/serde,toml/          # 全部符合预期（导出/不导出）
    serde,serde-saphyr/serde,rust-ini]
cargo tree --no-default-features --features serde|toml -e normal      # 依赖树按 feature 精确隔离
cargo package --allow-dirty --list                                    # 只含 src/**、Cargo.toml、
                                                                        #   README.md、CHANGELOG.md、
                                                                        #   LICENSE 及打包元文件；
                                                                        #   不含 tests/、docs/、
                                                                        #   develop.md、AGENTS.md、
                                                                        #   CLAUDE.md
```

`tests/feature_matrix.rs` 新增 `verifies_config_feature_api_matrix_and_dependency_boundaries`：
覆盖“无/仅后端 feature 不导出 API”“`serde` 单独启用只能用 JSON/`.env`”“单一后端 feature 下
`ConfigFormat::Yaml`/`Toml`/`Ini` 编译失败”等正负向断言，以及 `toml`/`serde-saphyr`/`rust-ini`
三个后端依赖树互不污染的断言，实测通过（既有 email 矩阵测试同时保持通过）。

## 相关路径

`docs/plans/config-file-loading.md`、`Cargo.toml`、`src/config/`、`src/utils/config_utils.rs`、
`src/lib.rs`、`src/utils/mod.rs`、`tests/config.rs`、`tests/fixtures/config/`、
`tests/fixtures/config_feature_matrix/`、`tests/feature_matrix.rs`、`README.md`、
`develop.md`、`docs/module-map.md`、`CHANGELOG.md`
