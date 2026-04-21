# 基于 `wp-motor` 与 `warp-parse` 的大型工程错误设计指南

本文档基于对 `wp-motor` 与 `warp-parse` 两个工程的静态代码分析，总结一套适用于大型 Rust 工程的错误设计方法。

目标不是解释某个 crate 的 API，而是回答四个更关键的问题：

- 大型工程里，错误应该如何分层
- 错误对象里应该放什么，不该放什么
- 错误如何在 crate / 模块 / CLI / HTTP 边界上传递
- 如何让错误既可被程序处理，又可被用户排障

## 1. 适用范围

这套设计适合以下场景：

- 多 crate 工程
- 有 CLI / API / 后台任务 / 流式运行时
- 有配置加载、文件系统、网络、远程更新、回滚恢复
- 需要把错误用于重试、容错、监控、运维排障

不适合的场景：

- 一次性脚本
- 非长期维护的小工具
- 没有跨层边界、没有诊断需求的极小项目

## 2. 从两个工程里看到的核心结论

`wp-motor` 的成熟点主要在：

- 统一的顶层错误模型
- 结构化上下文与元数据
- 独立的诊断渲染层
- 运行时错误处理策略分离

`warp-parse` 的成熟点主要在：

- CLI 入口统一收口
- 对远端同步、锁、快照、回滚做成完整失败协议
- 在 HTTP / 文件 / Git 边界做错误包装

两个工程共同说明了一点：

> 大型工程里的错误设计，不是“返回 `Result`”这么简单，而是“定义一条从底层失败到顶层诊断的稳定协议”。

## 2.1 `orion-error` 是什么

如果只看 `wp-motor` / `warp-parse` 的业务代码，很容易看到大量：

- `StructError<_>`
- `UvsReason`
- `OperationContext`
- `ErrorOwe` / `ErrorOweSource`
- `ErrorConv`
- `ErrorWith`
- `with_source(...)`
- `want(...)`

这些能力并不是工程内部零散发明的，而是由 `orion-error` 这个 crate 统一提供。

它的定位不是“替代 `thiserror` 再定义一套错误枚举”，而是：

> 在 `thiserror` 负责“定义错误类型”之外，补齐大型工程真正需要的错误治理能力。

可以把三者的职责简单理解为：

- `thiserror`：定义错误枚举，生成 `Display` / `Error`
- `anyhow`：快速聚合和传播错误，适合小边界和一次性逻辑
- `orion-error`：给大型工程提供分类、上下文、转换、source chain、错误码、诊断素材

### 2.1.1 它解决的不是“有没有错误”，而是“错误能不能治理”

一个大型工程真正缺的通常不是 `Result<T, E>`，而是下面这些能力：

- 同一工程里有没有统一错误分类
- 不同 crate 之间能不能稳定转换错误
- 错误能不能带操作上下文、路径和阶段
- 顶层 CLI / API 能不能提取出根因链和定位信息
- 错误能不能挂上稳定错误码
- 运行时能不能基于错误语义决定 retry / ignore / throw

`orion-error` 正是在补这些空白。

### 2.1.2 它提供的核心能力

#### 1. 统一分类：`UvsReason`

`UvsReason` 提供跨工程统一的错误分类，例如：

- `ValidationError`
- `BusinessError`
- `NotFoundError`
- `PermissionError`
- `DataError`
- `SystemError`
- `NetworkError`
- `ResourceError`
- `TimeoutError`
- `ConfigError`
- `ExternalError`

这层价值是：

- 让不同模块先共享一套“大类语义”
- 让领域错误枚举可以通过 `From<UvsReason>` 快速接入统一分类
- 让运行时、监控、CLI exit code 先有稳定基础

示例：

```rust
use thiserror::Error;
use orion_error::{ErrorCode, UvsReason};

#[derive(Debug, Error, Clone, PartialEq)]
enum AppReason {
    #[error("invalid request")]
    InvalidRequest,
    #[error("{0}")]
    Uvs(UvsReason),
}

impl From<UvsReason> for AppReason {
    fn from(value: UvsReason) -> Self {
        Self::Uvs(value)
    }
}

impl ErrorCode for AppReason {
    fn error_code(&self) -> i32 {
        match self {
            Self::InvalidRequest => 1000,
            Self::Uvs(reason) => reason.error_code(),
        }
    }
}
```

#### 2. 结构化错误壳：`StructError<R>`

`StructError<R>` 是 `orion-error` 的核心对象。

它不是简单包装一个字符串，而是承载：

- `reason`
- `detail`
- `position`
- `context`
- `source`
- `source_frames`

这层价值是：

- 错误对象可持续追加信息
- 展示层不必反向解析随意拼出的字符串
- 错误链可以结构化保留，而不是只保留一段 `Display`

