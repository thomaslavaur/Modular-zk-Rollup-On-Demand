import '@nomiclabs/hardhat-waffle';
import '@nomiclabs/hardhat-solpp';
import '@nomiclabs/hardhat-etherscan';
import 'hardhat-typechain';
import 'hardhat-contract-sizer';

const prodConfig = {
    // UPGRADE_NOTICE_PERIOD: 0,
    MAX_AMOUNT_OF_REGISTERED_TOKENS: 1023,
    // PRIORITY_EXPIRATION: 101,
    DUMMY_VERIFIER: false,
    ZKSYNC_ADDRESS: process.env.CONTRACTS_CONTRACT_ADDR,
    NEW_ADDITIONAL_ZKSYNC_ADDRESS: process.env.CONTRACTS_ADDITIONAL_ZKSYNC_ADDR,
    UPGRADE_GATEKEEPER_ADDRESS: process.env.CONTRACTS_UPGRADE_GATEKEEPER_ADDR,

    SECURITY_COUNCIL_MEMBERS_NUMBER: process.env.MISC_SECURITY_COUNCIL_MEMBERS_NUMBER,
    SECURITY_COUNCIL_MEMBERS: process.env.MISC_SECURITY_COUNCIL_MEMBERS,
    SECURITY_COUNCIL_THRESHOLD: process.env.MISC_SECURITY_COUNCIL_THRESHOLD
};

const testnetConfig = {
    UPGRADE_NOTICE_PERIOD: 0,
    MAX_AMOUNT_OF_REGISTERED_TOKENS: 1023,
    // PRIORITY_EXPIRATION: 101,
    DUMMY_VERIFIER: false,
    ZKSYNC_ADDRESS: process.env.CONTRACTS_CONTRACT_ADDR,
    NEW_ADDITIONAL_ZKSYNC_ADDRESS: process.env.CONTRACTS_ADDITIONAL_ZKSYNC_ADDR,
    UPGRADE_GATEKEEPER_ADDRESS: process.env.CONTRACTS_UPGRADE_GATEKEEPER_ADDR,

    SECURITY_COUNCIL_MEMBERS_NUMBER: process.env.MISC_SECURITY_COUNCIL_MEMBERS_NUMBER,
    SECURITY_COUNCIL_MEMBERS: process.env.MISC_SECURITY_COUNCIL_MEMBERS,
    SECURITY_COUNCIL_THRESHOLD: process.env.MISC_SECURITY_COUNCIL_THRESHOLD
};

const testConfig = {
    UPGRADE_NOTICE_PERIOD: 0,
    MAX_AMOUNT_OF_REGISTERED_TOKENS: 5,
    PRIORITY_EXPIRATION: 101,
    DUMMY_VERIFIER: true,
    ZKSYNC_ADDRESS: process.env.CONTRACTS_CONTRACT_ADDR,
    NEW_ADDITIONAL_ZKSYNC_ADDRESS: process.env.CONTRACTS_ADDITIONAL_ZKSYNC_ADDR,
    UPGRADE_GATEKEEPER_ADDRESS: process.env.CONTRACTS_UPGRADE_GATEKEEPER_ADDR,

    SECURITY_COUNCIL_MEMBERS_NUMBER: '3',
    // First 3 accounts obtained from `$ZKSYNC_HOME/etc/test_config/constant/test_mnemonic.json` mnemonic
    SECURITY_COUNCIL_MEMBERS:
        '0x36615Cf349d7F6344891B1e7CA7C72883F5dc049,0xa61464658AfeAf65CccaaFD3a512b69A83B77618,0x0D43eB5B8a47bA8900d84AA36656c92024e9772e',
    SECURITY_COUNCIL_THRESHOLD: '2'
};

const localConfig = Object.assign({}, prodConfig);
// @ts-ignore
localConfig.UPGRADE_NOTICE_PERIOD = 0;
localConfig.DUMMY_VERIFIER = process.env.CONTRACTS_TEST_DUMMY_VERIFIER === 'true';
// @ts-ignore
localConfig.NEW_ADDITIONAL_ZKSYNC_ADDRESS = process.env.CONTRACTS_ADDITIONAL_ZKSYNC_ADDR;

localConfig.SECURITY_COUNCIL_MEMBERS_NUMBER = process.env.MISC_SECURITY_COUNCIL_MEMBERS_NUMBER;
localConfig.SECURITY_COUNCIL_MEMBERS = process.env.MISC_SECURITY_COUNCIL_MEMBERS;
localConfig.SECURITY_COUNCIL_THRESHOLD = process.env.MISC_SECURITY_COUNCIL_THRESHOLD;

// @ts-ignore
localConfig.EASY_EXODUS = process.env.CONTRACTS_TEST_EASY_EXODUS === 'true';

const contractDefs = {
    goerli: testnetConfig,
    rinkeby: testnetConfig,
    ropsten: testnetConfig,
    mainnet: prodConfig,
    test: testConfig,
    localhost: localConfig
};

export default {
    solidity: {
        version: '0.7.6',
        settings: {
            optimizer: {
                enabled: true,
                runs: 150
            },
            outputSelection: {
                '*': {
                    '*': ['storageLayout']
                }
            }
        }
    },
    contractSizer: {
        runOnCompile: false
    },
    paths: {
        sources: './contracts'
    },
    solpp: {
        defs: (() => {
            if (process.env.CONTRACT_TESTS) {
                return contractDefs.test;
            }
            return contractDefs[process.env.CHAIN_ETH_NETWORK];
        })()
    },
    networks: {
        env: {
            //url: process.env.ETH_CLIENT_WEB3_URL?.split(',')[0]
            url: 'http://10.20.3.15:8545'
        },
        hardhat: {
            allowUnlimitedContractSize: true,
            /*forking: {
                url: 'https://eth-mainnet.alchemyapi.io/v2/' + process.env.ALCHEMY_KEY,
                enabled: process.env.TEST_CONTRACTS_FORK === '1'
            },*/
            chainId: 9,
            accounts: [
                {
                    privateKey: '0x27593fea79697e947890ecbecce7901b0008345e5d7259710d0dd5e500d040be',
                    balance: '100000000000000000000000000000000000000'
                },
                {
                    privateKey: '0x03c807e375d9a70fb5f21984496e018baed148dad00829b58d7ca9e557f2998c',
                    balance: '100000000000000000000000000000000000000'
                },
                {
                    privateKey: '0x0559b9f000b4e4bbb7fe02e1374cef9623c2ab7c3791204b490e1f229191d104',
                    balance: '100000000000000000000000000000000000000'
                },
                {
                    privateKey: '0xe131bc3f481277a8f73d680d9ba404cc6f959e64296e0914dded403030d4f705',
                    balance: '100000000000000000000000000000000000000'
                },
                {
                    privateKey: '0x7726827caac94a7f9e1b160f7ea819f172f7b6f9d2a97f992c38edeab82d4110',
                    balance: '100000000000000000000000000000000000000'
                }
            ]
        }
    },
    etherscan: {
        apiKey: process.env.MISC_ETHERSCAN_API_KEY
    }
};
