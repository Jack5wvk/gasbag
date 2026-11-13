import { RPC_URLS } from "./config.js";

/** Reads go straight to a public RPC. The page has no backend. */
async function rpc(method, params) {
  let lastError;
  for (const url of RPC_URLS) {
    try {
      const response = await fetch(url, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ jsonrpc: "2.0", id: "gasbag", method, params }),
      });
      if (!response.ok) throw new Error(`RPC ${response.status}`);
      const body = await response.json();
      // `data` says what went wrong, such as an unknown account. `message` is only "Server error".
      if (body.error) throw new Error(body.error.data ?? body.error.message ?? JSON.stringify(body.error));
      if (body.result?.error) throw new Error(body.result.error);
      return body.result;
    } catch (error) {
      lastError = error;
    }
  }
  throw lastError;
}

function toBase64(text) {
  return btoa(unescape(encodeURIComponent(text)));
}

export function liveChain() {
  return {
    live: true,
    async view(contract, method, args = {}) {
      const result = await rpc("query", {
        request_type: "call_function",
        finality: "optimistic",
        account_id: contract,
        method_name: method,
        args_base64: toBase64(JSON.stringify(args)),
      });
      return JSON.parse(new TextDecoder().decode(new Uint8Array(result.result)));
    },
    async keys(account) {
      const result = await rpc("query", { request_type: "view_access_key_list", finality: "final", account_id: account });
      return result.keys;
    },
    async account(account) {
      return rpc("query", { request_type: "view_account", finality: "final", account_id: account });
    },
  };
}
