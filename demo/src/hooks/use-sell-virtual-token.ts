import { useMutation, useQueryClient } from "@tanstack/react-query";
import {
	appendTransactionMessageInstruction,
	assertIsSendableTransaction,
	createSignerFromKeyPair,
	createTransactionMessage,
	getBase64EncodedWireTransaction,
	pipe,
	setTransactionMessageFeePayerSigner,
	setTransactionMessageLifetimeUsingBlockhash,
	signTransactionMessageWithSigners,
	type Address,
	type KeyPairSigner,
} from "@solana/kit";
import { getTxClient } from "../solana/tx-client";
import { getSellVirtualTokenInstructionAsync } from "@cbmm/js-client";

interface SellVirtualTokenParams {
	user: KeyPairSigner;
	pool: Address;
	aMint: Address; // Mint A of the pool
	bAmount: number | bigint; // amount of B (with decimals)
}

export function useSellVirtualToken() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: async ({ user, pool, aMint, bAmount }: SellVirtualTokenParams) => {
			const { rpc, sendAndConfirmTransaction } = await getTxClient();
			const payerSigner = await createSignerFromKeyPair(user.keyPair);

			const instruction = await getSellVirtualTokenInstructionAsync({
				payer: payerSigner,
				pool,
				aMint,
				bAmount,
			});

			const { value: latestBlockhash } = await rpc.getLatestBlockhash().send();
			const transactionMessage = pipe(
				createTransactionMessage({ version: 0 }),
				(tx) => setTransactionMessageFeePayerSigner(payerSigner, tx),
				(tx) => setTransactionMessageLifetimeUsingBlockhash(latestBlockhash, tx),
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
				console.log('simulate sellVirtualToken', simulateResult);

				await sendAndConfirmTransaction(signedTx as any, { commitment: 'confirmed' });
				console.log('sellVirtualToken tx sent and confirmed');
			} catch (error) {
				console.error('sellVirtualToken tx error', error);
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