示例：

```rust
use orion_error::{StructError, UvsReason};

let err = StructError::from(UvsReason::system_error())
    .with_detail("read config failed");
```

带 source 的例子：

```rust
use orion_error::{StructError, UvsReason};

let err = StructError::from(UvsReason::system_error())
    .with_detail("read config failed")
    .with_source(std::io::Error::other("disk offline"));
```

#### 3. 调用现场传播：`OperationContext`

`OperationContext` 用来记录“当前正在做什么”。

典型承载信息：

- 外层目标 `want`
- 内层 path
- 业务字段记录 `record(key, value)`
- 自动日志上下文

示例：

```rust
use orion_error::{ContextRecord, OperationContext};

let mut ctx = OperationContext::want("load_config");
ctx.record("path", "conf/wparse.toml");
ctx.record("component", "engine");
```

再把它挂到错误上：

```rust
use orion_error::{ErrorWith, ErrorOweSource};

let result = std::fs::read_to_string("conf/wparse.toml")
    .owe_sys_source()
    .want("read config file")
    .with(&ctx);
```

这层价值是：

- 上下文不再散落在日志和 `format!` 里
- 错误链里的“操作路径”可以稳定提取
- CLI / observability 层可以读到结构化上下文

#### 4. 非结构错误转结构错误：`ErrorOwe` / `ErrorOweSource`

这组 trait 解决的不是“怎么手写 `map_err`”，而是“怎么把标准库或第三方错误稳定映射到统一分类”。

推荐：

```rust
use orion_error::ErrorOweSource;

let body = std::fs::read_to_string(path).owe_conf_source()?;
```

如果只用 `Display` 错误，也有兼容路径：

```rust
use orion_error::ErrorOwe;

let value = parse_text().owe_validation()?;
```

这层价值是：

- 减少样板转换代码
- 强制把上游错误纳入统一分类
- `owe_*_source()` 默认保留真实 source

#### 5. 结构错误跨层转换：`ErrorConv` / `ErrorWrap`

这是大型工程特别需要但很多库没有补齐的一层。

场景一：

- 下层已经返回 `StructError<R1>`
- 上层只想把 `R1` 转成 `R2`

用 `err_conv()`：

```rust
use orion_error::ErrorConv;

let project = load_repo().err_conv()?;
```

场景二：

- 上层要重新定义一个新的 reason 边界
- 但又不想丢掉下层结构错误

用 `err_wrap(...)`：

```rust
use orion_error::ErrorWrap;

let project = load_repo().err_wrap(UvsReason::system_error())?;
```

这层价值是：

- 跨 crate 传播时不需要到处 `map_err`
- 可以重定 reason，又不丢掉原结构错误链

#### 6. 结构化 source chain：`with_source(...)` / `source_frames()`

`orion-error` 不只是实现了 `Error::source()`，它还会把 source chain 整理成结构化 frame。

这意味着上层可以直接拿到：

- 根因
- source chain
- 每层的 message / detail / path / reason / error_code

而不是只能打印一长串文本。

这也是为什么 `wp-motor` 的诊断层能从错误对象里提取：

- `location`
- `parse excerpt`
- `root cause`
- `hint`

### 2.1.3 它和 `thiserror`、`anyhow` 的关系

#### 和 `thiserror`

两者不是替代关系，而是分工关系。

推荐模式是：

- 用 `thiserror` 定义领域错误枚举
- 用 `orion-error` 提供统一分类、上下文传播、结构化包装和转换

也就是说：

- `thiserror` 解决“怎么定义一个好用的枚举”
- `orion-error` 解决“这个枚举如何进入大型工程的错误体系”

#### 和 `anyhow`

`anyhow` 的强项是快速聚合和快速返回。

但它默认不提供：

- 统一大类分类
- 错误码
- 上下文对象栈
- 结构化 source frame
- 领域错误跨层转换约束

所以它适合：

- 测试
- 一次性工具
- 很薄的入口

但不适合作为 `wp-motor` / `warp-parse` 这类系统的主错误模型。

### 2.1.4 为什么这两个工程围绕它组织错误设计

因为这两个工程同时具有以下特征：

- crate 多
- 配置加载链长
- 有 Source / Sink / Runtime / DSL / CLI / Admin API 多种边界
- 需要错误码、上下文、诊断、回滚、运维排障

如果没有 `orion-error` 这一层，工程很快会退化成：

- 一部分 `thiserror`
- 一部分 `anyhow`
- 一部分 `Result<T, String>`
- 一部分手写 `map_err(|e| format!(...))`

最终结果就是：

- 错误分类不稳定
- source chain 丢失
- 顶层只能靠字符串猜根因
- CLI 很难统一输出
- 运行时很难按语义做处置

