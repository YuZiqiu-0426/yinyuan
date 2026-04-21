# Y2M 本地快速上手

## 1. 编译打包

需要提前安装 [Rust 工具链](https://rustup.rs/)（stable 版本即可）。

```bash
# 在项目根目录执行
cargo build --release -p y2m         # 只打包客户端
cargo build --release -p y2m-server  # 只打包服务端

```

•  只构建服务端：cargo build --release -p y2m-server
•  构建整个工作区：cargo build --release --workspace
•  一次构建客户端+服务端：cargo build --release -p y2m -p y2m-serve

编译完成后，两个可执行文件位于 `target/release/`：

| 文件 | 用途 |
|------|------|
| `y2m-server` | 中转服务器 |
| `y2m` | 客户端 CLI |

---

## 2. 启动服务器

### 默认地址（127.0.0.1:8080）

```bash
./target/release/y2m-server
```

### 自定义地址

```bash
Y2M_SERVER_ADDR=0.0.0.0:9090 ./target/release/y2m-server
```

服务器启动后会打印类似：

```
{"level":"INFO","message":"y2m server listening","addr":"127.0.0.1:8080"}
```

---

## 3. 初始化客户端配置

每个客户端需要一个 JSON 配置文件，通过 `init` 子命令生成。

**客户端 Alice**（终端 2）：

```bash
./target/release/y2m init \
  --config alice.json \
  --server-url ws://127.0.0.1:8080/ws \
  --group mygroup \
  --client alice
```

**客户端 Bob**（终端 3）：

```bash
./target/release/y2m init \
  --config bob.json \
  --server-url ws://127.0.0.1:8080/ws \
  --group mygroup \
  --client bob \
  --download-dir ./bob-downloads
```

> `--download-dir` 指定接收文件的保存目录，缺省时使用当前目录下的 `downloads/`。

生成的 `alice.json` / `bob.json` 可直接用文本编辑器查看或修改。

---

## 4. 以交互模式（chat）启动两个客户端

**终端 2 — Alice**：

```bash
./target/release/y2m chat --config alice.json --to bob
```

**终端 3 — Bob**：

```bash
./target/release/y2m chat --config bob.json
```

连接成功后每个客户端会打印：

```
当前会话: group=mygroup, to=bob   # Alice 侧（已指定目标）
当前会话: group=mygroup, to=*     # Bob 侧（广播模式）
```

---

## 5. Chat 交互命令速查

直接在 chat 提示符下输入文字即可发送文本消息。斜杠命令如下：

### 会话控制

| 命令 | 说明 |
|------|------|
| `/to <client>` | 切换目标用户（单播） |
| `/to` | 清空目标，切换回广播 |
| `/group <group>` | 切换目标分组 |
| `/group` | 恢复默认分组 |
| `/status` | 显示当前会话信息 |
| `/help` | 显示帮助 |
| `/exit` | 退出 chat |

### 消息

| 命令 | 说明 |
|------|------|
| `<任意文字>` | 发送文本消息 |
| `/json <json>` | 发送 JSON 消息 |
| `/command <cmd>` | 远程执行命令并等待结果 |

### 文件传输

| 命令 | 说明 |
|------|------|
| `/file <路径>` | 向当前目标发送文件（需先 `/to` 指定目标） |
| `/files` | 查看本地所有待处理文件 |
| `/accept <fileId>` | 接受收到的文件请求 |
| `/reject <fileId>` | 拒绝收到的文件请求 |
| `/abort <fileId>` | 取消正在进行的文件传输 |

---

## 6. 完整示例流程

以下假设服务器已在 `127.0.0.1:8080` 运行，Alice 和 Bob 均已进入 chat 模式。

### 发送文本消息

```
# Alice 终端
hello from alice
```

Bob 收到：

```
[mygroup][alice] hello from alice
```

### 发送文件

```
# Alice 终端（需先指定目标）
/to bob
/file /path/to/photo.jpg
```

Bob 看到：

```
收到文件请求: id=xxxxxxxx-..., from=alice, name=photo.jpg, size=...
```

```
# Bob 终端
/accept xxxxxxxx-...
```

传输完成后：
- Bob：`文件已保存: ./bob-downloads/photo.jpg`
- Alice：`文件发送完成: photo.jpg`

### 取消文件传输

```
# Bob 终端（传输进行中）
/abort xxxxxxxx-...
```

双方均会收到取消通知。

---

## 7. 一次性发送（不进入交互模式）

如果只需要发送一条消息，可以直接用 `send` 子命令：

```bash
# 发送文本
./target/release/y2m send --config alice.json text --to bob "hello"

# 发送 JSON
./target/release/y2m send --config alice.json json --to bob '{"key":"value"}'

# 远程执行命令
./target/release/y2m send --config alice.json command --to bob "ls -la"

# 发送文件
./target/release/y2m send --config alice.json file --to bob /path/to/file.txt
```

---

## 8. 以守护模式（run）运行

`run` 模式只接收，不发送，适合在后台常驻：

```bash
./target/release/y2m run --config bob.json --reconnect-interval-sec 5
```

服务器断开后会每隔 5 秒自动重连。
