import { useMutation, useQueryClient } from "@tanstack/react-query";
import {
	appendTransactionMessageInstruction,
	assertIsSendableTransaction,
	createSignerFromKeyPair,
	createTransactionMessage,
	getAddressEncoder,
	getBase64EncodedWireTransaction,
	getBytesEncoder,
	getProgramDerivedAddress,
	pipe,
	setTransactionMessageFeePayerSigner,
	setTransactionMessageLifetimeUsingBlockhash,
	signTransactionMessageWithSigners,
	type Address,
	type KeyPairSigner,
} from "@solana/kit";
import { getTxClient } from "../solana/tx-client";
import { getBuyVirtualTokenInstructionAsync, CPMM_POC_PROGRAM_ADDRESS } from "@bcpmm/js-client";
import { getInitializeVirtualTokenAccountInstructionAsync } from "@bcpmm/js-client";

interface BuyVirtualTokenParams {
	user: KeyPairSigner;
	pool: Address;
	aMint: Address; // Mint A being paid
	aAmount: number | bigint; // amount of A (with decimals)
	bAmountMin: number | bigint; // slippage floor for B (with decimals)
}

export function useBuyVirtualToken() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: async ({ user, pool, aMint, aAmount, bAmountMin }: BuyVirtualTokenParams) => {
			const { rpc, sendAndConfirmTransaction } = await getTxClient();
			const payerSigner = await createSignerFromKeyPair(user.keyPair);

			// Get virtual token account address
			const [virtualTokenAccountAddress] = await getProgramDerivedAddress({
				programAddress: CPMM_POC_PROGRAM_ADDRESS,
				seeds: [
					getBytesEncoder().encode(
						new Uint8Array([
							118, 105, 114, 116, 117, 97, 108, 95, 116, 111, 107, 101, 110, 95,
							97, 99, 99, 111, 117, 110, 116, // "virtual_token_account"
						])
					),
					getAddressEncoder().encode(pool),
					getAddressEncoder().encode(payerSigner.address),
				],
			});

			// Ensure virtual token account exists; initialize if missing
			const vtaAccount = await rpc.getAccountInfo(virtualTokenAccountAddress, { commitment: 'confirmed', encoding: 'base64' }).send();
			let initializeVirtualTokenAccountInstruction: any | null = null;
			if (!vtaAccount.value) {
				initializeVirtualTokenAccountInstruction = await getInitializeVirtualTokenAccountInstructionAsync({
					payer: payerSigner,
					owner: payerSigner.address,
					virtualTokenAccount: virtualTokenAccountAddress,
					pool,
				});
			}

			// Build instruction (async variant derives PDAs for ATAs, VT account, central state, etc.)
			const instruction = await getBuyVirtualTokenInstructionAsync({
				payer: payerSigner,
				pool,
				aMint,
				aAmount,
				bAmountMin,
			});

			const { value: latestBlockhash } = await rpc.getLatestBlockhash().send();
			const transactionMessage = pipe(
				createTransactionMessage({ version: 0 }),
				(tx) => setTransactionMessageFeePayerSigner(payerSigner, tx),
				(tx) => setTransactionMessageLifetimeUsingBlockhash(latestBlockhash, tx),
				(tx) => (initializeVirtualTokenAccountInstruction ? appendTransactionMessageInstruction(initializeVirtualTokenAccountInstruction, tx) : tx),
				(tx) => appendTransactionMessageInstruction(instruction, tx),
			);

			const signedTx = await signTransactionMessageWithSigners(transactionMessage);
			assertIsSendableTransaction(signedTx);

			try {
				const txBase64 = getBase64EncodedWireTransaction(signedTx);
				const simulateResult = await rpc
					.simulateTransaction(txBase64, { encoding: 'base64', sigVerify: true, commitment: 'confirmed' })
					.send();
				console.log('txBase64', txBase64);
				console.log('simulate buyVirtualToken', simulateResult);

				await sendAndConfirmTransaction(signedTx as any, { commitment: 'confirmed' });
				console.log('buyVirtualToken tx sent and confirmed');
			} catch (error) {
				console.error('buyVirtualToken tx error', error);
				throw error;
			}
		},
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ['allPools'] });
			queryClient.invalidateQueries({ queryKey: ['userPool'] });
      queryClient.invalidateQueries({ queryKey: ['tokenBalance'] });
			queryClient.invalidateQueries({ queryKey: ['virtualTokenBalance'] });
		},
	});
}
