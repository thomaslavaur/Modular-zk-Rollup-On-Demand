# Modular zk-Rollup On-Demand

**Disclaimer**: This code is **NOT** intended for a production environment and was developed for research and measurement purposes only.
This code is a fork of the [ZkSync zk-rollup v1](https://github.com/matter-labs/zksync). 
We have maintained references to zkSync in the code but we are not affiliated with the company in any way.

## Aim and Motivation

This code is a proof of concept to measure and demonstrate our proposal from the article [Modular zk-Rollup On-Demand](To-be-include).
It aims to implement and evaluate the addition of partitioning within smart contracts supporting multiple groups.
Each group can thus become a zk-rollup in its own right but share smart contracts and pending balance with all the others.
This reduces the cost of creating a group and increases the cost of transactions only slightly.
At the same time, we have added a new type of transaction that allows funds to be transferred from one group to another without returning to the blockchain entirely (only returning to the smart contract).
This new transaction type also greatly reduces the cost of sending funds from one zk-rollup to another by avoiding the vulnerabilities often introduced by bridge solutions.
[All measurements are available in the docs section.](docs/Our%20Measures)

## Limitations

In order to demonstrate our proposal, many aspects have been given little or no attention.
We have not dealt with the problem of governance which no longer works in the current system.
Thus, each group is associated with a single validator and cannot assign new ones.
All tests are also deprecated and either no longer work or do not work properly.


## License

We reused the same license as zkSync v1 which is distributed under the terms of both the MIT license and the Apache License (Version 2.0).

See [LICENSE-APACHE](LICENSE-APACHE), [LICENSE-MIT](LICENSE-MIT) for details.


