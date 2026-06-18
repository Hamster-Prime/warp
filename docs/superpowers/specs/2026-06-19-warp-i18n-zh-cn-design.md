# Warp 简体中文国际化（i18n）设计

- **作者**：基于用户需求 + brainstorming 会话
- **日期**：2026-06-19
- **状态**：设计稿，待实施
- **目标版本**：MVP

## 1. 目标与非目标

### 1.1 目标
- 把 Warp 终端 App 内所有用户可见 UI 文案（约 24,000 处英文硬编码字符串）本地化为简体中文。
- 在设置面板提供**语言开关**，包含三项：
  1. **跟随系统语言**（默认）
  2. **English**
  3. **简体中文**
- 切换语言**无需重启 App**，下一帧立即生效。
- 首次启动时根据系统语言自动选择（系统是中文则 UI 显示中文，否则英文）。
- 建立可持续的翻译维护工作流（AI 批量翻译 + diff 驱动）。

### 1.2 非目标（推迟到后续版本）
- 繁体中文 / 日韩法德等其他语言。
- 实时监听系统语言变化事件（MVP 仅启动时检测 + 手动切换）。
- community 翻译平台接入（Crowdin/Weblate）。
- AI prompt 模板的本地化（仅本地化用户可见 UI 文案）。
- Warp Drive / 服务端文案本地化。
- RTL（从右到左）布局支持。
- 完全去掉英文 fallback（英文永远作为兜底）。

## 2. 现状调研结论

Warp 项目当前**完全没有 i18n 基础设施**：
- 工作区 `Cargo.toml` 无任何 i18n crate 依赖（`fluent`/`rust-i18n`/`gettext`/`i18n-embed` 等均无）
- 无 `.ftl` / `.po` / `.strings` / `locales/` 等翻译资源
- 设置项 schema（`crates/settings/src/schema.rs:12-46`）无 language/locale 字段
- 唯一 locale 相关代码 `app/src/terminal/platform.rs:34` 仅用于给 shell 子进程设 POSIX `LANG`，与 UI 无关
- git 历史 1138 个 commit 中无任何 i18n 尝试
- UI 文本规模：约 24,445 处英文硬编码字符串，最密集区域：
  - `app/src/settings_view/` 约 1980 处（设置面板）
  - `app/src/ai/` 约 3887 处
  - `app/src/terminal/` 约 2978 处
- 所有 UI 文本类型为 `&'static str` / `Cow<'static, str>` / `String` / `SharedString`，无任何本地化封装
- 文本最终汇聚到 `crates/warpui_core/src/ui_components/text.rs:23` 的 `WrappableText::new()` 与 `:60` 的 `Text::new()`

## 3. 技术方案

### 3.1 选型决策

| 决策点 | 选择 | 理由 |
|---|---|---|
| i18n 框架 | **rust-i18n 3.x** | Rust 生态主流、编译期嵌入 YAML、`t!()` 宏简洁、调试容易、YAML 对 AI 翻译友好 |
| 字符串抽取 | **半自动 AST + 人工补齐** | 写 `syn` 工具改写 4 类高频写法（覆盖 ~70%），剩余人工 |
| 翻译 key 风格 | **英文原文作 key** | 调用点直观、`en.yml` 不需要单独维护（fallback 返回 key 本身） |
| 翻译来源 | **AI 批量翻译为主** | YAML 文件结构化、易 diff、可回滚 |
| 语言开关位置 | **Appearance 设置页** | 复用现有 section、与 theme/font 同属"显示外观"；未来可拆 General |
| 跨平台检测 | **sys-locale crate** | 跨平台、社区维护 |

### 3.2 新增依赖（根 `Cargo.toml` `[workspace.dependencies]`）

```toml
rust-i18n = "3"
sys-locale = "0.3"
```

### 3.3 新增 crate：`crates/i18n`

作为整个 i18n 系统的**单一入口**，避免其他 crate 各自引入翻译逻辑：

