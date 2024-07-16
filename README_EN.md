# CFST-backend
A backend controller for resolving the fastest Cloudflare IP at the provincial level in China to Aliyun DNS for access optimization.     
This projects uses gRPC for client-server communications. See `proto/cfst_rpc.proto` for the full communication implementation.

This is alpha software. Use at your own risk. No warranty is provided.

## Install
1. Install the `rust` toolchain. See [here](https://rustup.rs)
2. Install sqlite3, [protoc](https://grpc.io/docs/protoc-installation/).
3. `git clone https://github.com/moohr/cfst-backend.git && cd cfst-backend && cargo build --release`
4. Grab `qqwry.dat` and place it under assets/.
5. Create sqlite database:
```
$ sqlite3 aliyundns.db
CREATE TABLE records (
    record_id TEXT PRIMARY KEY,
    isp TEXT NOT NULL,
    province TEXT NOT NULL,
);
```
6. `cargo build --release`     
   The finished build should be available at ./target/release/cfst-backend
7. Fill in `.env`

## Todo
- [ ] Add gRPC endpoints for manual interaction
- [ ] Migrate IP database to use the new CZDB format
- [ ] Add more features
- [ ] Create algorithm for allocating specific ranges for testing

## Disclaimer
- This project does not provide any website data.
- The main aim of this project is to create a publicly usable CNAME target for websites that have audiences in mainland China. Please contact Cloudflare directly to report any websites that violates their TOS. 
- This project does not make any websites that utilizes this service consume more bandwidth. 
- This project is not affiliated with Cloudflare or Aliyun.
- This project is strictly prohibited for use in violation of mainland China laws, including but not limited to circumventing the National Firewall, pornography, gambling, phishing, rogue advertising, copyright content, fake news/facts, trojans, viruses, brute forcing, politics, intimidation etc.

## Acknowledgements
- IP address location data is supported by [纯真CZ88](https://www.cz88.net)