因此，这两个工程真正复用的不是某个单独 API，而是一整套以 `orion-error` 为核心的错误治理模式：

- `UvsReason` 统一分类
- `StructError` 统一壳
- `OperationContext` 统一上下文
- `ErrorOwe*` 统一底层提升
- `ErrorConv` / `ErrorWrap` 统一跨层转换
- `source_frames` 支撑顶层诊断

## 2.2 `orion-error` 在 `wp-motor` / `warp-parse` 中的真实使用路径图

这一节不讲抽象接口，而是把仓库里的真实链路串起来。

先给结论：

> 在这两个工程里，`orion-error` 的典型链路不是“某一个函数调用”，而是一条跨层协议：
>
> `普通错误` -> `owe_*_source()` -> `with(...)` -> `want(...)` -> `err_conv()/wrap(...)` -> `RunError` -> `print_run_error()`

它可以拆成 5 个阶段。

### 2.2.1 阶段一：普通错误进入结构化错误体系

最底层通常先从标准库或第三方错误开始，例如 `std::io::Error`。

`orion-error` 用 `ErrorOweSource` 把这类错误提升为 `StructError<R>`。源码里这一层的关键语义很直接：

- `owe_*_source()` 根据 `UvsFrom` 选择 reason 大类
- `with_detail(e.to_string())`
- `with_source(e)` 保留真实底层错误

对应实现可见：