```
crates/i18n/
├── Cargo.toml
├── _locales/
│   └── zh-CN/
│       ├── _shared.yml       # 通用按钮、状态、品牌名
│       ├── settings.yml      # 设置面板
│       ├── appearance.yml    # 外观
│       ├── ai.yml            # AI 对话 UI 文案（不含 prompt 模板）
│       ├── terminal.yml      # 终端相关 UI
│       ├── workspace.yml     # 顶部菜单、命令面板、tab
│       ├── editor.yml        # 编辑器 UI
│       ├── onboarding.yml    # 启动引导
│       ├── errors.yml        # user-facing 错误消息
│       └── glossary.yml      # 术语表（约束翻译）
├── src/
│   ├── lib.rs                # rust_i18n::i18n!("../_locales") 宏注册 + re-export t!
│   ├── language.rs           # AppLanguage 枚举（System/English/SimplifiedChinese）
│   ├── detection.rs          # sys-locale 包装 + BCP-47 归一化
│   └── init.rs               # apply(language) → rust_i18n::set_locale
└── tests/
```

`Cargo.toml`：
```toml
[package]
name = "i18n"
version = "0.1.0"
edition = "2021"

[dependencies]
rust-i18n = { workspace = true }
sys-locale = { workspace = true }
serde = { workspace = true }
schemars = { workspace = true }
settings = { path = "../settings" }   # 复用 SettingsValue 派生宏

[lib]
path = "src/lib.rs"
```

`src/lib.rs` 核心：
```rust
pub use rust_i18n::t;

rust_i18n::i18n!("../_locales", fallback = "en");

pub mod detection;
pub mod init;
pub mod language;

pub use language::AppLanguage;
```

## 4. 字符串抽取策略

### 4.1 新增 cargo bin：`i18n-extract`

路径：`crates/i18n_tools/`（workspace 子项目，bin 型 crate）

**4 类可机械改写的写法**：

| # | 原始 | 改写后 |
|---|---|---|
| 1 | `ui_builder().label("X".to_string())` | `ui_builder().label(t!("X").to_string())` |
| 2 | `.with_text_label("X".to_string())` | `.with_text_label(t!("X").to_string())` |
| 3 | `Text::new("X")` / `Span::new("X")` / `Paragraph::new("X")` | `Text::new(t!("X"))` |
| 4 | 枚举 match 的 `&'static str`（如 `EnforceMinimumContrast::Always => "Always"`） | 由 `as_dropdown_label()` 返回 `t!("Always")` |

预估覆盖率：~70%（约 17k 串），剩余 30%（约 7k）人工处理。

### 4.2 排除清单（`.i18n-ignore.toml`）

工具维护路径黑名单 + 模式黑名单：
- 路径：`**/telemetry/**`、`**/events.rs`、`**/tests/**`、`**/benches/**`、`target/`
- 宏调用：`log::info!` / `debug!` / `warn!` / `error!` / `trace!`
- 函数：`FromStr::from_str` / serde `Deserialize` impl
- 字符串特征：命令名、keybinding ID（如 `"cmd+k"`）、文件路径、URL、shell 输出
- 测试代码：`#[cfg(test)]` 块

### 4.3 人工处理的部分

- `format!("Loading {}", name)` → `t!("Loading {}", name)`（rust-i18n 支持 `{0}` 占位符）
- user-facing 错误消息：如 `SettingsFileError`（`app/src/settings/mod.rs:72-118`）
- `Display` impl 的用户可见文案
- AI prompt 模板：**MVP 不本地化**，单独决策

### 4.4 调用点写法约定

- 普通：`t!("Font weight")`
- 带占位符：`t!("Loading {}", name)` 或 `t!("Loading {name}", name = name)`
- 复用同 key 不同上下文（罕见冲突）：`t!("Save@actions")`，YAML 中 `"Save@actions": 保存`

## 5. 翻译资源组织与 AI 工作流

### 5.1 YAML 文件拆分

24k 串单文件不可维护。按 `app/src/` 子目录切分为多文件（rust-i18n 3.x 递归合并）：

```yaml
# _locales/zh-CN/settings.yml 示例
"Settings": 设置
"Appearance": 外观
"Features": 功能
"Keyboard shortcuts": 快捷键
"Privacy": 隐私

# 跨文件复用的 key 由 _shared.yml 提供
```

每个 key 上方可加注释 `# src: app/src/settings_view/appearance_page.rs:4100` 便于回溯（注释不影响 rust-i18n 解析）。

