package io.eqoty.dapp.secret.types

import io.eqoty.dapp.secret.types.contract.EqotyRoyaltyMsgs
import io.eqoty.dapp.secret.types.contract.Snip721Msgs
import io.eqoty.secretk.types.Coin

data class MintedRelease(
    val totalShares: UInt,
    val purchasePrices: List<Coin>,
    val recipients: List<EqotyRoyaltyMsgs.TokenRecipient>,
    val royaltyContractInfo: ContractInfo,
    val purchaseContractInfo: ContractInfo,
    val mintNft: Snip721Msgs.ExecuteAnswer.MintNft
)
