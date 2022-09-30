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

## Conventions

There are no strict conventions, the only recommendation is to write test functions with "snake_case" naming sense (Only for the function name)
It is very important for the code to be clear and verbose in its outputs also as for the test functions to be self-explanatory.