### 5.2 术语表（`_locales/zh-CN/glossary.yml`）

```yaml
# 强制约束翻译（CI 校验）
"Warp": Warp                          # 品牌名不译
"Warp Drive": Warp Drive              # 专有功能
"Agent": Agent                        # Warp Agent
"Block": Block                        # 命令块术语
"Command Palette": 命令面板
"Workflow": 工作流
"Notebook": 笔记本
"MCP": MCP
"Oz Cloud": Oz Cloud
"Theme": 主题
"Settings": 设置
```

### 5.3 新增 cargo bin：`i18n-translate`

路径：`crates/i18n_tools/`（与 `i18n-extract` 同 crate，不同 bin）

工作流：
1. 读取代码中所有 `t!()` 的英文 key
2. 与 `zh-CN/*.yml` 比对，找出缺失/变化的 key
3. 按 200 串/批切分，调用翻译 API（OpenAI / Claude / DeepSeek，可配置）
4. prompt 包含：Warp 上下文 + glossary few-shot + 文件来源
5. 输出更新到对应 `zh-CN/*.yml`
6. 打印术语频次报告，提示疑似不一致（如 `Agent` 在不同处被译为「代理/Agent」）

**diff 模式**：`cargo run -p i18n-translate -- --diff` 只翻译新增/变化的 key。

**orphan 清理**：`cargo run -p i18n-translate -- --prune` 删除 zh-CN.yml 中存在但代码无调用的 key（30 天宽限期，先标 `# orphaned` 注释）。

### 5.4 持续维护

- 开发者改英文文案 → CI 的 `check-i18n` 失败，提示「key 'X' 在 zh-CN 中缺失」
- 跑 `i18n-translate --diff` 自动补翻译
- 每周人工抽审（重点：设置面板、AI UI、错误消息）
- `_locales/zh-CN/CHANGELOG.md` 记录每次批量翻译

## 6. 语言开关与运行时切换

### 6.1 数据层：`LanguageSettings`

新增 `app/src/settings/language.rs`，仿照 `app/src/settings/font.rs:15-113`：

```rust
use i18n::AppLanguage;

define_settings_group!(LanguageSettings, settings: [
    language: LanguageSetting {
        type: AppLanguage,
        default: AppLanguage::System,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Always,
        private: false,
        storage_key: "Language",
        toml_path: "appearance.language",
        description: "The display language of the Warp UI.",
    },
]);
```

注册：在 `app/src/settings/init.rs:61`（`FontSettings::register(ctx);` 之后）加 `LanguageSettings::register(ctx);`

### 6.2 枚举：`AppLanguage`

位于 `crates/i18n/src/language.rs`：

```rust
use settings::SettingsValue;
use serde::{Serialize, Deserialize};
use schemars::JsonSchema;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq,
    Serialize, Deserialize, JsonSchema, SettingsValue, Default,
)]
pub enum AppLanguage {
    #[default]
    System,
    English,
    SimplifiedChinese,
}

impl AppLanguage {
    pub fn all() -> &'static [AppLanguage] {
        &[Self::System, Self::English, Self::SimplifiedChinese]
    }

    /// 下拉项显示文案。非英文选项用目标语言显示，方便用户识别。
    pub fn dropdown_label(&self) -> &'static str {
        match self {
            Self::System           => "System",
            Self::English          => "English",
            Self::SimplifiedChinese => "简体中文",
        }
    }

    /// 解析为 BCP-47 语言代码。
    pub fn resolve(self) -> &'static str {
        match self {
            Self::System           => crate::detection::detect_system_language(),
            Self::English          => "en",
            Self::SimplifiedChinese => "zh-CN",
        }
    }
}
```

> 放在 `crates/i18n` 而非 `app/src/settings`，避免 `app` ↔ `i18n` 循环依赖。

### 6.3 UI 层：Appearance 设置页

改动 `app/src/settings_view/appearance_page.rs`，1:1 仿照 `monospace_font_weight` 端到端链路：

