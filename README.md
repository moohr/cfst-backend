# CFST-backend
用于将 cloudflare IP 解析至 aliyun 以优化访问的服务端。    
该项目使用 gRPC 进行客户端与服务器之间的通信。完整的通信实现请参见 `proto/cfst_rpc.proto`。

此项目目前在alpha状态。使用风险自负。不提供任何担保。

## 安装
1. 安装 `rust` 工具链。参见 [此处](https://rustup.rs)
2. 安装 sqlite3，[protoc](https://grpc.io/docs/protoc-installation/)。
3. git clone https://github.com/moohr/cfst-backend.git && cd cfst-backend && cargo build --release`
4. 抓取 `qqwry.dat` 并将其放在 assets/ 下。
5. 创建 sqlite 数据库：
```
$ sqlite3 aliyundns.db
CREATE TABLE records (
    record_id TEXT PRIMARY KEY、
    isp TEXT NOT NULL、
    province TEXT NOT NULL、
);
```
6. cargo build --release     
   编译完成后，可在 ./target/release/cfst-backend 中查看
7. 填写`.env`

## 待办事项
- [ ] 为手动交互添加 gRPC 端点
- [ ] 将 IP 数据库迁移为使用新的 CZDB 格式
- [ ] 添加更多功能
- [ ] 创建用于分配特定测试范围的算法

# 免责声明
- 本项目不提供任何网站数据。
- 本项目的主要目的是为在中国大陆拥有受众的网站创建一个公开可用的 CNAME 目标。如发现任何非法网站，请直接联系 Cloudflare 举报。
- 本项目不会使使用此服务的网站消耗更多带宽。
- 本项目与 Cloudflare 或阿里云无关。
- 此项目严禁用于违反中国大陆法律的用途，包括但不限于翻墙、黄赌毒、钓鱼、流氓广告、版权内容、虚假内容、木马、病毒、爆破、政治、恐吓等站点。