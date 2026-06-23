/*
 * Real testnet artifacts for the LANDING surface — every value here is a live,
 * verifiable on-chain id from deployments/testnet.json (the historic N=8
 * settle). Nothing fabricated; these are the links the landing cites. The DEMO
 * surface never reads this file — it gets its contract ids from /config.
 */

export const EVIDENCE = {
  /** Historic N=8 settle: 8 withdrawals aggregated into 1 tx · SUCCESS. */
  settleTx:
    "aedc1cc42f112d65913d4b1b5fb0e9b5636481e2f10a86f85ed21f5c0f605ea9",
  /** Deployed RISC Zero / Groth16-BN254 verifier contract. */
  verifier: "CBQFQLSBYXUYLD2Q5EWHVNNI6VO33NAVRDUDIGJNMC5TUAINK5BXO2LJ",
  /** Rollup contract that settled the historic N=8 batch. */
  rollup: "CCGUQKT4CWEZBVECATHLZJRUELXNRUATHAXUUTPFIW4GMKRBQ4K36HF5",
  /** Privacy pool contract for that batch. */
  pool: "CCE4URVAZ5HS7MBL5QMFQXQ6GV4TFQXARFFXZQENFNVQFNAY2FVI2DL6",
  /** Guest image id pinned by the deployed rollup — binds the proof. */
  imageId:
    "cbeab7aa6ce69944e10cca8c7ed94d15aae297f2580752f07a15c6cab6ba0d46",
} as const;

export const EXPLORER = {
  network: "testnet",
  base: "https://stellar.expert/explorer/testnet",
  repo: "https://github.com/DavidZapataOh/sincerin-stellar",
} as const;

export function txUrl(hash: string): string {
  return `${EXPLORER.base}/tx/${hash}`;
}

export function contractUrl(id: string): string {
  return `${EXPLORER.base}/contract/${id}`;
}