- [owenance.rs:36](/Users/zuowenjian/devspace/wp-labs/dev/wparse/orion-error-pr/src/traits/owenance.rs#L36)
- [owenance.rs:105](/Users/zuowenjian/devspace/wp-labs/dev/wparse/orion-error-pr/src/traits/owenance.rs#L105)

最小形态可以写成：

```rust
let body = std::fs::read_to_string(path).owe_conf_source()?;
```

这一步做完以后，错误已经不再只是 `std::io::Error`，而是：

- 有分类
- 有 detail
- 有 source

### 2.2.2 阶段二：给错误补调用现场

仅有 reason 还不够，接下来要补“当前层正在做什么”。

这一步由：

- `with(...)`
- `want(...)`
- `OperationContext`

共同完成。

仓库里一个最典型的通用封装是 [error_handler.rs:137](/Users/zuowenjian/devspace/wp-labs/dev/wparse/wp-motor/crates/wp-proj/src/utils/error_handler.rs#L137)：

```rust
pub fn safe_file_operation<T>(
    operation: &str,
    path: &Path,
    op: impl FnOnce() -> Result<T, std::io::Error>,
) -> RunResult<T> {
    op().owe_conf_source().with(path).want(operation)
}
```

这一行非常有代表性，含义是：

1. `owe_conf_source()`
   把 `std::io::Error` 提升成结构化配置错误，并保留真实 source。
2. `with(path)`
   给错误挂上当前对象，这里是文件路径。
3. `want(operation)`
   给错误挂上当前操作名，比如 `read file`、`write file`。

结果就是，同一个底层 `io::Error` 现在变成了：

- reason: 配置/系统大类
- detail: 原始 `io::Error` 文本
- path: 当前文件
- want/path stack: 当前操作链
- source: 原始错误对象

### 2.2.3 阶段三：复杂场景里补更丰富的上下文

如果只是 `with(path)` 还不够，就会上 `OperationContext`。

`wp-motor` 里一个非常清楚的例子是 [wparse/mod.rs:47](/Users/zuowenjian/devspace/wp-labs/dev/wparse/wp-motor/crates/wp-proj/src/wparse/mod.rs#L47)：

```rust
std::fs::remove_dir_all(&run_dir)
    .owe_conf_source()
    .with(wparse_clean_context(
        &run_dir,
        OperationKind::LoadConfigFile,
    ))
    .with(&run_dir)
    .want("remove wparse runtime dir")
```

这里链路的语义比前一个例子更完整：

- `owe_conf_source()`
  把 `remove_dir_all` 的 `io::Error` 提升为结构化错误
- `with(wparse_clean_context(...))`
  附加 `RuntimeStage`、`OperationKind`、目录路径等上下文元数据
- `with(&run_dir)`
  再明确补一层目标路径
- `want("remove wparse runtime dir")`
  补充当前层操作目标

这就是为什么后面测试里可以直接断言错误报告中保留了：

- `RuntimeStage::SystemOperations`
- `DIR_PATH`
- `source_frames`

也就是：错误对象已经足够结构化，可以被程序直接读，而不必只靠字符串解析。

### 2.2.4 阶段四：跨 crate / 跨层转换

错误进入结构化体系之后，不代表马上就变成最终顶层错误。

在这两个工程里，常见有两种跨层动作。

#### 1. `err_conv()`：同一条错误链，换上层 reason 类型

`orion-error` 在 [conversion.rs:20](/Users/zuowenjian/devspace/wp-labs/dev/wparse/orion-error-pr/src/traits/conversion.rs#L20) 定义了 `err_conv()`。

它的语义是：

- 下层已经是 `StructError<R1>`
- 上层只想把它转换成 `StructError<R2>`
- detail / context / source 不丢

在 `warp-parse` 中，一个直接的例子是 [wpgen/conf.rs:6](/Users/zuowenjian/devspace/wp-labs/dev/wparse/warp-parse/src/wpgen/conf.rs#L6)：

```rust
pub async fn init(work_root: &str) -> RunResult<()> {
    gen_conf_init(work_root).err_conv()?;
    Ok(())
}
```

这里的含义不是“重新构造一个错误消息”，而是：

- `gen_conf_init(work_root)` 返回某个下层 `StructError<_>`
- `err_conv()` 把它提升为当前边界需要的 `RunResult`
- 结构化字段继续保留

另一个例子是 `wp-motor` 入口配置日志初始化 [engine.rs:90](/Users/zuowenjian/devspace/wp-labs/dev/wparse/wp-motor/src/facade/engine.rs#L90)：

```rust
log_init(main_conf.log_conf()).err_conv()?;
```

这说明在应用边界，工程更倾向于：

- 不重新发明 detail
- 不把错误压平
- 直接转换结构错误类型

#### 2. `wrap(...)` / `err_wrap(...)`：明确建立新的错误边界

如果上层不只是换类型，而是要明确建立一个新的语义边界，就会用 `wrap(...)`。

`wp-motor` 里典型例子还是 [wparse/mod.rs:55](/Users/zuowenjian/devspace/wp-labs/dev/wparse/wp-motor/crates/wp-proj/src/wparse/mod.rs#L55)：

```rust
.map_err(|e: StructError<ConfIOReason>| {
    e.wrap(wp_error::RunReason::from_conf())
        .with_detail("清理 wparse 运行目录失败")
})?;
```

这里和 `err_conv()` 的差异很关键：

- `err_conv()` 倾向于“沿着原错误链继续上浮”
- `wrap(...)` 倾向于“在这里定义一个新的上层边界”

也就是：

- 下层 `ConfIOReason` 仍保留在 source chain 里
- 当前层重新定义成 `RunReason::from_conf()`
- 当前层再补自己的 detail: `清理 wparse 运行目录失败`

这就是大型工程里非常需要的“错误边界语义”。

### 2.2.5 阶段五：顶层 CLI 统一消费 `RunError`

当错误最终到达 CLI 入口时，不再继续层层 `map_err(format!(...))`，而是统一交给诊断层。

例如：

- [wpgen/main.rs:23](/Users/zuowenjian/devspace/wp-labs/dev/wparse/warp-parse/src/wpgen/main.rs#L23)
- [wproj/main.rs:12](/Users/zuowenjian/devspace/wp-labs/dev/wparse/warp-parse/src/wproj/main.rs#L12)

`wpgen` 的入口非常典型：

```rust
if let Err(e) = do_main().await {
    print_run_error("wpgen", &e);
    std::process::exit(exit_code_for(e.reason()));
}
```

而 `print_run_error()` 的真实工作在 [diagnostics.rs:757](/Users/zuowenjian/devspace/wp-labs/dev/wparse/wp-motor/src/facade/diagnostics.rs#L757)：

```rust
pub fn print_run_error(app: &str, e: &RunError) {
    let summary = summarize_run_error(e);
    let report = e.report();
    let display_chain = e.display_chain();
    let hints = collect_report_hints(&report, &summary, &display_chain);
    let code = exit_code_for(e.reason());
    // ...
}
```

也就是说，顶层诊断层真正消费的是：

- `e.reason()`
- `e.report()`
- `e.display_chain()`
- `source_frames`
- `detail`
- `path`

而这些信息之所以存在，正是因为前面几层一直没有把错误压平成字符串。

### 2.2.6 把整条链画成一张图

可以把典型链路画成这样：

```text
std::io::Error
  |
  | owe_conf_source()
  v
StructError<ConfIOReason>
  - reason = conf/system 类别
  - detail = io error 文本
  - source = 原始 io::Error
  |
  | with(path) / with(OperationContext)
  | want("read file")
  v
StructError<ConfIOReason>
  - context/path/operation 已补齐
  |
  | err_conv()
  | 或 wrap(RunReason::from_conf())
  v
RunError
  - 顶层应用错误
  - 仍保留下层 source chain
  |
  | print_run_error()
  v
CLI 诊断输出
  - reason
  - detail
  - location
  - parse excerpt
  - root cause
  - hints
  - exit code
```

### 2.2.7 用一句话总结这条链

`owe_conf_source -> with -> want -> err_conv -> print_run_error`

这条链每一段的职责可以压缩成一句话：

- `owe_conf_source`
  把普通错误纳入统一分类，并保留真实 source。
- `with`
  把当前对象、路径、上下文挂到错误上。
- `want`
  说明当前层正在做什么，形成可追踪的操作路径。
- `err_conv`
  在不丢结构化信息的前提下跨层提升错误。
- `print_run_error`
  统一把结构化错误翻译成用户可读、可排障的 CLI 输出。

### 2.2.8 为什么这条链很重要

因为如果少掉任何一段，系统能力都会明显退化：

- 没有 `owe_conf_source`
  错误分类不统一，source 容易丢。
- 没有 `with` / `want`
  顶层知道“错了”，但不知道“在哪个操作、哪个对象上错了”。
- 没有 `err_conv`
  crate 边界会充满手写 `map_err`，错误契约很快变乱。
- 没有 `print_run_error`
  结构化错误到了 CLI 仍然只会打印成一长串字符串。

所以这条链不是语法糖，而是这两个工程里错误体系真正跑起来的主干。

## 3. 推荐的总体分层

推荐把错误分为 4 层。

### 3.1 基础设施层错误

面向：

- `std::io::Error`
- `serde_json::Error`
- `reqwest::Error`
- `git2::Error`
- 各类第三方库错误

这一层可以先保留细粒度错误，但不要直接暴露到系统顶层。

职责：

- 表示“底层发生了什么”
- 保留原始 `source`
- 不承担跨系统展示职责

### 3.2 领域层错误

面向：

- 配置错误
- Source / Sink 错误
- 运行时错误
- 语义校验错误
- DSL 解析错误

职责：

- 提供稳定分类
- 提供可编程判断的语义
- 不把动态上下文塞进枚举名

### 3.3 应用编排层错误

面向：

- crate 边界
- facade / handler / service 边界
- CLI / daemon / admin API 调用边界

职责：

- 统一错误返回类型
- 做错误提升与归一化
- 附加当前操作、当前对象、当前路径
- 保留上游 `source`

### 3.4 展示与运维层

面向：

- CLI
- HTTP 错误响应
- 日志
- 运维告警

职责：

- 从结构化错误提取可读摘要
- 输出定位信息
- 输出根因与 hints
- 做退出码映射

## 4. 推荐的错误对象结构

大型工程里，顶层错误至少应包含 5 类信息：

- `reason`
- `detail`
- `source`
- `context`
- `metadata`

它们的职责必须稳定。

### 4.1 `reason`

`reason` 负责稳定分类，不负责拼装当前层动态说明。

推荐：

```rust
#[derive(Debug, thiserror::Error)]
pub enum AppReason {
    #[error("configuration error")]
    Config,
    #[error("validation error")]
    Validation,
    #[error("runtime error")]
    Runtime,
    #[error("source error")]
    Source,
    #[error("sink error")]
    Sink,
    #[error("remote update error")]
    RemoteUpdate,
}
```

不推荐：

```rust
#[derive(Debug, thiserror::Error)]
pub enum AppReason {
    #[error("read /tmp/a.toml failed because file is empty")]
    ReadTmpATomlFailedBecauseFileIsEmpty,
}
```

原因：

- `reason` 要稳定
- `detail` 才应该放动态说明

### 4.2 `detail`

`detail` 负责描述“当前层在做什么时失败了”。

推荐：

- `"load engine config failed"`
- `"parse source config failed"`
- `"read token file /path/token failed"`
- `"requested version '1.4.3' was not found"`

不推荐：

- `"error"`
- `"failed"`
- `"configuration error"`

`detail` 应该帮助用户回答：

- 当前动作是什么
- 失败对象是什么
- 是哪一层补充的说明

### 4.3 `source`

`source` 负责保留上游根因。

推荐：

```rust
fn load_file(path: &Path) -> Result<String, AppError> {
    std::fs::read_to_string(path).map_err(|e| {
        AppError::new(AppReason::Config)
            .with_detail(format!("read {} failed", path.display()))
            .with_source(e)
    })
}
```

不推荐：

```rust
fn load_file(path: &Path) -> Result<String, AppError> {
    std::fs::read_to_string(path).map_err(|e| {
        AppError::new(AppReason::Config)
            .with_detail(format!("read {} failed: {}", path.display(), e))
    })
}
```

第二种写法把根因压扁成字符串了，后面无法再提取：

- 错误链
- 根因类型
- 精确的 source frame

### 4.4 `context`

`context` 负责描述调用现场。

建议至少记录：

- 当前操作
- 目标路径 / 资源
- 所属阶段
- 所属组件

例子：

```rust
#[derive(Debug, Clone)]
pub struct ErrorContext {
    pub operation: &'static str,
    pub stage: &'static str,
    pub component: &'static str,
    pub path: Option<std::path::PathBuf>,
}
```

### 4.5 `metadata`

`metadata` 负责机器可消费的信息。

推荐包含：

- 错误码
- 配置种类
- 组件种类
- hint code
- 运行阶段

`wp-motor` 的经验说明，只有底层持续附加这些元数据，顶层诊断层才能稳定生成：

- parse excerpt
- 路径定位
- 自动 hints
- exit code

## 5. 推荐的返回值契约

### 5.1 核心公共边界不要直接返回 `anyhow::Result`

推荐：

- 配置层返回自己的配置结果类型
- source/sink/runtime 返回自己的领域结果类型
- 应用编排层统一返回系统顶层结果类型

示例：

```rust
pub type AppResult<T> = Result<T, AppError>;
pub type SourceResult<T> = Result<T, SourceError>;
pub type SinkResult<T> = Result<T, SinkError>;
pub type ConfigResult<T> = Result<T, ConfigError>;
```

### 5.2 `Result<T, String>` 只能停留在局部工具层

`wp-motor` / `warp-parse` 里仍然能看到少量 `Result<T, String>`。这类写法在：

- 小型 checker 汇总
- 临时工具逻辑
- 迁移中的旧代码

还能接受，但不应该穿越核心模块边界。

建议规则：

- 模块内部临时用 `String` 可以
- 一旦要跨 crate / 跨模块 / 到 CLI，就必须提升为结构化错误

不推荐：

```rust
pub fn check_config(path: &Path) -> Result<(), String> {
    let body = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    parse_conf(&body).map_err(|e| e.to_string())?;
    Ok(())
}
```

推荐：

```rust
pub fn check_config(path: &Path) -> AppResult<()> {
    let body = std::fs::read_to_string(path).map_err(|e| {
        AppError::new(AppReason::Config)
            .with_detail(format!("read {} failed", path.display()))
            .with_source(e)
    })?;
    parse_conf(&body).map_err(|e| {
        AppError::new(AppReason::Config)
            .with_detail(format!("parse {} failed", path.display()))
            .with_source(e)
    })?;
    Ok(())
}
```

## 6. 推荐的错误传递方式

### 6.1 在边界层做转换，不在核心逻辑里乱转

这是两个工程里最值得复用的约束。

推荐规则：

- 下层模块保留自己的错误类型
- 在 facade / handler / adapter 边界统一转顶层错误
- 转换时只补当前层 detail/context，不重写根因

示例：

```rust
pub fn load_project(path: &Path) -> AppResult<Project> {
    config::load_project(path).map_err(|e| {
        AppError::new(AppReason::Config)
            .with_detail("load project config failed")
            .with_path(path)
            .with_source(e)
    })
}
```

### 6.2 保留 source，禁止过早 `to_string()`

坏例子：

```rust
.map_err(|e| AppError::new(AppReason::Runtime).with_detail(e.to_string()))
```

好例子：

```rust
.map_err(|e| {
    AppError::new(AppReason::Runtime)
        .with_detail("run background task failed")
        .with_source(e)
})
```

### 6.3 缺失文件、EOF、disabled 这类“预期状态”不要强行当异常

这一点 `project_remote/state.rs` 很典型。

推荐：

```rust
pub fn load_state(path: &Path) -> AppResult<Option<State>> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(Some(decode(&bytes)?)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(AppError::new(AppReason::Runtime)
            .with_detail(format!("read {} failed", path.display()))
            .with_source(e)),
    }
}
```

这样做的价值：

- 调用方可以用类型系统表达“可能不存在”
- 错误流不会被预期缺失淹没

## 7. 推荐的运行时错误处理策略

对于长生命周期运行时，不要只设计“错误类型”，还要设计“错误处置策略”。

推荐把两者分开：

```rust
#[derive(Debug, Clone, Copy)]
pub enum ErrorPolicy {
    Ignore,
    Retry,
    Tolerate,
    Terminate,
    Throw,
}
```

再做独立映射：

```rust
fn policy_for_source_error(err: &SourceError, mode: RobustnessMode) -> ErrorPolicy {
    match err.reason() {
        SourceReason::NotData => ErrorPolicy::Tolerate,
        SourceReason::Disconnect => ErrorPolicy::Retry,
        SourceReason::EOF => ErrorPolicy::Terminate,
        SourceReason::Validation => ErrorPolicy::Throw,
        SourceReason::System => {
            if mode == RobustnessMode::Strict {
                ErrorPolicy::Throw
            } else {
                ErrorPolicy::Retry
            }
        }
    }
}
```

这样做的价值：

- 错误分类稳定
- 策略可单独演进
- 同一错误可在不同模式下有不同处置

## 8. 推荐的诊断层设计

错误生产与错误展示应分离。

顶层诊断层建议做 5 件事：

- 提取主 reason
- 提取 detail
- 提取文件 / 资源位置
- 提取根因链
- 生成修复 hints

建议对外暴露一个稳定摘要对象：

```rust
pub struct DiagnosticSummary {
    pub reason: String,
    pub detail: Option<String>,
    pub location: Option<String>,
    pub root_cause: Option<String>,
    pub hints: Vec<String>,
    pub exit_code: i32,
}
```

CLI 层只消费 `DiagnosticSummary`，不要让每个子命令自己拼错误输出。

示例：

```rust
fn main() {
    if let Err(err) = do_main() {
        let diag = summarize_error(&err);
        eprintln!("ERROR: {}", diag.reason);
        if let Some(detail) = diag.detail {
            eprintln!("detail: {}", detail);
        }
        if let Some(location) = diag.location {
            eprintln!("file: {}", location);
        }
        for hint in diag.hints {
            eprintln!("hint: {}", hint);
        }
        std::process::exit(diag.exit_code);
    }
}
```

## 9. 推荐的回滚与恢复错误协议

`warp-parse` 的远程更新流程给出了一条非常值得复用的经验：

> 有副作用的更新流程，必须把“锁、快照、应用、校验、回滚”纳入错误设计，而不是只纳入业务流程。

推荐模式：

1. 加锁
2. 拍快照
3. 执行更新
4. 做强校验
5. 失败则回滚
6. 回滚失败要和主失败一起汇报

示例：

```rust
pub fn apply_update() -> AppResult<()> {
    let _lock = acquire_lock()?;
    let snapshot = capture_snapshot()?;

    let apply_result = do_apply();
    if let Err(apply_err) = apply_result {
        let rollback_result = restore_snapshot(&snapshot);
        return match rollback_result {
            Ok(()) => Err(AppError::new(AppReason::RemoteUpdate)
                .with_detail("apply update failed")
                .with_source(apply_err)),
            Err(rollback_err) => Err(AppError::new(AppReason::RemoteUpdate)
                .with_detail("apply update failed and rollback failed")
                .with_source(CombinedError::new(apply_err, rollback_err))),
        };
    }

    validate_after_apply()?;
    Ok(())
}
```

如果没有这套协议，大型工程在配置更新、模型更新、远端同步时会很脆弱。

## 10. 这两个工程里值得直接复用的具体模式

### 10.1 统一错误转换辅助

适用场景：

- 业务层想把不同来源错误统一映射到顶层错误

建议做一个统一 trait：

```rust
pub trait ResultExt<T, E> {
    fn to_app_err(self, context: &str) -> AppResult<T>;
    fn to_app_err_source(self, context: &str) -> AppResult<T>
    where
        E: std::error::Error + Send + Sync + 'static;
}
```

这样调用层会非常稳定：

```rust
let token = std::fs::read_to_string(&token_path)
    .to_app_err_source("read token file failed")?;
```

### 10.2 统一文件操作包装

适用场景：

- 文件存在性检查
- 读写文件
- 创建目录

推荐：

```rust
pub struct ErrorHelper;

impl ErrorHelper {
    pub fn safe_read(path: &Path) -> AppResult<String> {
        std::fs::read_to_string(path).map_err(|e| {
            AppError::new(AppReason::Config)
                .with_detail(format!("read {} failed", path.display()))
                .with_path(path)
                .with_source(e)
        })
    }

    pub fn safe_write(path: &Path, body: &str) -> AppResult<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                AppError::new(AppReason::Runtime)
                    .with_detail(format!("create {} failed", parent.display()))
                    .with_path(parent)
                    .with_source(e)
            })?;
        }
        std::fs::write(path, body).map_err(|e| {
            AppError::new(AppReason::Runtime)
                .with_detail(format!("write {} failed", path.display()))
                .with_path(path)
                .with_source(e)
        })
    }
}
```

### 10.3 DSL / 配置解析错误带 excerpt

适用场景：

- TOML / YAML / DSL 解析
- WPL / OML 这类自定义语言

推荐输出结构：

```rust
pub struct ParseDiagnostic {
    pub path: String,
    pub excerpt: String,
    pub line: usize,
    pub column: usize,
    pub message: String,
}
```

不要只返回：

```rust
Err("parse failed".to_string())
```

推荐：

```rust
Err(AppError::new(AppReason::Config)
    .with_detail("parse source config failed")
    .with_path(path)
    .with_metadata("line", line)
    .with_metadata("column", column)
    .with_metadata("excerpt", excerpt)
    .with_source(parse_err))
```

## 11. 不推荐的反模式

### 11.1 把所有错误都包成 `anyhow`

坏处：

- 失去稳定分类
- 后续很难补错误码、阶段、组件、hint

### 11.2 把所有错误都降级成字符串

坏处：

- source chain 消失
- 根因定位困难
- 调用方只能靠字符串匹配

### 11.3 在每一层都重新发明错误枚举

坏处：

- 转换成本过高
- 错误语义碎片化
- 边界契约混乱

推荐：

- 底层少量局部错误
- 领域层稳定错误
- 顶层统一错误

### 11.4 让 CLI 自己拼业务错误文本

坏处：

- 同类错误展示不一致
- 无法统一抽取 hint / exit code / location

推荐：

- 统一 `summarize_error`
- 统一 `print_error`

### 11.5 预期缺失也走异常

坏处：

- 大量噪音
- 正常分支被错误流污染

推荐：

- `Option<T>`
- 状态枚举
- `NotFound` 特判为正常状态

## 12. 推荐的最小模板

下面给出一套可直接起步的最小模板。

```rust
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppReason {
    #[error("configuration error")]
    Config,
    #[error("validation error")]
    Validation,
    #[error("runtime error")]
    Runtime,
}

#[derive(Debug)]
pub struct AppError {
    pub reason: AppReason,
    pub detail: Option<String>,
    pub path: Option<PathBuf>,
    pub operation: Option<&'static str>,
    pub source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

pub type AppResult<T> = Result<T, AppError>;

impl AppError {
    pub fn new(reason: AppReason) -> Self {
        Self {
            reason,
            detail: None,
            path: None,
            operation: None,
            source: None,
        }
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn with_path(mut self, path: impl AsRef<Path>) -> Self {
        self.path = Some(path.as_ref().to_path_buf());
        self
    }

    pub fn with_operation(mut self, operation: &'static str) -> Self {
        self.operation = Some(operation);
        self
    }

    pub fn with_source<E>(mut self, source: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        self.source = Some(Box::new(source));
        self
    }
}

pub fn load_engine_config(path: &Path) -> AppResult<String> {
    std::fs::read_to_string(path).map_err(|e| {
        AppError::new(AppReason::Config)
            .with_operation("load engine config")
            .with_detail(format!("read {} failed", path.display()))
            .with_path(path)
            .with_source(e)
    })
}
```

这套模板还不够完整，但已经具备大型工程最关键的骨架：

- 稳定 reason
- 当前层 detail
- 明确 path
- 操作名
- source chain

## 13. 增量迁移建议

如果现有工程已经充满：

- `anyhow::Result`
- `Result<T, String>`
- `map_err(|e| e.to_string())`

建议按下面顺序迁移。

### 第一步

先定义统一顶层错误类型和结果别名。

### 第二步

优先改造边界层：

- CLI handler
- facade
- service
- adapter

因为这里最容易产生成果，且不会立刻重写全部底层。

### 第三步

把“文件 / 配置 / HTTP / Git / 远程调用”这些高频失败点统一包起来。

### 第四步

补诊断层：

- 提取 location
- 提取 root cause
- 提取 parse excerpt
- 生成 hints

### 第五步

最后再消灭深层 `Result<T, String>`。

## 14. 评审清单

评审错误设计时可以直接问：

1. 这个边界为什么不是结构化错误？
2. 这里有没有丢失 `source`？
3. `detail` 是否说明了当前层动作？
4. 这里是否应该带路径、组件、阶段？
5. 顶层 CLI 最终能否给出可定位的诊断？
6. 这个错误应不应该区分可重试与不可重试？
7. 这里是否需要快照、回滚、恢复协议？

## 15. 最终建议

基于 `wp-motor` 与 `warp-parse` 的经验，可以把大型工程错误设计压缩为 7 条落地规则：

- 用结构化错误作为核心链路主模型，不用 `anyhow` 充当全局错误模型。
- `reason` 做稳定分类，`detail` 写当前动作，`source` 保留根因。
- 在边界层统一转换，在核心逻辑里避免无序转换。
- 持续附加 `operation/path/stage/component` 等上下文元数据。
- 诊断层独立设计，统一负责摘要、位置、根因、hints、exit code。
- 把错误语义和运行时处置策略分开建模。
- 对有副作用的更新流程，把锁、快照、回滚、恢复纳入错误协议。

如果只能先做一件事，优先做这件：

> 禁止新的 `Result<T, String>` 和 `map_err(|e| e.to_string())` 穿越公共边界。

这是把错误系统从“能报错”升级到“能维护、能排障、能演进”的第一步。
