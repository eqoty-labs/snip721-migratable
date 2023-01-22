# Secret Contract Integration Tests

This is an example of how to write integration tests in Kotlin using secretk
The goal of integration tests is to test our contract on-chain, to test the integration between multiple transactions / queries and even to check the integration between multiple contracts.

In order to run the tests for all multiplatform targets run:
```sh
./gradlew check
```

In order to run the tests for jvm only run:
```sh
./gradlew jvmTest
```

## Setting Testnet Endpoints
Testnet endpoints can be configured in `tests/src/commonTest/resources/config/testnets.json`

Choosing a testnet type is done through environment variables.
The default testnet type is set in `gradle.properties`
Creating a `local.properties` file and setting `NODE_TYPE` will override `gradle.properties`
By default `NODE_TYPE=Pulsar2` is set in `gradle.properties`

For CI we override this to use `NODE_TYPE=LocalSecret`

To use gitpod, in `local.properties` set for example:

```
NODE_TYPE=Gitpod
GITPOD_ID=eqotylabs-gitpodlocalse-mztv8v7iwww.ws-us69
```

## Conventions

There are no strict conventions, the only recommendation is to write test functions with "snake_case" naming sense (Only for the function name)
It is very important for the code to be clear and verbose in its outputs also as for the test functions to be self-explanatory.
