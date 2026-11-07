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

const sessionName = (accountId, coin) => `gasbag.session.${accountId}.${coin}`;

function readSession(accountId, coin) {
  try {
    return localStorage.getItem(sessionName(accountId, coin));
  } catch {
    return null;
  }
}

export const hasSession = (accountId, coin) => Boolean(readSession(accountId, coin));

async function authorise(accountId, coin) {
  const pair = KeyPair.fromRandom();
  const wallet = await connector.wallet();
  await wallet.signAndSendTransaction({
    receiverId: accountId,
    actions: [
      {
        type: "AddKey",
        params: {
          publicKey: pair.getPublicKey().toString(),
          accessKey: { permission: { receiverId: coin, allowance: NEAR.toString(), methodNames: ["ignite"] } },
        },
      },
    ],
  });
  localStorage.setItem(sessionName(accountId, coin), pair.toString());
  return pair.toString();
}

/** Starts a lung with the most gas a transaction can carry, and returns once it is included. */
export async function ignite(accountId, coin) {
  const secret = readSession(accountId, coin) ?? (await authorise(accountId, coin));
  const account = new Account(accountId, new JsonRpcProvider({ url: RPC_URLS[0] }), secret);
  const result = await account.signAndSendTransaction({
    receiverId: coin,
    actions: [actions.functionCall("ignite", {}, BigInt(MAX_TX_TGAS) * TGAS, 0n)],
    waitUntil: "INCLUDED",
    throwOnFailure: false,
  });
  return result.transaction?.hash ?? null;
}
