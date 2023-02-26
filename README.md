# A Migratable SNIP-721 Implementation


## snip721-migratable:
A snip721 implementation which can be migrated. It can be used on its own and managed by a user's wallet or it can be instantiated by the example snip721-dealer or some other contract.

## snip721-dealer:
A minimal contract which allows anyone to buy a mint of an nft with fixed metadata. It instantiates its own `snip721-migratable` instance. And registers itself as a minter

Both contracts can be migrated without breaking their references to each other. When `snip721-dealer` migrates it will automatically notify its `snip721-migratable` instance to update its minters list.
And when `snip721-migratable` migrates it automatically notifies the dealer to update its reference to the new `snip721-migratable address`
