# JSON-RPC

# Contents
- [JSON-RPC](#json-rpc)
- [Contents](#contents)
  - [Getting Blocks](#getting-blocks)
    - [eth_blockNumber](#eth_blocknumber)
    - [eth_getBlockByHash](#eth_getblockbyhash)
    - [eth_getBlockByNumber](#eth_getblockbynumber)
  - [Reading Transactions](#reading-transactions)
    - [eth_getTransactionByHash](#eth_gettransactionbyhash)
    - [eth_getTransactionCount](#eth_gettransactioncount)
    - [eth_getTransactionReceipt](#eth_gettransactionreceipt)
    - [eth_getBlockTransactionCountByHash](#eth_getblocktransactioncountbyhash)
    - [eth_getBlockTransactionCountByNumber](#eth_getblocktransactioncountbynumber)
    - [eth_getTransactionByBlockHashAndIndex](#eth_gettransactionbyblockhashandindex)
    - [eth_getTransactionByBlockNumberAndIndex](#eth_gettransactionbyblocknumberandindex)
  - [Writing Transactions](#writing-transactions)
    - [eth_sendRawTransaction](#eth_sendrawtransaction)
  - [Account Information](#account-information)
    - [eth_getBalance](#eth_getbalance)
    - [eth_getStorageAt](#eth_getstorageat)
    - [eth_getCode](#eth_getcode)
    - [eth_accounts](#eth_accounts)
  - [EVM/Smart Contract Execution](#evmsmart-contract-execution)
    - [eth_call](#eth_call)
  - [Event Logs](#event-logs)
    - [eth_getLogs](#eth_getlogs)
    - [eth_getFilterChanges](#eth_getfilterchanges)
    - [eth_getFilterLogs](#eth_getfilterlogs)
    - [eth_newBlockFilter](#eth_newblockfilter)
    - [eth_newFilter](#eth_newfilter)
    - [eth_newPendingTransactionFilter](#eth_newpendingtransactionfilter)
    - [eth_uninstallFilter](#eth_uninstallfilter)
  - [Chain Information](#chain-information)
    - [eth_protocolVersion](#eth_protocolversion)
    - [eth_gasPrice](#eth_gasprice)
    - [eth_estimateGas](#eth_estimategas)
    - [eth_chainId](#eth_chainid)
    - [net_version](#net_version)
    - [net_listening](#net_listening)
  - [Getting Uncles](#getting-uncles)
    - [eth_getUncleByBlockHashAndIndex](#eth_getunclebyblockhashandindex)
    - [eth_getUncleByBlockNumberAndIndex](#eth_getunclebyblocknumberandindex)
    - [eth_getUncleCountByBlockHash](#eth_getunclecountbyblockhash)
    - [eth_getUncleCountByBlockNumber](#eth_getunclecountbyblocknumber)
  - [Web3](#web3)
    - [web3_clientVersion](#web3_clientversion)
    - [web3_sha3](#web3_sha3)
  - [others](#others)
    - [eth_syncing](#eth_syncing)
    - [eth_coinbase](#eth_coinbase)
    - [net_peerCount](#net_peercount)
    - [eth_mining](#eth_mining)
    - [eth_hashrate](#eth_hashrate)
    - [eth_sign](#eth_sign)
    - [eth_signTransaction](#eth_signtransaction)
    - [eth_sendTransaction](#eth_sendtransaction)
    - [eth_getCompilers](#eth_getcompilers)
    - [eth_compileSolidity](#eth_compilesolidity)
    - [eth_compileLLL](#eth_compilelll)
    - [eth_compileSerpent](#eth_compileserpent)
    - [eth_getWork](#eth_getwork)
    - [eth_submitWork](#eth_submitwork)
    - [eth_submitHashrate](#eth_submithashrate)
- [Referance](#referance)

## Getting Blocks

### eth_blockNumber
- [x] Ethereum JSON-RPC API
- [x] Canister need support
- [x] Already supported

```sh
curl https://eth-mainnet.alchemyapi.io/v2/demo \
-X POST \
-H 'content-type: application/json' \
-d '{"jsonrpc":"2.0","id":1,"method":"eth_blockNumber","params":[]}'

{"jsonrpc":"2.0","id":1,"result":"0xed8d1f"}%
```

### eth_getBlockByHash
- [x] Ethereum JSON-RPC API
- [x] Canister need support
- [x] Already supported

```sh
curl https://eth-mainnet.alchemyapi.io/v2/demo \
-X POST \
-H 'content-type: application/json' \
-d '{"jsonrpc":"2.0","id":1,"method":"eth_getBlockByHash","params":["0x9a834c53bbee9c2665a5a84789a1d1ad73750b2d77b50de44f457f411d02e52e", true]}'

{"jsonrpc":"2.0","id":1,"result":{"number":"0x100000","hash":"0x9a834c53bbee9c2665a5a84789a1d1ad73750b2d77b50de44f457f411d02e52e","transactions":[{"blockHash":"0x9a834c53bbee9c2665a5a84789a1d1ad73750b2d77b50de44f457f411d02e52e","blockNumber":"0x100000","hash":"0x01c5a8461d06c2c195035c148af0f871c7679841d86ae5bb98676bb2d8e68dfa","from":"0x68795c4aa09d6f4ed3e5deddf8c2ad3049a601da","gas":"0x15f90","gasPrice":"0xba43b7400","input":"0x","nonce":"0x2f31","r":"0x1d263f32de00456cbc3e4099c623a6ca48e3188f01dd16461687512f8c3bb7d6","s":"0x62629f42a98f7c5fa60f8c3630cf67b5eb4772e0ff65a458d2116c15e22d6451","to":"0xdae787ec66e65c60ad35203800b97b45fcb0f909","transactionIndex":"0x0","type":"0x0","v":"0x1b","value":"0x2b81097c919a9e000"},{"blockHash":"0x9a834c53bbee9c2665a5a84789a1d1ad73750b2d77b50de44f457f411d02e52e","blockNumber":"0x100000","hash":"0xb4216e88df6ebfe666bbe43370d1ddfe8d9c1975d2b8375a522700f7f59c3ed0","from":"0x68795c4aa09d6f4ed3e5deddf8c2ad3049a601da","gas":"0x15f90","gasPrice":"0xba43b7400","input":"0x","nonce":"0x2f32","r":"0xc25ceef654cd3bd734201fc08589970e0663431121bc47cc4a70335e072b09c0","s":"0x1c67b037a59a8f5badde3a7eb586d7888dcf6dad64aabb080c4c0db593a91254","to":"0x04a2a3ee9c9ea82e1f918ece209f8a7e6d7521e7","transactionIndex":"0x1","type":"0x0","v":"0x1c","value":"0x377fb2f2cbd73c00"}],"difficulty":"0xc6d2fa46fd6","extraData":"0xd783010400844765746887676f312e352e31856c696e7578","gasLimit":"0x2fefd8","gasUsed":"0xa410","logsBloom":"0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000","miner":"0x68795c4aa09d6f4ed3e5deddf8c2ad3049a601da","mixHash":"0x72224fd0b7b32d0fea13f2953eba28d2512af43632725ed75583dd30e42dfc64","nonce":"0xf1a453a44f9de627","parentHash":"0x2dcaf9a8a3fd329d925eb47c1f22022d4f2d562500e546d878bad4247d013858","receiptsRoot":"0x178000485de197ed06a4ebada431864e5b564b43f994fb8888521f74b4b3257b","sha3Uncles":"0x1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347","size":"0x303","stateRoot":"0xeaddaa0b6b673ba3f8cb7235ca656340ccf703295cf5252cb04a4202c47252c2","timestamp":"0x56cc7b73","totalDifficulty":"0x6bab93cdf810b22a","transactionsRoot":"0x91d9b419b34a790cc2a7d59c6b95e57ccc5ec9bc1034ce9510dfb551d3049c93","uncles":[]}}%
```

### eth_getBlockByNumber
- [x] Ethereum JSON-RPC API
- [x] Canister need support
- [x] Already supported

```sh
curl https://eth-mainnet.alchemyapi.io/v2/demo \
-X POST \
-H 'content-type: application/json' \
-d '{"jsonrpc":"2.0","id":1,"method":"eth_getBlockByNumber","params":["0x100000", true]}'

{"jsonrpc":"2.0","id":1,"result":{"number":"0x100000","hash":"0x9a834c53bbee9c2665a5a84789a1d1ad73750b2d77b50de44f457f411d02e52e","transactions":[{"blockHash":"0x9a834c53bbee9c2665a5a84789a1d1ad73750b2d77b50de44f457f411d02e52e","blockNumber":"0x100000","hash":"0x01c5a8461d06c2c195035c148af0f871c7679841d86ae5bb98676bb2d8e68dfa","from":"0x68795c4aa09d6f4ed3e5deddf8c2ad3049a601da","gas":"0x15f90","gasPrice":"0xba43b7400","input":"0x","nonce":"0x2f31","r":"0x1d263f32de00456cbc3e4099c623a6ca48e3188f01dd16461687512f8c3bb7d6","s":"0x62629f42a98f7c5fa60f8c3630cf67b5eb4772e0ff65a458d2116c15e22d6451","to":"0xdae787ec66e65c60ad35203800b97b45fcb0f909","transactionIndex":"0x0","type":"0x0","v":"0x1b","value":"0x2b81097c919a9e000"},{"blockHash":"0x9a834c53bbee9c2665a5a84789a1d1ad73750b2d77b50de44f457f411d02e52e","blockNumber":"0x100000","hash":"0xb4216e88df6ebfe666bbe43370d1ddfe8d9c1975d2b8375a522700f7f59c3ed0","from":"0x68795c4aa09d6f4ed3e5deddf8c2ad3049a601da","gas":"0x15f90","gasPrice":"0xba43b7400","input":"0x","nonce":"0x2f32","r":"0xc25ceef654cd3bd734201fc08589970e0663431121bc47cc4a70335e072b09c0","s":"0x1c67b037a59a8f5badde3a7eb586d7888dcf6dad64aabb080c4c0db593a91254","to":"0x04a2a3ee9c9ea82e1f918ece209f8a7e6d7521e7","transactionIndex":"0x1","type":"0x0","v":"0x1c","value":"0x377fb2f2cbd73c00"}],"difficulty":"0xc6d2fa46fd6","extraData":"0xd783010400844765746887676f312e352e31856c696e7578","gasLimit":"0x2fefd8","gasUsed":"0xa410","logsBloom":"0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000","miner":"0x68795c4aa09d6f4ed3e5deddf8c2ad3049a601da","mixHash":"0x72224fd0b7b32d0fea13f2953eba28d2512af43632725ed75583dd30e42dfc64","nonce":"0xf1a453a44f9de627","parentHash":"0x2dcaf9a8a3fd329d925eb47c1f22022d4f2d562500e546d878bad4247d013858","receiptsRoot":"0x178000485de197ed06a4ebada431864e5b564b43f994fb8888521f74b4b3257b","sha3Uncles":"0x1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347","size":"0x303","stateRoot":"0xeaddaa0b6b673ba3f8cb7235ca656340ccf703295cf5252cb04a4202c47252c2","timestamp":"0x56cc7b73","totalDifficulty":"0x6bab93cdf810b22a","transactionsRoot":"0x91d9b419b34a790cc2a7d59c6b95e57ccc5ec9bc1034ce9510dfb551d3049c93","uncles":[]}}%
```

## Reading Transactions

### eth_getTransactionByHash
- [x] Ethereum JSON-RPC API
- [x] Canister need support
- [x] Already supported

```sh
curl https://eth-mainnet.alchemyapi.io/v2/demo \
-X POST -H 'content-Type: application/json' \
-d '{"jsonrpc":"2.0","id":1,"method":"eth_getTransactionByHash","params":["0x6a8128d38a5eeefd8c102423c74570d0a8f733e4ab06b8539eeaa407115cc1c6"]}'

{"jsonrpc":"2.0","id":1,"result":{"blockHash":"0xf3cd331b8af49500e288470107d90a2f01712879f50754f7c993f99b39299321","blockNumber":"0xedd8c4","hash":"0x6a8128d38a5eeefd8c102423c74570d0a8f733e4ab06b8539eeaa407115cc1c6","accessList":[],"chainId":"0x1","from":"0xd12c3b755d9fda802b0efae5f9c74f7c683ceb13","gas":"0x56bc4","gasPrice":"0xde01c39d","input":"0x49318354000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000400000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000000a000000000000000000000000000000000000000000000000000000000000000e00000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000959d0000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000916100000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000009167","maxFeePerGas":"0xde01c39d","maxPriorityFeePerGas":"0x43c9016f","nonce":"0x2063","r":"0x3fa41d9fc2e3f1a7051eddfc738da5a4be6f91a550e9aaf84dbc82be19cb6fae","s":"0x7e48dd38ac7ff16e9c0ac33aa19923cd6603b4838cb0a718a9928cae2bdaa219","to":"0xe544cf993c7d477c7ef8e91d28aca250d135aa03","transactionIndex":"0x39","type":"0x2","v":"0x0","value":"0x0"}}%
```

### eth_getTransactionCount
- [x] Ethereum JSON-RPC API
- [x] Canister need support
- [x] Already supported

```sh
curl https://eth-mainnet.alchemyapi.io/v2/demo \
-X POST -H 'content-Type: application/json' \
-d '{"jsonrpc":"2.0","id":1,"method":"eth_getTransactionCount","params":["0xbd70d89667a3e1bd341ac235259c5f2dde8172a9", "latest"]}'

{"jsonrpc":"2.0","id":1,"result":"0x0"}%
```

### eth_getTransactionReceipt
- [x] Ethereum JSON-RPC API
- [x] Canister need support
- [x] Already supported

```sh
curl https://eth-mainnet.alchemyapi.io/v2/demo \
-X POST -H 'content-Type: application/json' \
-d '{"jsonrpc":"2.0","id":1,"method":"eth_getTransactionReceipt","params":["0x6a8128d38a5eeefd8c102423c74570d0a8f733e4ab06b8539eeaa407115cc1c6"]}'

{"jsonrpc":"2.0","id":1,"result":{"transactionHash":"0x6a8128d38a5eeefd8c102423c74570d0a8f733e4ab06b8539eeaa407115cc1c6","blockHash":"0xf3cd331b8af49500e288470107d90a2f01712879f50754f7c993f99b39299321","blockNumber":"0xedd8c4","logs":[],"contractAddress":null,"effectiveGasPrice":"0xde01c39d","cumulativeGasUsed":"0x46488a","from":"0xd12c3b755d9fda802b0efae5f9c74f7c683ceb13","gasUsed":"0x24ea4","logsBloom":"0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000","status":"0x1","to":"0xe544cf993c7d477c7ef8e91d28aca250d135aa03","transactionIndex":"0x39","type":"0x2"}}%
```

### eth_getBlockTransactionCountByHash
- [x] Ethereum JSON-RPC API
- [x] Canister need support
- [x] Already supported

```sh
curl https://eth-mainnet.alchemyapi.io/v2/demo \
-X POST -H 'content-Type: application/json' \
-d '{"jsonrpc":"2.0","id":1,"method":"eth_getBlockTransactionCountByHash","params":["0x9a834c53bbee9c2665a5a84789a1d1ad73750b2d77b50de44f457f411d02e52e"]}'

{"jsonrpc":"2.0","id":1,"result":"0x2"}%
```

### eth_getBlockTransactionCountByNumber
- [x] Ethereum JSON-RPC API
- [x] Canister need support
- [x] Already supported

```sh
curl https://eth-mainnet.alchemyapi.io/v2/demo \
-X POST -H 'content-Type: application/json' \
-d '{"jsonrpc":"2.0","id":1,"method":"eth_getBlockTransactionCountByNumber","params":["0x100000"]}'

{"jsonrpc":"2.0","id":1,"result":"0x2"}%
```

### eth_getTransactionByBlockHashAndIndex
- [x] Ethereum JSON-RPC API
- [x] Canister need support
- [x] Already supported

```sh
curl https://eth-mainnet.alchemyapi.io/v2/demo \
-X POST -H 'content-Type: application/json' \
-d '{"jsonrpc":"2.0","id":1,"method":"eth_getTransactionByBlockHashAndIndex","params":["0xf3cd331b8af49500e288470107d90a2f01712879f50754f7c993f99b39299321","0x0"]}'

{"jsonrpc":"2.0","id":1,"result":{"blockHash":"0xf3cd331b8af49500e288470107d90a2f01712879f50754f7c993f99b39299321","blockNumber":"0xedd8c4","from":"0xfbb1b73c4f0bda4f67dca266ce6ef42f520fbb98","gas":"0x249f0","gasPrice":"0x6fc23ac00","hash":"0x3ef6b6b5c26b647a08b0581f96a530724cba8e0ca0c7b901ace9055d878fccdf","input":"0x","nonce":"0xaa6f3b","to":"0xd86b048cc53c5fc2c86cc89d8e046e79804ea661","transactionIndex":"0x0","value":"0x849370e4336400","type":"0x0","chainId":"0x1","v":"0x25","r":"0xbeb873e3b6d2934f2c2bb356128740da2b858db4282a472d87b131b963fee39e","s":"0x5b294a9f173c9c3d0b290e488318db7c4a842652c2a2589fba966c7349a50f4c"}}%
```

### eth_getTransactionByBlockNumberAndIndex
- [x] Ethereum JSON-RPC API
- [x] Canister need support
- [x] Already supported

```sh
curl https://eth-mainnet.alchemyapi.io/v2/demo \
-X POST -H 'content-Type: application/json' \
-d '{"jsonrpc":"2.0","id":1,"method":"eth_getTransactionByBlockNumberAndIndex","params":["0xEDD8C4","0x0"]}'
# 15587524
{"jsonrpc":"2.0","id":1,"result":{"blockHash":"0xf3cd331b8af49500e288470107d90a2f01712879f50754f7c993f99b39299321","blockNumber":"0xedd8c4","from":"0xfbb1b73c4f0bda4f67dca266ce6ef42f520fbb98","gas":"0x249f0","gasPrice":"0x6fc23ac00","hash":"0x3ef6b6b5c26b647a08b0581f96a530724cba8e0ca0c7b901ace9055d878fccdf","input":"0x","nonce":"0xaa6f3b","to":"0xd86b048cc53c5fc2c86cc89d8e046e79804ea661","transactionIndex":"0x0","value":"0x849370e4336400","type":"0x0","chainId":"0x1","v":"0x25","r":"0xbeb873e3b6d2934f2c2bb356128740da2b858db4282a472d87b131b963fee39e","s":"0x5b294a9f173c9c3d0b290e488318db7c4a842652c2a2589fba966c7349a50f4c"}}%

```

## Writing Transactions
### eth_sendRawTransaction
- [x] Ethereum JSON-RPC API
- [x] Canister need support
- [x] Already supported

```sh
curl https://eth-mainnet.alchemyapi.io/v2/demo \
-X POST -H 'content-Type: application/json' \
-d '{"jsonrpc":"2.0","id":1,"method":"eth_sendRawTransaction","params":["0xd46e8dd67c5d32be8d46e8dd67c5d32be8058bb8eb970870f072445675058bb8eb970870f072445675"]}'

{"jsonrpc":"2.0","id":1,"error":{"code":-32000,"message":"rlp: element is larger than containing list"}}%
```

## Account Information

### eth_getBalance
- [x] Ethereum JSON-RPC API
- [x] Canister need support
- [x] Already supported

```sh
curl https://eth-mainnet.alchemyapi.io/v2/demo \
-X POST -H 'content-Type: application/json' \
-d '{"jsonrpc":"2.0","id":1,"method":"eth_getBalance","params":["0xde0B295669a9FD93d5F28D9Ec85E40f4cb697BAe", "latest"]}'

{"jsonrpc":"2.0","id":1,"result":"0x484456e6bc71b489eb07"}%
```

### eth_getStorageAt

- [x] Ethereum JSON-RPC API
- [x] Canister need support
- [x] Already supported

```sh
curl https://eth-mainnet.alchemyapi.io/v2/demo \
-X POST -H 'content-Type: application/json' \
-d '{"jsonrpc":"2.0","id":1,"method":"eth_getStorageAt","params": ["0x00000000219ab540356cBB839Cbe05303d7705Fa", "0x0", "latest"], "id": 1}}'

{"jsonrpc":"2.0","id":1,"result":"0x2b442e0c52aabd4dad489be054aa15f70b895fa7e55a78ccd74611921ff399ae"}%
```

### eth_getCode

- [x] Ethereum JSON-RPC API
- [x] Canister need support
- [x] Already supported

```sh
curl https://eth-mainnet.alchemyapi.io/v2/demo \
-X POST -H 'content-Type: application/json' \
-d '{"jsonrpc":"2.0","id":1,"method":"eth_getCode","params":["0x00000000219ab540356cBB839Cbe05303d7705Fa", "latest"]}'

{"jsonrpc":"2.0","id":1,"result":"0x60806..."}
```

### eth_accounts

- [x] Ethereum JSON-RPC API
- [x] Canister need support
- [x] Already supported

```sh
curl https://eth-mainnet.alchemyapi.io/v2/demo \
-X POST -H 'content-Type: application/json' \
-d '{"jsonrpc":"2.0","id":1,"method":"eth_accounts","params":[]}'

{"jsonrpc":"2.0","id":1,"result":[]}%
```

## EVM/Smart Contract Execution

### eth_call
- [x] Ethereum JSON-RPC API
- [x] Canister need support
- [ ] Already supported

```sh
curl https://eth-mainnet.alchemyapi.io/v2/demo \
-X POST -H 'content-Type: application/json' \
-d '{"jsonrpc":"2.0","id":1,"method":"eth_call","params":[{"to":"0xebe8efa441b9302a0d7eaecc277c09d20d684540","data":"0x45848dfc"},"latest"]}'

{"jsonrpc":"2.0","id":1,"result":"0x"}%

# earliest, finalized, safe, latest, pending
# For simulated execution, will not join the blockchain
```

## Event Logs
### eth_getLogs
- [x] Ethereum JSON-RPC API
- [x] Canister need support
- [ ] Already supported

```sh
curl https://eth-mainnet.alchemyapi.io/v2/demo \
-X POST -H 'content-Type: application/json' \
-d '{"jsonrpc":"2.0","id":1,"method":"eth_getLogs","params":[{"topics":["0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef"]}]}'

# delete too many results
{"jsonrpc":"2.0","id":1,"result":[{"address":"0xdac17f958d2ee523a2206206994597c13d831ec7","topics":["0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef","0x00000000000000000000000049ee1ba0473e22e0d1b787f7fd063ce30a1db329","0x0000000000000000000000004914dcdf92360564da562917b6050d8385017a92"],"data":"0x0000000000000000000000000000000000000000000000000000000014dc9380","blockNumber":"0xedf416","transactionHash":"0xe560148552efd7ae1e24007728cedb772ce248dcf49405e36d6ec7ef905f4624","transactionIndex":"0xeb","blockHash":"0x4530424b6eedf77c89df9896328a351744773495e7bd5514e076dfa218b53ed9","logIndex":"0x206","removed":false},{"address":"0x4fabb145d64652a948d72533023f6e7a623c7c53","topics":["0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef","0x0000000000000000000000009bac26d2df7eb8532aea6bef5c8647cc66eb82f7","0x00000000000000000000000026ca0fea5c32ef8297c0c98d12fd3b41bbf51b03"],"data":"0x00000000000000000000000000000000000000000000032d18f077e463fc0000","blockNumber":"0xedf416","transactionHash":"0xe2ec96cc63501dadef9fc7dd0ed3ddadb3a014296a17f0358cc91d36083bb9fc","transactionIndex":"0xed","blockHash":"0x4530424b6eedf77c89df9896328a351744773495e7bd5514e076dfa218b53ed9","logIndex":"0x207","removed":false}]}%
```

### eth_getFilterChanges
- [x] Ethereum JSON-RPC API
- [x] Canister need support
- [ ] Already supported

```sh
curl https://eth-mainnet.alchemyapi.io/v2/demo \
-X POST -H 'content-Type: application/json' \
-d '{"jsonrpc":"2.0","id":1,"method":"eth_getFilterChanges","params":["0xf7474b25c6bf8c261693e41c83aa995c"]}'

{"jsonrpc":"2.0","id":1,"result":[]}%
```

### eth_getFilterLogs
- [x] Ethereum JSON-RPC API
- [x] Canister need support
- [ ] Already supported

```sh
curl https://eth-mainnet.alchemyapi.io/v2/demo \
-X POST -H 'content-Type: application/json' \
-d '{"jsonrpc":"2.0","id":1,"method":"eth_getFilterLogs","params":["0xf7474b25c6bf8c261693e41c83aa995c"]}'

{"jsonrpc":"2.0","id":1,"result":[]}%
```

### eth_newBlockFilter
- [x] Ethereum JSON-RPC API
- [x] Canister need support
- [ ] Already supported

```sh
curl https://eth-mainnet.alchemyapi.io/v2/demo \
-X POST -H 'content-Type: application/json' \
-d '{"jsonrpc":"2.0","id":1,"method":"eth_newBlockFilter","params":[]}'

{"jsonrpc":"2.0","id":1,"result":"0x81440fb6e65887432f4518d3c0eee8def5e5d7a2662d"}%
```

### eth_newFilter
- [x] Ethereum JSON-RPC API
- [x] Canister need support
- [ ] Already supported

```sh
curl https://eth-mainnet.alchemyapi.io/v2/demo \
-X POST -H 'content-Type: application/json' \
-d '{"jsonrpc":"2.0","id":1,"method":"eth_newFilter","params":[{"fromBlock": "latest","toBlock": "latest"}]}'

{"jsonrpc":"2.0","id":1,"result":"0x10ff0fb6e639fd2a9f59b83965f1b253a1e916d891a2"}%


curl https://eth-mainnet.alchemyapi.io/v2/demo \
-X POST -H 'content-Type: application/json' \
-d '{"jsonrpc":"2.0","id":1,"method":"eth_newFilter","params":[{"fromBlock": "0xEDF3D9","toBlock":"latest","address":"0x51115f73f927fcf13ef6f27f6baa170156d44489", "topics":["0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef"]}]}'

{"jsonrpc":"2.0","id":1,"result":"0xf7474b25c6bf8c261693e41c83aa995c"}%
```

### eth_newPendingTransactionFilter
- [x] Ethereum JSON-RPC API
- [x] Canister need support
- [ ] Already supported

```sh
curl https://eth-mainnet.alchemyapi.io/v2/demo \
-X POST -H 'content-Type: application/json' \
-d '{"jsonrpc":"2.0","id":1,"method":"eth_newPendingTransactionFilter","params":[]}'

{"jsonrpc":"2.0","id":1,"result":"0x79291219a056dcdd613148f7408099eb"}%
```

### eth_uninstallFilter
- [x] Ethereum JSON-RPC API
- [x] Canister need support
- [ ] Already supported

```sh
curl https://eth-mainnet.alchemyapi.io/v2/demo \
-X POST -H 'content-Type: application/json' \
-d '{"jsonrpc":"2.0","id":1,"method":"eth_uninstallFilter","params":["0x79291219a056dcdd613148f7408099eb"]}'

{"jsonrpc":"2.0","id":1,"result":true}%
```

## Chain Information

### eth_protocolVersion
- [x] Ethereum JSON-RPC API
- [x] Canister need support
- [x] Already supported

```sh
curl https://eth-mainnet.alchemyapi.io/v2/demo \
-X POST -H 'content-Type: application/json' \
-d '{"jsonrpc":"2.0","id":1,"method":"eth_protocolVersion","params":[]}'

{"jsonrpc":"2.0","id":1,"result":"0x41"}%
```

### eth_gasPrice
- [x] Ethereum JSON-RPC API
- [x] Canister need support
- [x] Already supported

```sh
curl https://eth-mainnet.alchemyapi.io/v2/demo \
-X POST -H 'content-Type: application/json' \
-d '{"jsonrpc":"2.0","id":1,"method":"eth_gasPrice","params":[]}'

{"jsonrpc":"2.0","id":1,"result":"0xf3668fd6"}%
```

### eth_estimateGas
- [x] Ethereum JSON-RPC API
- [x] Canister need support
- [x] Already supported

```sh
curl https://eth-mainnet.alchemyapi.io/v2/demo \
-X POST -H 'content-Type: application/json' \
-d '{"jsonrpc":"2.0","id":1,"method":"eth_estimateGas","params":[{"from": "0x8D97689C9818892B700e27F316cc3E41e17fBeb9","to": "0xd3CdA913deB6f67967B99D67aCDFa1712C293601","value": "0x186a0"}]}'

{"jsonrpc":"2.0","id":1,"result":"0x5208"}%  # 21000
```

### eth_chainId
- [x] **Not** Ethereum JSON-RPC API
- [x] Canister need support
- [x] Already supported

```sh
curl https://eth-testnet.alchemyapi.io/v2/demo \
-X POST -H 'content-Type: application/json' \
-d '{"jsonrpc":"2.0","id":1,"method":"eth_chainId","params":[]}'

{"jsonrpc":"2.0","id":1,"result":"0x1"}%
```

### net_version
- [x] Ethereum JSON-RPC API
- [x] Canister need support
- [x] Already supported

```sh
curl https://eth-mainnet.alchemyapi.io/v2/demo \
-X POST -H 'content-Type: application/json' \
-d '{"jsonrpc":"2.0","id":1,"method":"net_version","params":[]}'

{"jsonrpc":"2.0","id":1,"result":"1"}%
```

### net_listening
- [x] Ethereum JSON-RPC API
- [x] Canister need support
- [x] Already supported

```sh
curl https://eth-mainnet.alchemyapi.io/v2/demo \
-X POST -H 'content-Type: application/json' \
-d '{"jsonrpc":"2.0","id":1,"method":"net_listening","params":[]}'

{"jsonrpc":"2.0","id":1,"result":true}%
```

## Getting Uncles

### eth_getUncleByBlockHashAndIndex
- [x] Ethereum JSON-RPC API
- [x] Canister need support
- [ ] Already supported

```sh
curl https://eth-mainnet.alchemyapi.io/v2/demo \
-X POST -H 'content-Type: application/json' \
-d '{"jsonrpc":"2.0","id":1,"method":"eth_getUncleByBlockHashAndIndex","params":["0xf3cd331b8af49500e288470107d90a2f01712879f50754f7c993f99b39299321", "0x0"]}'

{"jsonrpc":"2.0","id":1,"result":null}%
```

### eth_getUncleByBlockNumberAndIndex
- [x] Ethereum JSON-RPC API
- [x] Canister need support
- [ ] Already supported

```sh
curl https://eth-mainnet.alchemyapi.io/v2/demo \
-X POST -H 'content-Type: application/json' \
-d '{"jsonrpc":"2.0","id":1,"method":"eth_getUncleByBlockNumberAndIndex","params":["0xEDD8C4", "0x0"]}'

{"jsonrpc":"2.0","id":1,"result":null}%
```

### eth_getUncleCountByBlockHash
- [x] Ethereum JSON-RPC API
- [x] Canister need support
- [ ] Already supported

```sh
curl https://eth-mainnet.alchemyapi.io/v2/demo \
-X POST -H 'content-Type: application/json' \
-d '{"jsonrpc":"2.0","id":1,"method":"eth_getUncleCountByBlockHash","params":["0x9a834c53bbee9c2665a5a84789a1d1ad73750b2d77b50de44f457f411d02e52e"]}'

{"jsonrpc":"2.0","id":1,"result":"0x0"}%

# {"jsonrpc":"2.0","id":1,"error":{"code":-32602,"message":"too many arguments, want at most 1"}}%
```

### eth_getUncleCountByBlockNumber
- [x] Ethereum JSON-RPC API
- [x] Canister need support
- [ ] Already supported

```sh
curl https://eth-mainnet.alchemyapi.io/v2/demo \
-X POST -H 'content-Type: application/json' \
-d '{"jsonrpc":"2.0","id":1,"method":"eth_getUncleCountByBlockNumber","params":["0x100000"]}'

{"jsonrpc":"2.0","id":1,"result":"0x0"}%
```

## Web3
### web3_clientVersion
- [x] Ethereum JSON-RPC API
- [x] Canister need support
- [x] Already supported

```sh
curl https://eth-mainnet.alchemyapi.io/v2/demo \
-X POST -H 'content-Type: application/json' \
-d '{"jsonrpc":"2.0","id":1,"method":"web3_clientVersion","params":[]}'

{"jsonrpc":"2.0","id":1,"result":"Geth/v1.10.23-stable-d901d853/linux-amd64/go1.18.5"}%


curl https://mainnet.infura.io/v3/65b956e7a8c245f08b2809f6e91f3181 \
-X POST -H 'content-Type: application/json' \
-d '{"jsonrpc":"2.0","id":1,"method":"web3_clientVersion","params":[]}'

{"jsonrpc":"2.0","id":1,"result":"Geth/v1.10.23-omnibus-b38477ec/linux-amd64/go1.18.5"}%
```

### web3_sha3
- [x] Ethereum JSON-RPC API
- [x] Canister need support
- [x] Already supported

```sh
curl https://eth-mainnet.alchemyapi.io/v2/demo \
-X POST -H 'content-Type: application/json' \
-d '{"jsonrpc":"2.0","id":1,"method":"web3_sha3","params":["0x68656c6c6f20776f726c64"]}'

{"jsonrpc":"2.0","id":1,"result":"0x47173285a8d7341e5e972fc677286384f802f8ef42a5ec5f03bbfa254cb01fad"}%
```

## others
### eth_syncing
- [x] Ethereum JSON-RPC API
- [x] Canister need support
- [x] Already supported

```sh
curl https://eth-mainnet.alchemyapi.io/v2/demo \
-X POST -H 'content-Type: application/json' \
-d '{"jsonrpc":"2.0","id":1,"method":"eth_syncing","params":[]}'

{"jsonrpc":"2.0","id":1,"result":false}%
```

### eth_coinbase
- [x] Ethereum JSON-RPC API
- [x] Canister **needn't** support

```sh
curl https://eth-mainnet.alchemyapi.io/v2/demo \
-X POST -H 'content-Type: application/json' \
-d '{"jsonrpc":"2.0","id":1,"method":"eth_coinbase","params":[]}'

{"jsonrpc":"2.0","id":1,"error":{"code":-32600,"message":"Unsupported method: eth_coinbase. Alchemy does not support mining eth. See available methods at https://docs.alchemy.com/alchemy/documentation/apis"}}%

curl https://mainnet.infura.io/v3/65b956e7a8c245f08b2809f6e91f3181 \
-X POST -H 'content-Type: application/json' \
-d '{"jsonrpc":"2.0","id":1,"method":"eth_coinbase","params":[]}'
{"jsonrpc":"2.0","id":1,"error":{"code":-32601,"message":"The method eth_coinbase does not exist/is not available"}}%
```

### net_peerCount
- [x] Ethereum JSON-RPC API
- [x] Canister need support
- [x] Already supported

```sh
curl https://eth-mainnet.alchemyapi.io/v2/demo \
-X POST -H 'content-Type: application/json' \
-d '{"jsonrpc":"2.0","id":1,"method":"net_peerCount","params":[]}'

{"jsonrpc":"2.0","id":1,"error":{"code":-32600,"message":"Unsupported method: net_peerCount. See available methods at https://docs.alchemy.com/alchemy/documentation/apis"}}%

curl https://mainnet.infura.io/v3/65b956e7a8c245f08b2809f6e91f3181 \
-X POST -H 'content-Type: application/json' \
-d '{"jsonrpc":"2.0","id":1,"method":"net_peerCount","params":[]}'

{"jsonrpc":"2.0","id":1,"result":"0x64"}%
```

### eth_mining
- [x] Ethereum JSON-RPC API
- [x] Canister need support
- [x] Already supported

```sh
curl https://mainnet.infura.io/v3/65b956e7a8c245f08b2809f6e91f3181 \
-X POST -H 'content-Type: application/json' \
-d '{"jsonrpc":"2.0","id":1,"method":"eth_mining","params":[]}'

{"jsonrpc":"2.0","id":1,"result":false}%
```

### eth_hashrate
- [x] Ethereum JSON-RPC API
- [x] Canister need support
- [x] Already supported

```sh
curl https://mainnet.infura.io/v3/65b956e7a8c245f08b2809f6e91f3181 \
-X POST -H 'content-Type: application/json' \
-d '{"jsonrpc":"2.0","id":1,"method":"eth_hashrate","params":[]}'

{"jsonrpc":"2.0","id":1,"result":"0x0"}%
```

### eth_sign
- [x] Ethereum JSON-RPC API
- [x] Canister **needn't** support

```sh
curl https://mainnet.infura.io/v3/65b956e7a8c245f08b2809f6e91f3181 \
-X POST -H 'content-Type: application/json' \
-d '{"jsonrpc":"2.0","id":1,"method":"eth_sign","params":["0x9b2055d370f73ec7d8a03e965129118dc8f5bf83", "0xdeadbeaf"]}'

{"jsonrpc":"2.0","id":1,"error":{"code":-32601,"message":"The method eth_sign does not exist/is not available"}}%
```

### eth_signTransaction
- [x] Ethereum JSON-RPC API
- [x] Canister **needn't** support

```sh
curl https://mainnet.infura.io/v3/65b956e7a8c245f08b2809f6e91f3181 \
-X POST -H 'content-Type: application/json' \
-d '{"jsonrpc":"2.0","id":1,"method":"eth_signTransaction","params":[{"data":"0xd46e8dd67c5d32be8d46e8dd67c5d32be8058bb8eb970870f072445675058bb8eb970870f072445675","from": "0xb60e8dd61c5d32be8058bb8eb970870f07233155","gas": "0x76c0","gasPrice": "0x9184e72a000","to": "0xd46e8dd67c5d32be8058bb8eb970870f07244567","value": "0x9184e72a"}]}'

{"jsonrpc":"2.0","id":1,"error":{"code":-32601,"message":"The method eth_signTransaction does not exist/is not available"}}%
```

### eth_sendTransaction
- [x] Ethereum JSON-RPC API
- [x] Canister **needn't** support

```sh
curl https://mainnet.infura.io/v3/65b956e7a8c245f08b2809f6e91f3181 \
-X POST -H 'content-Type: application/json' \
-d '{"jsonrpc":"2.0","id":1,"method":"eth_sendTransaction","params":[{"from":"0xb60e8dd61c5d32be8058bb8eb970870f07233155","to": "0xd46e8dd67c5d32be8058bb8eb970870f07244567","gas": "0x76c0","gasPrice": "0x9184e72a000","value": "0x9184e72a","data":"0xd46e8dd67c5d32be8d46e8dd67c5d32be8058bb8eb970870f072445675058bb8eb970870f072445675"}]}'

{"jsonrpc":"2.0","id":1,"error":{"code":-32601,"message":"The method eth_sign does not exist/is not available"}}%
```

### eth_getCompilers
- [x] Ethereum JSON-RPC API
- [x] Canister **needn't** support

```sh
curl https://mainnet.infura.io/v3/65b956e7a8c245f08b2809f6e91f3181 \
-X POST -H 'content-Type: application/json' \
-d '{"jsonrpc":"2.0","id":1,"method":"eth_getCompilers","params":[]}'

{"jsonrpc":"2.0","id":1,"error":{"code":-32601,"message":"the method eth_getCompilers does not exist/is not available"}}%
```

### eth_compileSolidity
- [x] Ethereum JSON-RPC API
- [x] Canister **needn't** support

```sh
curl https://mainnet.infura.io/v3/65b956e7a8c245f08b2809f6e91f3181 \
-X POST -H 'content-Type: application/json' \
-d '{"jsonrpc":"2.0","id":1,"method":"eth_compileSolidity","params":["contract test { function multiply(uint a) returns(uint d) {   return a * 7;   } }"]}'

{"jsonrpc":"2.0","id":1,"error":{"code":-32601,"message":"the method eth_compileSolidity does not exist/is not available"}}%
```

### eth_compileLLL
- [x] Ethereum JSON-RPC API
- [x] Canister **needn't** support

```sh
curl https://mainnet.infura.io/v3/65b956e7a8c245f08b2809f6e91f3181 \
-X POST -H 'content-Type: application/json' \
-d '{"jsonrpc":"2.0","id":1,"method":"eth_compileLLL","params":["(returnlll (suicide (caller)))"]}'

{"jsonrpc":"2.0","id":1,"error":{"code":-32601,"message":"the method eth_compileLLL does not exist/is not available"}}%
```

### eth_compileSerpent
- [x] Ethereum JSON-RPC API
- [x] Canister **needn't** support

```sh
curl https://mainnet.infura.io/v3/65b956e7a8c245f08b2809f6e91f3181 \
-X POST -H 'content-Type: application/json' \
-d '{"jsonrpc":"2.0","id":1,"method":"eth_compileSerpent","params":["/* some serpent */"]}'

{"jsonrpc":"2.0","id":1,"error":{"code":-32601,"message":"the method eth_compileSerpent does not exist/is not available"}}%
```

### eth_getWork
- [x] Ethereum JSON-RPC API
- [x] Canister **needn't** support

```sh
curl https://mainnet.infura.io/v3/65b956e7a8c245f08b2809f6e91f3181 \
-X POST -H 'content-Type: application/json' \
-d '{"jsonrpc":"2.0","id":1,"method":"eth_getWork","params":[]}'

{"jsonrpc":"2.0","id":1,"error":{"code":-32000,"message":"no mining work available yet"}}%
```


### eth_submitWork
- [x] Ethereum JSON-RPC API
- [x] Canister **needn't** support

```sh
curl https://mainnet.infura.io/v3/65b956e7a8c245f08b2809f6e91f3181 \
-X POST -H 'content-Type: application/json' \
-d '{"jsonrpc":"2.0","id":1,"method":"eth_submitWork","params":["0x0000000000000001","0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef","0xD1FE5700000000000000000000000000D1FE5700000000000000000000000000"]}'

{"jsonrpc":"2.0","id":1,"result":false}%
```

### eth_submitHashrate
- [x] Ethereum JSON-RPC API
- [x] Canister **needn't** support

```sh
curl https://mainnet.infura.io/v3/65b956e7a8c245f08b2809f6e91f3181 \
-X POST -H 'content-Type: application/json' \
-d '{"jsonrpc":"2.0","id":1,"method":"eth_submitHashrate","params":["0x500000","0x59daa26581d0acd1fce254fb7e85952f4c09d0915afd33d3886cd914bc7d283c"]}'

{"jsonrpc":"2.0","id":1,"result":true}%
```



# Reference
* https://ethereum.org/en/developers/docs/apis/json-rpc/#eth_getstorageat
* https://docs.alchemy.com/reference/eth-getblockbynumber-astar
