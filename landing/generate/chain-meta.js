// Display-only details for the chain picker. Chain ids and names still come from the engine.

export const POPULAR = ["sol", "evm", "btc", "ton", "trx", "sui"];

export const GROUPS = [
  { title: "Bitcoin family", ids: ["btc", "btc-segwit", "btc-taproot", "ltc", "doge", "dash"] },
  { title: "EVM and Move", ids: ["evm", "aptos", "sui"] },
  { title: "Cosmos", ids: ["cosmos", "tia", "inj", "sei", "dydx", "osmo"] },
  { title: "Polkadot", ids: ["dot", "ksm"] },
];
export const OTHER_TITLE = "More networks";

// ticker, brand colour, and extra words people search for
export const META = {
  algo: { ticker: "ALGO", color: "#9aa3ad", words: "algorand" },
  aptos: { ticker: "APT", color: "#3dd6c6", words: "aptos move" },
  btc: { ticker: "BTC", color: "#f7931a", words: "bitcoin legacy p2pkh 1" },
  "btc-segwit": { ticker: "BTC", color: "#f7931a", words: "bitcoin segwit bc1q native" },
  "btc-taproot": { ticker: "BTC", color: "#f7931a", words: "bitcoin taproot bc1p" },
  ada: { ticker: "ADA", color: "#2f6bff", words: "cardano addr1" },
  tia: { ticker: "TIA", color: "#8b46ff", words: "celestia" },
  cosmos: { ticker: "ATOM", color: "#8c8fb0", words: "cosmos hub atom" },
  dash: { ticker: "DASH", color: "#1c8de3", words: "dash" },
  doge: { ticker: "DOGE", color: "#c9a93a", words: "dogecoin" },
  dydx: { ticker: "DYDX", color: "#7a77ff", words: "dydx" },
  evm: {
    ticker: "ETH",
    color: "#7b8ff0",
    words:
      "ethereum metamask base arbitrum optimism polygon bnb bsc avalanche hyperliquid sonic robinhood linea scroll zksync blast monad berachain",
  },
  fil: { ticker: "FIL", color: "#1a9bff", words: "filecoin" },
  hedera: { ticker: "HBAR", color: "#9a8cff", words: "hedera hashgraph" },
  inj: { ticker: "INJ", color: "#1d8cff", words: "injective" },
  icp: { ticker: "ICP", color: "#29abe2", words: "internet computer dfinity principal" },
  kaspa: { ticker: "KAS", color: "#49eacb", words: "kaspa" },
  ksm: { ticker: "KSM", color: "#e8026d", words: "kusama" },
  ltc: { ticker: "LTC", color: "#6f8fc7", words: "litecoin" },
  erd: { ticker: "EGLD", color: "#23f7dd", words: "multiversx elrond" },
  near: { ticker: "NEAR", color: "#00c08b", words: "near protocol" },
  osmo: { ticker: "OSMO", color: "#a24bd8", words: "osmosis" },
  dot: { ticker: "DOT", color: "#e6007a", words: "polkadot" },
  xrp: { ticker: "XRP", color: "#a3b1c2", words: "ripple xrpl" },
  sei: { ticker: "SEI", color: "#c8433c", words: "sei" },
  sol: { ticker: "SOL", color: "#a066ff", words: "solana phantom solflare backpack" },
  xlm: { ticker: "XLM", color: "#b3b0ff", words: "stellar lumens" },
  sui: { ticker: "SUI", color: "#4da2ff", words: "sui move" },
  xtz: { ticker: "XTZ", color: "#2c7df7", words: "tezos" },
  ton: { ticker: "TON", color: "#0098ea", words: "toncoin telegram tonkeeper" },
  trx: { ticker: "TRX", color: "#eb0029", words: "tron tronlink" },
};

// "Bitcoin SegWit (bc1q · P2WPKH)" -> { title: "Bitcoin SegWit", detail: "bc1q · P2WPKH" }
export function splitLabel(label) {
  const m = /^(.*?)\s*\((.*)\)\s*$/.exec(label);
  return m ? { title: m[1], detail: m[2] } : { title: label, detail: "" };
}