- 字段声明（参照 `:561`）：`language_dropdown: ViewHandle<Dropdown<AppearancePageAction>>,`
- Action（参照 `:488`）：`SetLanguage(AppLanguage),`
- dispatch（参照 `:603`）：`SetLanguage(value) => self.set_language(*value, ctx),`
- 构造（参照 `:1095-1113`）：`Dropdown::new` + `add_items` + `set_selected_by_name`
- 渲染（参照 `:4100-4121`）：标签 + 下拉
- 处理函数（参照 `:1815-1819`）：
  ```rust
  pub fn set_language(&mut self, value: AppLanguage, ctx: &mut ViewContext<Self>) {
      LanguageSettings::handle(ctx).update(ctx, |settings, ctx| {
          report_if_error!(settings.language.set_value(value, ctx));
      });
      rust_i18n::set_locale(value.resolve());
      ctx.invalidate_all_views();
  }
  ```

### 6.4 跟随系统语言检测（`crates/i18n/src/detection.rs`）

```rust
use std::sync::OnceLock;

static DETECTED: OnceLock<&'static str> = OnceLock::new();

pub fn detect_system_language() -> &'static str {
    *DETECTED.get_or_init(|| {
        let raw = sys_locale::get_locale().unwrap_or_else(|| "en".to_string());
        normalize(&raw).unwrap_or("en")
    })
}

fn normalize(raw: &str) -> Option<&'static str> {
    let lower = raw.to_lowercase();
    if lower.starts_with("zh") {
        // POSIX: zh_CN.UTF-8 / zh_CN / macOS: zh-hans-cn / Windows: zh-CN
        if lower.contains("hant") || lower.ends_with("-tw") || lower.ends_with("-hk") || lower.ends_with("-mo") {
            Some("zh-CN")  // MVP 繁中回退到简中资源
        } else {
            Some("zh-CN")
        }
    } else {
        Some("en")  // 其他系统语言一律 fallback 英文
    }
}
```

OnceLock 缓存避免每次 `t!()` 都检测系统语言；只在 `AppLanguage::System` 模式下首次访问时检测一次。

### 6.5 启动时初始化

在 `app` 启动序列、`register_all_settings`（`app/src/settings/init.rs:55`）之后：

```rust
let lang = LanguageSettings::handle(ctx).read().language;
i18n::init::apply(lang);  // → rust_i18n::set_locale(lang.resolve())
```

### 6.6 持久化

`~/.config/warp/settings.toml`：
```toml
[appearance]
language = "SimplifiedChinese"   # 或 "System" / "English"
```

老用户无此字段时，宏 fallback 到 `AppLanguage::System`（默认），无破坏。

### 6.7 切换表现

- `set_locale()` + `invalidate_all_views()` → 下一帧全部 `t!()` 重新查表 → 全界面立即变中文
- 已打开的对话框、命令面板、AI 对话气泡都会刷新
- **无需重启 App**

## 7. 测试策略

### 7.1 单元测试（`crates/i18n/tests/`）

- `detection.rs`：mock 各种输入（`zh_CN.UTF-8` / `zh-Hans-CN` / `zh-CN` / `en-US` / 空），验证 BCP-47 归一化
- `language.rs`：`AppLanguage::resolve()` 三个变体返回值；`dropdown_label()` 输出
- `yml_load.rs`：rust-i18n 加载后所有 key 都能查到（防 YAML 语法错误）；`{}` 占位符数量在 en/zh 之间一致

### 7.2 调用点测试

- 抽取后断言：所有 `t!()` 的 key ⊆ `zh-CN.yml` 的 key 集合
- 单点：`assert_eq!(t!("Font weight"), "字体粗细")` 在 `set_locale("zh-CN")` 之后

### 7.3 集成测试（仿照 `crates/integration/`）

- 启动 App → 打开 Settings → 切换 Language 下拉到「简体中文」→ 截图断言 appearance 页标签全部为中文（OCR 或预定义锚点文本「字体粗细」）
- 切换回 English → 断言回到英文
- 重启 App，验证 settings.toml 持久化生效

### 7.4 CI 校验（`cargo xtask check-i18n` + `.github/workflows/i18n.yml`）

1. **key 完整性**：所有 `t!()` 调用点 key 必须在 zh-CN.yml 中存在
2. **glossary 一致性**：违反术语表的 key 列为 warn（不阻塞）
3. **orphan 检测**：zh-CN.yml 中存在但代码无调用的 key（30 天宽限期）
4. **YAML 语法校验**
5. **占位符数量校验**：`{}` 在 en/zh 中数量一致

