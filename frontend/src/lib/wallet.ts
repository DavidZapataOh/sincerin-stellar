/*
 * Wallet connection via Stellar Wallets Kit (Freighter on testnet).
 *
 * The judge's connected G-address is the withdrawal recipient. We construct the
 * kit lazily (it touches the DOM / browser globals) and expose a tiny async API
 * the React layer can await.
 */

import type {
  StellarWalletsKit,
  ISupportedWallet,
} from "@creit.tech/stellar-wallets-kit";

let kit: StellarWalletsKit | null = null;

/**
 * Lazily import + construct the kit. Dynamic import keeps the wallet bundle
 * (WalletConnect, etc.) out of the initial paint — the hero renders without it.
 */
async function getKit(): Promise<StellarWalletsKit> {
  if (!kit) {
    const { StellarWalletsKit, WalletNetwork, FreighterModule, FREIGHTER_ID } =
      await import("@creit.tech/stellar-wallets-kit");
    kit = new StellarWalletsKit({
      network: WalletNetwork.TESTNET,
      selectedWalletId: FREIGHTER_ID,
      modules: [new FreighterModule()],
    });
  }
  return kit;
}

/**
 * Open the wallet picker and resolve to the selected account's G-address.
 * Resolves `null` if the user closes the modal without choosing a wallet.
 */
export async function connectWallet(): Promise<string | null> {
  const k = await getKit();
  return new Promise<string | null>((resolve, reject) => {
    k.openModal({
      modalTitle: "Connect a testnet wallet",
      onWalletSelected: async (wallet: ISupportedWallet) => {
        try {
          k.setWallet(wallet.id);
          const { address } = await k.getAddress();
          resolve(address);
        } catch (e) {
          reject(
            e instanceof Error
              ? e
              : new Error("Could not read the wallet address."),
          );
        }
      },
      onClosed: () => resolve(null),
    });
  });
}

export async function disconnectWallet(): Promise<void> {
  if (kit) await kit.disconnect();
}
