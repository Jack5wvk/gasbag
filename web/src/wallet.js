import { NearConnector } from "@hot-labs/near-connect";
import { Account, JsonRpcProvider, KeyPair, actions } from "near-api-js";
import { MAX_TX_TGAS, NETWORK, NEAR, RPC_URLS, TGAS } from "./config.js";

const connector = new NearConnector({ network: NETWORK });
const listeners = new Set();
let signedIn = null;

function publish(accountId) {
  signedIn = accountId;
  for (const listener of listeners) listener(accountId);
}

connector.on("wallet:signIn", (event) => publish(event.accounts?.[0]?.accountId ?? null));
connector.on("wallet:signOut", () => publish(null));

export function onAccount(listener) {
  listeners.add(listener);
  listener(signedIn);
}

export async function restore() {
  try {
    const wallet = await connector.wallet();
    const accounts = await wallet.getAccounts();
    if (accounts.length) publish(accounts[0].accountId);
  } catch {
    // Nobody is signed in yet.
  }
}

export const connect = () => connector.connect();
export const disconnect = () => connector.disconnect();

function call(contract, method, args, tgas, deposit) {
  return {
    receiverId: contract,
    actions: [{ type: "FunctionCall", params: { methodName: method, args, gas: (BigInt(tgas) * TGAS).toString(), deposit: deposit.toString() } }],
  };
}

export async function send(contract, method, args, tgas, deposit = 0n) {
  const wallet = await connector.wallet();
  return wallet.signAndSendTransaction(call(contract, method, args, tgas, deposit));
}

// Session keys. A lung's transaction stays open for hours, and wallets wait for a transaction to
// finish, so a wallet can report a timeout even though the lung is fine. A small key that can only
// call `ignite` on this coin lets the page submit the lung itself and move on. It lives in this
// browser, can't move NEAR or tokens, and its gas allowance is capped.

// Prepaid gas is bought at 0.001 NEAR per TGas, so a full lung needs just over 1 NEAR of allowance
// even though only about 0.1 NEAR is finally burned. The rest is refunded breath by breath.
const SESSION_ALLOWANCE = (NEAR * 3n) / 2n;

const sessionName = (accountId, coin) => `gasbag.session.${accountId}.${coin}`;

function readSession(accountId, coin) {
  try {
    return localStorage.getItem(sessionName(accountId, coin));
  } catch {
    return null;
  }
}

function writeSession(accountId, coin, secret) {
  try {
    if (secret) localStorage.setItem(sessionName(accountId, coin), secret);
    else localStorage.removeItem(sessionName(accountId, coin));
  } catch {
    // Without storage the key lasts for this one lung, and the next lung asks the wallet again.
  }
}

async function authorise(accountId, coin) {
  const pair = KeyPair.fromRandom("ed25519");
  const wallet = await connector.wallet();
  await wallet.signAndSendTransaction({
    signerId: accountId,
    receiverId: accountId,
    actions: [
      {
        type: "AddKey",
        params: {
          publicKey: pair.getPublicKey().toString(),
          accessKey: { permission: { receiverId: coin, allowance: SESSION_ALLOWANCE.toString(), methodNames: ["ignite"] } },
        },
      },
    ],
  });
  writeSession(accountId, coin, pair.toString());
  return pair.toString();
}

// Public RPCs rate-limit, so a busy endpoint moves on to the next one. Anything else is a real
// answer from the chain. A repeat can't start a second lung, because each account gets one.
async function startLung(accountId, coin, secret, tgas) {
  let lastError;
  for (const url of RPC_URLS) {
    const account = new Account(accountId, new JsonRpcProvider({ url }), secret);
    try {
      return await account.signAndSendTransaction({
        receiverId: coin,
        actions: [actions.functionCall("ignite", {}, BigInt(tgas) * TGAS, 0n)],
        waitUntil: "INCLUDED",
        throwOnFailure: false,
      });
    } catch (error) {
      lastError = error;
      if (!/rate limit|-429|failed to fetch|network|timed? ?out/i.test(String(error.message ?? error))) throw error;
    }
  }
  throw lastError;
}

/** Starts a lung and returns once it is included. Inclusion doesn't mean the contract accepted it. */
export async function ignite(accountId, coin, tgas = MAX_TX_TGAS) {
  const saved = readSession(accountId, coin);
  if (saved) {
    try {
      return (await startLung(accountId, coin, saved, tgas)).transaction?.hash ?? null;
    } catch (error) {
      // Each lung burns a little of the allowance, and a key can be removed from the wallet. Either
      // way the saved key can't sign, so it is replaced below.
      if (!/allowance|does not exist/i.test(String(error.message ?? error))) throw error;
      writeSession(accountId, coin, null);
    }
  }
  const secret = await authorise(accountId, coin);
  return (await startLung(accountId, coin, secret, tgas)).transaction?.hash ?? null;
}