## 8. 工程里程碑

| 里程碑 | 范围 | 周期 |
|---|---|---|
| **M1：i18n 基础设施** | `crates/i18n` crate + rust-i18n 集成 + `LanguageSettings` 数据层 + Appearance 页下拉 + 系统检测 + 启动时 apply + 单元测试 + `zh-CN.yml` 占位（10 个示例 key） | 1 周 |
| **M2：抽取工具链** | `i18n-extract` cargo bin（4 类 AST 模式）+ `.i18n-ignore.toml` + `check-i18n` xtask + CI | 1.5 周 |
| **M3：AI 翻译管线** | `i18n-translate` cargo bin（glossary + 批量 + 频次报告） | 1 周 |
| **M4：批量抽取 + 翻译** | 跑抽取 + AI 翻译 zh-CN/*.yml + 人工补齐 30% | 2-3 周 |
| **M5：测试 + 打磨** | 集成测试 + 术语校对 + 用户体验验证 | 0.5-1 周 |

**总周期**：约 6 周。

## 9. 风险与缓解

| 风险 | 概率 | 缓解 |
|---|---|---|
| 24k 调用点改写遗漏 | 中 | CI 的 `check-i18n` 强制每条 `t!()` 有翻译；定期增量扫描 |
| 同一英文 key 上下文歧义 | 低 | YAML 用 `"Save@actions"` 限定；抽取工具识别 call-site 上下文 |
| AI 翻译术语不一致 | 中 | glossary.yml 强制 + 频次报告 + 周度人工抽审 |
| 占位符 `{}` 被翻译破坏 | 中 | 抽取工具保留；CI 校验 `{}` 数量一致 |
| 性能回退（HashMap 查表） | 低 | `t!()` 是 `OnceCell` 一次查表，O(1)；CI 加 perf benchmark |
| rust-i18n 升级 breaking change | 低 | pin 主版本；i18n crate 是唯一依赖点 |
| AI prompt 模板本地化影响 AI 行为 | 中 | MVP 不本地化 prompt 模板 |
| 系统语言检测在特殊环境不准 | 低 | 独立开关作为兜底 |

## 10. 关键文件速查表

| 主题 | 文件 | 行号 |
|---|---|---|
| Settings tab 枚举 | `app/src/settings_view/mod.rs` | 240-278 |
| 字体设置 group（最佳模板） | `app/src/settings/font.rs` | 15-113 |
| 字体下拉 UI（端到端模板） | `app/src/settings_view/appearance_page.rs` | 561, 1095, 1815, 4100 |
| 设置注册入口 | `app/src/settings/init.rs` | 55-108 |
| 持久化文件路径 | `app/src/settings/mod.rs` | 597 (`settings.toml`) |
| 设置定义宏 | `crates/settings/src/macros.rs` | 203, 519, 704 |
| Dropdown 组件 | `app/src/view_components/dropdown.rs` | 101, 157 |
| 文本渲染底层 | `crates/warpui_core/src/ui_components/text.rs` | 23, 60 |
| `ui_builder()` 定义 | `crates/warp_core/src/ui/appearance.rs` | 272 |
| 枚举设置模板（theme） | `app/src/settings/theme.rs` | 14-48 |
| schema 生成器 | `app/src/bin/generate_settings_schema.rs` | 5 |

## 11. 验收标准

- [ ] `LanguageSettings` 在 `settings.toml` 正确读写，跨设备同步生效
- [ ] Appearance 页有"Language"下拉，含三项："System" / "English" / "简体中文"
- [ ] 切换"简体中文"后无需重启，下一帧 UI 全部变中文
- [ ] 切换"English"后回到英文
- [ ] 系统语言为中文时，首次启动默认显示中文（setting = System）
- [ ] `t!()` 覆盖全部 24k 字符串调用点（CI 强制）
- [ ] `zh-CN/*.yml` 翻译完整，glossary 一致
- [ ] 集成测试通过（截图锚点为中文）
- [ ] CI 的 `check-i18n` 全绿
- [ ] 现有用户的 `settings.toml` 兼容（无破坏性变更）
